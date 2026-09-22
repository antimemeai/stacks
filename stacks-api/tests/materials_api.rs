use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use stacks_core::materials::*;
use stacks_core::Store;
use tempfile::TempDir;
use tower::ServiceExt;

fn fixture(tmp: &TempDir) -> axum::Router {
    let recipes = Store::open(tmp.path().join("stacks.db")).unwrap();
    let m = MaterialsStore::open(tmp.path().join("materials.db")).unwrap();
    for (i, gap) in [(1, 0.5), (2, 1.7), (3, 3.2)] {
        let key = format!("mp:mp-{i}");
        stacks_core::materials::insert_entry(
            m.raw(),
            &MaterialEntry {
                external_key: key.clone(),
                source: "mp".to_string(),
                source_id: format!("mp-{i}"),
                formula: Some(format!("Fe{i}O3")),
                elements: Some(vec!["Fe".to_string(), "O".to_string()]),
                nsites: Some(10),
                spacegroup: Some("R-3c".to_string()),
                spacegroup_number: Some(167),
                crystal_system: Some("Trigonal".to_string()),
                cell: None,
                structure: None,
                density: Some(5.2),
                description: if i == 1 {
                    Some("Iron oxide crystallizes…".to_string())
                } else {
                    None
                },
                reference_path: None,
                imported_at: "2026-09-22".to_string(),
            },
        )
        .unwrap();
        stacks_core::materials::insert_property(
            m.raw(),
            &Property {
                id: 0,
                external_key: key,
                collection: "summary".to_string(),
                kind: "band_gap".to_string(),
                value: Some(gap),
                unit: Some("eV".to_string()),
                text_value: None,
                extra: None,
            },
        )
        .unwrap();
    }
    stacks_api::build_app_materials(recipes, None, None, Some(m))
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
async fn materials_endpoints() {
    let tmp = TempDir::new().unwrap();
    let app = fixture(&tmp);

    // (c) unknown param -> 400 invalid_query
    let (status, body) = get(&app, "/api/v1/materials?bogus=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_query");

    // band_gap range filter
    let (status, body) = get(&app, "/api/v1/materials?band_gap_min=1.0&band_gap_max=2.0").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["formula"], "Fe2O3");

    // elements + query filters
    let (status, body) = get(&app, "/api/v1/materials?query=Fe2O3&source=mp").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    let (status, body) = get(&app, "/api/v1/materials?source=cod").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);

    // detail with property bundle + robocrys
    let (status, body) = get(&app, "/api/v1/materials/mp:mp-1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["entry"]["description"], "Iron oxide crystallizes…");
    assert_eq!(body["properties"][0]["kind"], "band_gap");

    let (status, body) = get(&app, "/api/v1/materials/mp:nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // status
    let (status, body) = get(&app, "/api/v1/materials/status").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["counts"]["entries"], 3);
}
