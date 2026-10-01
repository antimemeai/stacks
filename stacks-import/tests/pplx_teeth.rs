//! Integration teeth for the pplx-embed shadow path: a canned TcpListener
//! stub stands in for the laptop sidecar, a temp library DB stands in for
//! /srv/stacks/db/library.db.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rusqlite::params;
use stacks_core::library::LibraryStore;
use stacks_import::pplx_embed::{
    backfill, chunks_missing_pplx, HttpPplxClient, BackfillStats, DIMS,
};
use tempfile::TempDir;

struct Stub {
    url: String,
    encode_calls: Arc<AtomicUsize>,
    join: Option<std::thread::JoinHandle<()>>,
}

/// Serve canned pplx-embed responses until the listener is dropped.
/// /encode_documents replies with deterministic 2048-dim i8 vectors whose
/// first element encodes (doc_index, chunk_index) so tests can tell windows
/// apart; /healthz reports a pinned revision.
fn start_stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let encode_calls = Arc::new(AtomicUsize::new(0));
    let calls = encode_calls.clone();
    let join = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
                return;
            }
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).unwrap();
            let body: serde_json::Value =
                serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);

            let payload = if request_line.contains("/healthz") {
                serde_json::json!({"model": "pplx-embed-v2-context-9b-preview", "revision": "abc123"})
            } else if request_line.contains("/encode_documents") {
                calls.fetch_add(1, Ordering::SeqCst);
                let docs = body["documents"].as_array().unwrap();
                let embeddings: Vec<Vec<Vec<i8>>> = docs
                    .iter()
                    .enumerate()
                    .map(|(d, doc)| {
                        doc.as_array()
                            .unwrap()
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                let mut v = vec![0i8; DIMS];
                                v[0] = d as i8;
                                v[1] = (i % 127) as i8;
                                v
                            })
                            .collect()
                    })
                    .collect();
                serde_json::json!({"embeddings": embeddings})
            } else if request_line.contains("/encode_queries") {
                let n = body["queries"].as_array().unwrap().len();
                serde_json::json!({"embeddings": vec![vec![0i8; DIMS]; n]})
            } else {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                stream.write_all(resp.as_bytes()).unwrap();
                continue;
            };
            let payload = payload.to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                payload.len(),
                payload
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });
    Stub {
        url,
        encode_calls,
        join: Some(join),
    }
}

fn fixture_db(tmp: &TempDir, papers: &[(&str, usize)]) -> LibraryStore {
    let store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    for (p, (sha, n_chunks)) in papers.iter().enumerate() {
        let base = p as i64 * 1000;
        store
            .raw()
            .execute(
                "INSERT INTO paper (sha256, filename, title) VALUES (?1, 'f.pdf', 'fixture paper')",
                params![sha],
            )
            .unwrap();
        for id in base + 1..=base + *n_chunks as i64 {
            store
                .raw()
                .execute(
                    "INSERT INTO chunk (corpus, chunk_id, sha256, filename, text)
                     VALUES ('papers', ?1, ?2, 'f.pdf', ?3)",
                    params![id, sha, format!("chunk {id} of {sha}")],
                )
                .unwrap();
        }
    }
    store
}

fn count_pplx(store: &LibraryStore, model: &str) -> i64 {
    store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM chunk_embedding_pplx WHERE model = ?1",
            params![model],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn backfill_happy_path_and_rerun_idempotent() {
    let stub = start_stub();
    let client = HttpPplxClient::new(&stub.url);
    let model = stacks_import::pplx_embed::stamped_model(&client, "pplx-embed-v2-context-9b-preview");
    assert_eq!(model, "pplx-embed-v2-context-9b-preview@abc123");

    let tmp = TempDir::new().unwrap();
    let mut store = fixture_db(&tmp, &[(&"a".repeat(64), 3), (&"b".repeat(64), 5)]);

    let stats: BackfillStats = backfill(&mut store, &client, &model, None).unwrap();
    assert_eq!(stats.papers_done, 2);
    assert_eq!(stats.chunks_embedded, 8);
    assert_eq!(stats.skipped_existing, 0);
    assert_eq!(stats.errors, 0);
    assert_eq!(count_pplx(&store, &model), 8);
    // Every blob is exactly DIMS bytes and round-trips.
    let blob: Vec<u8> = store
        .raw()
        .query_row(
            "SELECT embedding FROM chunk_embedding_pplx WHERE model = ?1 LIMIT 1",
            params![model],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(blob.len(), DIMS);

    // Rerun: nothing left to do (selection excludes fully-embedded papers).
    let stats = backfill(&mut store, &client, &model, None).unwrap();
    assert_eq!(stats.papers_done, 0);
    assert_eq!(stats.chunks_embedded, 0);

    // Partial rerun: delete one row, backfill fills exactly the hole via
    // INSERT OR IGNORE — skipped_existing counts the already-present rows
    // when chunks_missing_pplx is bypassed... here selection picks the paper
    // but only the missing chunk is refetched, so embed exactly 1.
    store
        .raw()
        .execute(
            "DELETE FROM chunk_embedding_pplx WHERE corpus = 'papers' AND chunk_id = 2
               AND model = ?1",
            params![model],
        )
        .unwrap();
    let stats = backfill(&mut store, &client, &model, None).unwrap();
    assert_eq!(stats.papers_done, 2 - 1, "only paper aaa has a hole");
    assert_eq!(stats.chunks_embedded, 1);
    assert_eq!(count_pplx(&store, &model), 8);
}

#[test]
fn windowing_over_128_chunks() {
    let stub = start_stub();
    let client = HttpPplxClient::new(&stub.url);
    let tmp = TempDir::new().unwrap();
    let mut store = fixture_db(&tmp, &[(&"c".repeat(64), 130)]);
    let model = "m@rev";

    let stats = backfill(&mut store, &client, model, None).unwrap();
    assert_eq!(stats.papers_done, 1);
    assert_eq!(stats.chunks_embedded, 130);
    assert_eq!(
        stub.encode_calls.load(Ordering::SeqCst),
        2,
        "130 chunks = windows of 128 + 2"
    );
    assert_eq!(count_pplx(&store, model), 130);
    assert!(chunks_missing_pplx(store.raw(), model, "papers", &"c".repeat(64))
        .unwrap()
        .is_empty());
}

#[test]
fn limit_caps_papers_per_run() {
    let stub = start_stub();
    let client = HttpPplxClient::new(&stub.url);
    let tmp = TempDir::new().unwrap();
    let mut store = fixture_db(&tmp, &[(&"a".repeat(64), 2), (&"b".repeat(64), 2), (&"c".repeat(64), 2)]);
    let model = "m@rev";

    let stats = backfill(&mut store, &client, model, Some(2)).unwrap();
    assert_eq!(stats.papers_done, 2);
    assert_eq!(count_pplx(&store, model), 4);
    let stats = backfill(&mut store, &client, model, Some(2)).unwrap();
    assert_eq!(stats.papers_done, 1);
    assert_eq!(count_pplx(&store, model), 6);
}
