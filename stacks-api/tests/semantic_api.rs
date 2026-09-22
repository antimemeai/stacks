use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use stacks_api::semantic::{brute_force_topk, Embedder};
use stacks_core::library::*;
use stacks_core::Store;
use tempfile::TempDir;
use tower::ServiceExt;

struct FixtureEmbedder(Vec<f32>);

impl Embedder for FixtureEmbedder {
    fn embed(&mut self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(self.0.clone())
    }
}

fn unit(dim_index: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 384];
    v[dim_index] = 1.0;
    v
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn fixture(tmp: &TempDir) -> (Store, LibraryStore) {
    let recipes = Store::open(tmp.path().join("stacks.db")).unwrap();
    let lib = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    // Three chunks, orthogonal unit vectors; query will be axis 1.
    for (id, axis) in [(1, 0), (2, 1), (3, 2)] {
        insert_chunk(
            lib.raw(),
            &LibraryChunk {
                rowid: 0,
                corpus: "test".to_string(),
                chunk_id: id,
                sha256: None,
                filename: "f.pdf".to_string(),
                title: None,
                section: None,
                text: format!("chunk {id}"),
                word_count: None,
                embedding: Some(to_blob(&unit(axis))),
            },
        )
        .unwrap();
    }
    (recipes, lib)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn semantic_validation_and_retrieval_correctness() {
    let tmp = TempDir::new().unwrap();
    let (recipes, lib) = fixture(&tmp);
    let embedder = FixtureEmbedder(unit(1));
    let app = stacks_api::build_app_semantic(recipes, Some(lib), Some(Box::new(embedder)));

    // (a) missing / empty query -> 400 invalid_query.
    for uri in ["/api/v1/chunks/semantic", "/api/v1/chunks/semantic?query="] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["error"]["code"], "invalid_query");
    }
    // k over cap.
    let (status, body) = get(&app, "/api/v1/chunks/semantic?query=x&k=51").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["details"]["max"], 50);
    // unknown param.
    let (status, _body) = get(&app, "/api/v1/chunks/semantic?query=x&bogus=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // (b) retrieval correctness: query vector = axis 1 -> chunk 2 wins.
    let (status, body) = get(&app, "/api/v1/chunks/semantic?query=anything&k=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["chunk_id"], 2, "nearest must be the axis-1 chunk");
    assert!(rows[0]["score"].as_f64().unwrap() > 0.99);
    assert_eq!(body["meta"]["max_k"], 50);

    // hybrid mode fuses with BM25 (sparse leg finds 'chunk 2' text).
    let (status, body) = get(&app, "/api/v1/chunks/semantic?query=chunk&mode=hybrid&k=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["meta"]["fusion"], "rrf(k=60)");
    assert!(!body["data"].as_array().unwrap().is_empty());
}

#[test]
fn brute_force_detects_corruption() {
    // Axis-1 chunk must win for an axis-1 query; with its bytes corrupted
    // (zeroed), it must NOT win — the assertion can detect wrong retrieval.
    let rowids = vec![1, 2, 3];
    let mut matrix = [unit(0), unit(1), unit(2)].concat();
    let q = unit(1);
    let top = brute_force_topk(&rowids, &matrix, &q, 1);
    assert_eq!(top[0].0, 2);
    assert!(top[0].1 > 0.99);

    // Corrupt chunk 2's bytes.
    for f in matrix[384..768].iter_mut() {
        *f = 0.0;
    }
    let top = brute_force_topk(&rowids, &matrix, &q, 1);
    assert_ne!(top[0].0, 2, "corrupted winner must not survive");
}

#[test]
fn rrf_matches_reference_formula() {
    // NL scripts/wave_corpus/retriever.py: score = Σ 1/(60 + rank), 1-based.
    let dense = vec![(10, 0.9f32), (20, 0.8), (30, 0.7)];
    let sparse = vec![(20, -5.0f64), (10, -6.0), (40, -7.0)];
    let fused = stacks_api::semantic::rrf_fuse(&dense, &sparse, 60, 10);
    let score_of = |rid: i64| fused.iter().find(|(r, _)| *r == rid).unwrap().1;
    let s10 = 1.0 / 61.0 + 1.0 / 62.0; // dense rank 1 + sparse rank 2
    assert!((score_of(10) - s10).abs() < 1e-12);
    assert_eq!(fused[0].0, 10);
    assert_eq!(fused.len(), 4);
}
