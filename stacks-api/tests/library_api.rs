use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use stacks_core::library::*;
use stacks_core::Store;
use tempfile::TempDir;
use tower::ServiceExt;

fn sha(ch: char) -> String {
    ch.to_string().repeat(64)
}

fn paper(ch: char, title: &str, subfield: &str, year: i64, doi: Option<&str>) -> LibraryPaper {
    LibraryPaper {
        sha256: sha(ch),
        filename: format!("{ch}.pdf"),
        path: Some(format!("lib/{subfield}/{ch}.pdf")),
        size_bytes: None,
        registered_at: None,
        on_disk: None,
        doi: doi.map(str::to_string),
        arxiv_id: None,
        title: Some(title.to_string()),
        authors: Some("A. Author".to_string()),
        year: Some(year),
        abstract_: None,
        journal: None,
        source_url: None,
        access: None,
        blob_key: None,
        blob_synced_at: None,
        subfield: Some(subfield.to_string()),
        tags: None,
        original_language: None,
        original_script_title: None,
        transliterated_title: None,
        translation_of: None,
        translated_in: None,
        soviet_stratum: None,
        source_collection: None,
    }
}

fn fixture(tmp: &TempDir) -> (Store, LibraryStore) {
    // Minimal recipes DB (schema only).
    let recipes = Store::open(tmp.path().join("stacks.db")).unwrap();

    let lib = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    // 25 papers: 15 in ceramics (years 2000-2014, doi), 10 in optics
    // (years 1990-1999, no doi).
    for i in 0..25 {
        let ch = char::from(b'a' + i as u8);
        let (subfield, year, doi) = if i < 15 {
            ("ceramics", 2000 + i as i64, Some("10.1/x"))
        } else {
            ("optics", 1990 + i as i64 - 15, None)
        };
        stacks_core::library::insert_paper(
            lib.raw(),
            &paper(ch, &format!("Paper {i} on sintering"), subfield, year, doi),
        )
        .unwrap();
    }
    stacks_core::library::insert_enrichment(
        lib.raw(),
        &PaperEnrichment {
            sha256: sha('a'),
            openalex_id: Some("W1".to_string()),
            openalex_topics: None,
            openalex_concepts: None,
            openalex_cited_by: Some(42),
            s2_paper_id: None,
            s2_tldr: Some("a paper".to_string()),
            s2_fields_of_study: None,
            s2_influential_citation_count: None,
            unpaywall_oa_status: None,
            unpaywall_oa_url: None,
            enriched_at: None,
        },
    )
    .unwrap();
    // Chunks for paper a: 7 chunks with searchable text.
    for j in 1..=7 {
        stacks_core::library::insert_chunk(
            lib.raw(),
            &LibraryChunk {
                rowid: 0,
                corpus: "ceramics".to_string(),
                chunk_id: j,
                sha256: Some(sha('a')),
                filename: "a.pdf".to_string(),
                title: None,
                section: Some("methods".to_string()),
                text: format!("sintering of alumina ceramics at high temperature, part {j}"),
                word_count: Some(9),
                embedding: None,
            },
        )
        .unwrap();
    }
    // One chunk in another corpus for the corpus filter.
    stacks_core::library::insert_chunk(
        lib.raw(),
        &LibraryChunk {
            rowid: 0,
            corpus: "optics".to_string(),
            chunk_id: 1,
            sha256: Some(sha('p')),
            filename: "p.pdf".to_string(),
            title: None,
            section: None,
            text: "sintering is irrelevant here; lenses and mirrors".to_string(),
            word_count: Some(7),
            embedding: None,
        },
    )
    .unwrap();
    stacks_import::library_import::rebuild_fts(&lib).unwrap();
    stacks_core::library::insert_import_meta(
        lib.raw(),
        "neurotic_library",
        "/nl/catalog/papers.parquet",
        Some("2026-06-11"),
        "2026-09-22T00:00:00Z",
        &serde_json::json!({"papers": 25}),
    )
    .unwrap();
    (recipes, lib)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn library_unknown_params_and_pagination() {
    let tmp = TempDir::new().unwrap();
    let (recipes, lib) = fixture(&tmp);
    let app = stacks_api::build_app_full(recipes, Some(lib));

    // Unknown params rejected with the stable shape.
    for uri in [
        "/api/v1/papers?bogus=1",
        "/api/v1/chunks/search?query=x&bogus=1",
        "/api/v1/papers?limit=1000",
    ] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["error"]["code"], "invalid_query");
    }

    // Deterministic keyset pagination over papers.
    async fn crawl(app: &axum::Router, base: &str) -> Vec<String> {
        let mut keys = Vec::new();
        let mut cursor: Option<i64> = None;
        loop {
            let uri = match cursor {
                Some(c) => format!("{base}&cursor={c}"),
                None => base.to_string(),
            };
            let (status, body) = get(app, &uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            for row in body["data"].as_array().unwrap() {
                let key = row["sha256"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| row["rowid"].as_i64().map(|i| i.to_string()));
                keys.push(key.unwrap_or_else(|| panic!("no key in {row}")));
            }
            match body["meta"]["next_cursor"].as_i64() {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        keys
    }
    let run1 = crawl(&app, "/api/v1/papers?limit=7").await;
    let run2 = crawl(&app, "/api/v1/papers?limit=7").await;
    assert_eq!(run1, run2);
    assert_eq!(run1.len(), 25);

    let ceramics = crawl(&app, "/api/v1/papers?limit=5&subfield=ceramics").await;
    assert_eq!(ceramics.len(), 15);
    let nodoi = crawl(&app, "/api/v1/papers?limit=100&has_doi=false").await;
    assert_eq!(nodoi.len(), 10);
    let recent = crawl(&app, "/api/v1/papers?limit=100&year_min=2010").await;
    assert_eq!(recent.len(), 5);

    // Paper detail with enrichment + chunk count.
    let (status, body) = get(&app, &format!("/api/v1/papers/{}", sha('a'))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["paper"]["title"], "Paper 0 on sintering");
    assert_eq!(body["enrichment"]["openalex_cited_by"], 42);
    assert_eq!(body["chunk_count"], 7);

    let (status, body) = get(&app, &format!("/api/v1/papers/{}", sha('z'))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // Paper chunks paged.
    let chunks = crawl(&app, &format!("/api/v1/papers/{}/chunks?limit=3", sha('a'))).await;
    assert_eq!(chunks.len(), 7);

    // FTS search: finds sintering chunks, corpus filter narrows.
    let (status, body) = get(&app, "/api/v1/chunks/search?query=sintering&limit=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 3);
    let (status, body) = get(
        &app,
        "/api/v1/chunks/search?query=sintering&corpus=optics&limit=10",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["corpus"], "optics");

    // Search pagination covers all 8 matches deterministically.
    let mut seen = std::collections::BTreeSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let uri = match &cursor {
            Some(c) => format!("/api/v1/chunks/search?query=sintering&limit=3&cursor={c}"),
            None => "/api/v1/chunks/search?query=sintering&limit=3".to_string(),
        };
        let (status, body) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        for row in body["data"].as_array().unwrap() {
            assert!(
                seen.insert(row["rowid"].as_i64().unwrap()),
                "duplicate in crawl"
            );
        }
        match body["meta"]["next_cursor"].as_str() {
            Some(c) => cursor = Some(c.to_string()),
            None => break,
        }
    }
    assert_eq!(seen.len(), 8);

    // Status endpoint.
    let (status, body) = get(&app, "/api/v1/library/status").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["counts"]["papers"], 25);
    assert_eq!(body["counts"]["chunks"], 8);
    assert!(body["imports"][0]["source_mtime"].is_string());

    // Schemas registered.
    let (_, body) = get(&app, "/api/v1/schemas").await;
    let names: Vec<&str> = body["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for n in ["LibraryPaper", "PaperEnrichment", "LibraryChunk"] {
        assert!(names.contains(&n), "missing {n}");
    }
}
