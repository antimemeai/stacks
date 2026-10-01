//! Integration smoke test: real streamable-HTTP transport against an
//! ephemeral loopback port with a temp fixture library DB.

use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use stacks_core::library::*;
use tempfile::TempDir;

fn sha(ch: char) -> String {
    ch.to_string().repeat(64)
}

fn fixture(tmp: &TempDir) -> std::path::PathBuf {
    let path = tmp.path().join("library.db");
    let lib = LibraryStore::open(&path).unwrap();
    insert_paper(
        lib.raw(),
        &LibraryPaper {
            sha256: sha('a'),
            filename: "a.pdf".to_string(),
            path: Some("lib/ceramics/a.pdf".to_string()),
            size_bytes: None,
            registered_at: None,
            on_disk: None,
            doi: Some("10.1/x".to_string()),
            arxiv_id: None,
            title: Some("Sintering of zirconia".to_string()),
            authors: Some("A. Author".to_string()),
            year: Some(2001),
            abstract_: None,
            journal: None,
            source_url: None,
            access: None,
            blob_key: None,
            blob_synced_at: None,
            subfield: Some("ceramics".to_string()),
            tags: None,
            original_language: None,
            original_script_title: None,
            transliterated_title: None,
            translation_of: None,
            translated_in: None,
            soviet_stratum: None,
            source_collection: None,
        },
    )
    .unwrap();
    insert_chunk(
        lib.raw(),
        &LibraryChunk {
            rowid: 0,
            corpus: "test".to_string(),
            chunk_id: 1,
            sha256: Some(sha('a')),
            filename: "a.pdf".to_string(),
            title: None,
            section: None,
            text: "zirconia sintering kinetics grain growth".to_string(),
            word_count: None,
            embedding: None,
        },
    )
    .unwrap();
    lib.raw()
        .execute_batch("INSERT INTO chunk_fts(chunk_fts) VALUES('rebuild')")
        .unwrap();
    // WAL-checkpoint so a separate read-only connection sees the data.
    lib.raw()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    path
}

fn call_args(name: &'static str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name).with_arguments(args.as_object().cloned().unwrap())
}

fn text_of(result: &CallToolResult) -> serde_json::Value {
    let ContentBlock::Text(t) = result.content.first().expect("tool returned no content") else {
        panic!("expected text content, got {:?}", result.content);
    };
    serde_json::from_str(&t.text).expect("tool content must be JSON")
}

#[tokio::test]
async fn mcp_over_http_initialize_list_call() {
    let tmp = TempDir::new().unwrap();
    let db = fixture(&tmp);
    let library = LibraryStore::open_read_only(&db).unwrap();
    let service = stacks_mcp::build_service(library, tmp.path().join("no-model-cache"));
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let transport = StreamableHttpClientTransport::from_uri(format!("http://{addr}/mcp").as_str());
    let client = ().serve(transport).await.expect("initialize handshake");
    let info = client.peer_info().expect("server info");
    assert_eq!(
        info.server_info.as_ref().map(|i| i.name.as_str()),
        Some("stacks-mcp")
    );

    let tools = client.list_tools(None).await.expect("tools/list");
    let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in [
        "search_chunks_bm25",
        "search_chunks_semantic",
        "get_paper",
        "list_papers",
        "library_status",
        "get_document",
        "get_chunk",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}: {names:?}");
    }

    let result = client
        .call_tool(call_args(
            "search_chunks_bm25",
            serde_json::json!({"query": "zirconia"}),
        ))
        .await
        .expect("tools/call search_chunks_bm25");
    let body = text_of(&result);
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{body}");
    assert_eq!(rows[0]["corpus"], "test");
    assert!(rows[0]["snippet"].as_str().unwrap().contains("<b>"));

    let result = client
        .call_tool(call_args(
            "get_chunk",
            serde_json::json!({"corpus": "test", "chunk_id": 1}),
        ))
        .await
        .expect("tools/call get_chunk");
    let body = text_of(&result);
    assert_eq!(body["text"], "zirconia sintering kinetics grain growth", "{body}");
    assert_eq!(body["sha256"], sha('a'));

    let result = client
        .call_tool(call_args("library_status", serde_json::json!({})))
        .await
        .expect("tools/call library_status");
    let body = text_of(&result);
    assert_eq!(body["counts"]["papers"], 1, "{body}");
    assert_eq!(body["counts"]["chunks"], 1);

    // Not-found returns an error result (visible to the caller), not a
    // protocol error.
    let result = client
        .call_tool(call_args("get_paper", serde_json::json!({"sha256": sha('z')})))
        .await
        .expect("tools/call get_paper");
    assert_eq!(result.is_error, Some(true));

    // Invalid params surface as a JSON-RPC error.
    let err = client
        .call_tool(call_args(
            "search_chunks_bm25",
            serde_json::json!({"query": "x", "limit": 101}),
        ))
        .await
        .expect_err("limit 101 must be rejected");
    assert!(err.to_string().contains("limit"), "{err}");

    client.cancel().await.unwrap();
}
