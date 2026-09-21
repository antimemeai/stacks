use std::fs;

use stacks_core::*;
use stacks_import::verify::{verify_extraction, Verdict};
use tempfile::TempDir;

fn build_fixture(tmp: &TempDir) -> (Store, std::path::PathBuf) {
    let pages_dir = tmp.path().join("pages");
    fs::create_dir(&pages_dir).unwrap();
    fs::write(
        pages_dir.join("page-0100.txt"),
        "100\nCalcium Fluoride\nCaF2\n100 g. of CaCO3 is suspended in boiling water and 40% HF \
         added until evolution of CO2 ceases. The mixture is filtered hot and dried at 300°C \
         for several hours.\nPROPERTIES:\nWhite powder. M.p. 1418°C.\nREFERENCE:\nO. Ruff.\n",
    )
    .unwrap();
    fs::write(
        pages_dir.join("page-0101.txt"),
        "101\nSome Other Section\nBSO3 is an OCR-mangled formula that appears nowhere else.\n",
    )
    .unwrap();

    let store = Store::open(tmp.path().join("verify.db")).unwrap();
    let insert =
        |key: &str, locator: &str, temp: Option<f64>, formula: &str, grams: Option<f64>| {
            let prov = Provenance {
                id: 0,
                kind: ProvenanceKind::File,
                doi: None,
                source_dataset: Some("fixture".to_string()),
                path: Some("/x.pdf".to_string()),
                sha256: None,
                locator: Some(locator.to_string()),
                extractor_version: Some("fixture".to_string()),
                extraction_method: ExtractionMethod::LlmExtracted,
                confidence: 0.9,
            };
            let prov_id = store.insert_provenance(&prov).unwrap();
            let mat = Material {
                id: 0,
                kind: MaterialKind::Formula,
                inchikey: None,
                canonical_smiles: None,
                formula: Some(formula.to_string()),
                composition: None,
                names: vec![],
                cas: None,
                identity: Some(format!("formula:{formula}")),
            };
            let mat_id = store.insert_material(&mat).unwrap();
            let recipe = Recipe {
                id: 0,
                name: key.to_string(),
                version: 1,
                status: RecipeStatus::Draft,
                target_material_id: Some(mat_id),
                target_quantity: None,
                synthesis_type: None,
                narrative: None,
                created_from_recipe_id: None,
                provenance_id: Some(prov_id),
                created_at: "2026-09-21T00:00:00Z".to_string(),
                created_by: None,
                supersedes: None,
                external_key: Some(format!("fixture:{key}")),
            };
            let rid = store.insert_recipe(&recipe).unwrap();
            let step = RecipeStep {
                id: 0,
                recipe_id: rid,
                ordering: 1,
                operation: Operation::Heat,
                parameters: serde_json::json!({}),
                conditions: Conditions {
                    temperature: temp
                        .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                    ..Conditions::default()
                },
            };
            let sid = store.insert_step(&step).unwrap();
            store
                .insert_step_material(&StepMaterial {
                    id: 0,
                    step_id: sid,
                    material_id: mat_id,
                    role: MaterialRole::Precursor,
                    quantity: grams.map(|g| Quantity::exact(g, Unit::Gram)),
                    equivalents: None,
                    is_reference: false,
                    optional: false,
                    notes: None,
                })
                .unwrap();
        };
    insert("good", "p. 100", Some(300.0), "CaF2", Some(100.0));
    insert("fabricated", "p. 100", Some(999.0), "CaCO3", None);
    // OCR-mangled in source ("BSO3"), repaired in extraction: WARN, not FAIL.
    insert("mangled", "p. 101", None, "B2O3", None);
    (store, pages_dir)
}

#[test]
fn verifier_fails_fabrication_and_warns_on_ocr_repair() {
    let tmp = TempDir::new().unwrap();
    let (store, pages_dir) = build_fixture(&tmp);
    let report = verify_extraction(&store, "fixture", &pages_dir).unwrap();

    assert_eq!(report.recipes, 3);
    let by_key = |k: &str| {
        report
            .results
            .iter()
            .find(|r| r.external_key == format!("fixture:{k}"))
            .unwrap()
    };

    let good = by_key("good");
    assert_eq!(good.verdict, Verdict::Pass, "{:?}", good.fails);

    let fab = by_key("fabricated");
    assert_eq!(fab.verdict, Verdict::Fail);
    assert!(
        fab.fails
            .iter()
            .any(|f| f.kind == "number" && f.fragment.contains("999")),
        "fabricated 999C must be a number FAIL: {:?}",
        fab.fails
    );

    let mangled = by_key("mangled");
    assert_eq!(mangled.verdict, Verdict::Warn, "{:?}", mangled.fails);
    assert!(mangled
        .warns
        .iter()
        .any(|w| w.kind == "formula" && w.fragment == "B2O3"));

    // The good recipe asserts 300C and 100 g, both present; its coverage
    // check must not flag them.
    assert!(!good.coverage.iter().any(|c| c.fragment.contains("300")));
}
