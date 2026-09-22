use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use stacks_core::*;
use tempfile::TempDir;
use tower::ServiceExt;

fn recipe(i: i64, source: &str, status: RecipeStatus) -> Recipe {
    Recipe {
        id: 0,
        name: format!("fixture recipe {i}"),
        version: 1,
        status,
        target_material_id: None,
        target_quantity: None,
        synthesis_type: Some(SynthesisType::SolidState),
        narrative: None,
        created_from_recipe_id: None,
        provenance_id: None,
        created_at: "2026-09-21T00:00:00Z".to_string(),
        created_by: Some("fixture".to_string()),
        supersedes: None,
        external_key: Some(format!("{source}:fixture-{i}")),
        outcome: None,
        outcome_score: None,
    }
}

fn provenance(source: &str) -> Provenance {
    Provenance {
        id: 0,
        kind: ProvenanceKind::DatasetImport,
        doi: None,
        source_dataset: Some(source.to_string()),
        path: None,
        sha256: None,
        locator: None,
        extractor_version: None,
        extraction_method: ExtractionMethod::Structured,
        confidence: 1.0,
        note: None,
    }
}

fn material(i: i64) -> Material {
    Material {
        id: 0,
        kind: MaterialKind::Formula,
        inchikey: None,
        canonical_smiles: None,
        formula: Some(format!("Fe{i}O3")),
        composition: None,
        names: vec![format!("fixture oxide {i}")],
        cas: None,
        identity: Some(format!("formula:Fe{i}O3")),
    }
}

/// Build a fixture DB and return the app plus the expected "full" recipe
/// bundle (with ids filled in) for the detail round-trip test.
fn fixture() -> (TempDir, axum::Router, Value) {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("fixture.db");
    let bundle;
    {
        let store = Store::open(&db).unwrap();
        // 25 paged recipes: src_a 15 draft, src_b 10 deployed.
        for i in 1..=25 {
            let (source, status) = if i <= 15 {
                ("src_a", RecipeStatus::Draft)
            } else {
                ("src_b", RecipeStatus::Deployed)
            };
            let prov_id = store.insert_provenance(&provenance(source)).unwrap();
            let mat_id = store.insert_material(&material(i)).unwrap();
            let mut r = recipe(i, source, status.clone());
            r.provenance_id = Some(prov_id);
            r.target_material_id = Some(mat_id);
            store.insert_recipe(&r).unwrap();
        }
        // One full recipe with steps + materials + provenance.
        let prov_id = store.insert_provenance(&provenance("src_full")).unwrap();
        let target_id = store.insert_material(&material(100)).unwrap();
        let solvent_id = store
            .insert_material(&Material {
                id: 0,
                kind: MaterialKind::Molecule,
                inchikey: None,
                canonical_smiles: Some("O".to_string()),
                formula: None,
                composition: None,
                names: vec!["water".to_string()],
                cas: Some("7732-18-5".to_string()),
                identity: Some("smiles:O".to_string()),
            })
            .unwrap();
        let mut r = recipe(100, "src_full", RecipeStatus::Draft);
        r.provenance_id = Some(prov_id);
        r.target_material_id = Some(target_id);
        r.narrative = Some("full fixture recipe".to_string());
        r.target_quantity = Some(Quantity::exact(5.0, Unit::Gram));
        let recipe_id = store.insert_recipe(&r).unwrap();
        let step = RecipeStep {
            id: 0,
            recipe_id,
            ordering: 1,
            operation: Operation::Heat,
            parameters: serde_json::json!({"token": "heated"}),
            conditions: Conditions {
                temperature: Some(Temperature::MinMax {
                    min: Quantity::exact(800.0, Unit::Celsius),
                    max: Quantity::exact(900.0, Unit::Celsius),
                }),
                pressure: None,
                duration: Some(Quantity::exact(2.0, Unit::Hour)),
                atmosphere: Some(Atmosphere::Air),
                ph: None,
                stirring: None,
                atmosphere_note: None,
            },
        };
        let step_id = store.insert_step(&step).unwrap();
        let sm = StepMaterial {
            id: 0,
            step_id,
            material_id: solvent_id,
            role: MaterialRole::Solvent,
            quantity: Some(Quantity {
                value: 50.0,
                unit: Unit::Milliliter,
                operator: Operator::Approx,
            }),
            equivalents: Some(10.0),
            is_reference: false,
            optional: false,
            notes: Some("DI".to_string()),
        };
        store.insert_step_material(&sm).unwrap();
        bundle = store.load_recipe(recipe_id).unwrap().unwrap();
    }
    let store = Store::open_read_only(&db).unwrap();
    let expected = serde_json::json!({
        "recipe": bundle.recipe,
        "provenance": bundle.provenance,
        "steps": [{
            "step": bundle.steps[0].step,
            "materials": bundle.steps[0].materials,
        }],
    });
    (tmp, stacks_api::build_app(store), expected)
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
async fn error_shape_for_bad_queries() {
    let (_t, app, _) = fixture();

    // Unknown parameter.
    let (status, body) = get(&app, "/api/v1/recipes?bogus=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_query");
    assert!(body["error"]["message"].is_string());

    // Over-cap limit.
    let (status, body) = get(&app, "/api/v1/recipes?limit=5000").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_query");
    assert_eq!(body["error"]["details"]["max"], 100);

    // Non-integer limit and cursor.
    for uri in ["/api/v1/recipes?limit=abc", "/api/v1/recipes?cursor=xyz"] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["error"]["code"], "invalid_query");
    }

    // Same contract on /materials.
    let (status, body) = get(&app, "/api/v1/materials?bogus=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_query");

    // Not found is its own stable code.
    let (status, body) = get(&app, "/api/v1/recipes/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error"]["code"], "not_found");

    // No internals anywhere in any error body.
    for uri in [
        "/api/v1/recipes?bogus=1",
        "/api/v1/recipes?limit=5000",
        "/api/v1/recipes/999999",
        "/api/v1/schemas/Nope",
    ] {
        let (_, body) = get(&app, uri).await;
        let text = body.to_string();
        for leak in ["rusqlite", "sqlite", "SQL", "panicked"] {
            assert!(!text.contains(leak), "{uri} leaks {leak}: {text}");
        }
    }
}

#[tokio::test]
async fn pagination_is_deterministic_and_complete() {
    let (_t, app, _) = fixture();

    async fn crawl(app: &axum::Router, base: &str) -> Vec<i64> {
        let mut ids = Vec::new();
        let mut cursor: Option<i64> = None;
        loop {
            let uri = match cursor {
                Some(c) => format!("{base}&cursor={c}"),
                None => base.to_string(),
            };
            let (status, body) = get(app, &uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            for row in body["data"].as_array().unwrap() {
                ids.push(row["id"].as_i64().unwrap());
            }
            match body["meta"]["next_cursor"].as_i64() {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        ids
    }

    // Two identical crawls return identical id sequences.
    let run1 = crawl(&app, "/api/v1/recipes?limit=7").await;
    let run2 = crawl(&app, "/api/v1/recipes?limit=7").await;
    assert_eq!(run1, run2, "same paged requests must be deterministic");

    // The cursor chain covers all 26 recipes (25 paged + 1 full) without
    // duplicates or gaps.
    assert_eq!(run1.len(), 26);
    let mut sorted = run1.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 26, "duplicates in cursor chain");

    // Source filter composes with pagination.
    let a = crawl(&app, "/api/v1/recipes?limit=4&source=src_a").await;
    assert_eq!(a.len(), 15);
    let b = crawl(&app, "/api/v1/recipes?limit=100&source=src_b").await;
    assert_eq!(b.len(), 10);

    // Materials pagination: 27 materials total.
    let mats = crawl(&app, "/api/v1/recipes/materials?limit=10").await;
    assert_eq!(mats.len(), 27);
}

#[tokio::test]
async fn schema_endpoints_serve_valid_json_schema() {
    let (_t, app, _) = fixture();

    let (status, body) = get(&app, "/api/v1/schemas").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = body["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for required in [
        "Recipe",
        "Material",
        "RecipeStep",
        "StepMaterial",
        "Quantity",
        "Conditions",
        "Run",
        "Provenance",
    ] {
        assert!(names.contains(&required), "missing schema {required}");
    }

    let (status, body) = get(&app, "/api/v1/schemas/Quantity").await;
    assert_eq!(status, StatusCode::OK);
    let text = body.to_string();
    // Closed unit token list must be visible to agents.
    for token in ["\"g\"", "\"µL\"", "\"mbar\"", "\"Torr\"", "\"mmol\""] {
        assert!(
            text.contains(token),
            "unit token {token} missing from schema"
        );
    }
    // Valid JSON Schema shape.
    assert!(
        body.get("definitions").is_some() || body.get("$schema").is_some(),
        "{text}"
    );

    let (status, body) = get(&app, "/api/v1/schemas/Nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn recipe_detail_round_trips_fixture() {
    let (_t, app, expected) = fixture();
    let id = expected["recipe"]["id"].as_i64().unwrap();
    let (status, body) = get(&app, &format!("/api/v1/recipes/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body, expected,
        "detail must match the fixture field-for-field"
    );
}

#[tokio::test]
async fn discovery_and_health() {
    let (_t, app, _) = fixture();
    let (status, body) = get(&app, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let (status, body) = get(&app, "/api/v1").await;
    assert_eq!(status, StatusCode::OK);
    let text = body.to_string();
    assert!(text.contains("/api/v1/schemas"));
    assert!(text.contains("/api/v1/recipes"));
}
