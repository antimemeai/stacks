use std::fs;

use stacks_core::Store;
use stacks_import::{load_extracted, ExtractSource};
use tempfile::TempDir;

fn source() -> ExtractSource {
    ExtractSource {
        source_dataset: "sciencemadness".to_string(),
        book: "brauer".to_string(),
        path: "/corpus/brauer_ocr.pdf".to_string(),
        sha256: "ab".repeat(32),
        extractor_version: "kimi-agent pilot-1".to_string(),
    }
}

fn fixture_jsonl() -> String {
    [
        // Good record.
        r#"{"slug":"brauer:caf2","locator":"p. 233","target_formula":"CaF2","target_names":["Calcium fluoride"],"confidence":0.9,"narrative":"CaCO3 suspended in boiling water; 40% HF added until CO2 evolution ceases; filtered hot; dried at 300 C.","steps":[{"operation":"precipitate","conditions":{},"materials":[{"formula":"CaCO3","role":"precursor","quantity":{"value":100.0,"unit":"g","operator":"eq"}},{"formula":"HF","role":"reactant"}]},{"operation":"dry","conditions":{"temperature":{"kind":"scalar","value":300.0,"unit":"°C","operator":"eq"}},"materials":[]}]}"#.to_string(),
        // Confidence out of range.
        r#"{"slug":"brauer:bad-conf","locator":"p. 1","target_formula":"X","confidence":1.7,"steps":[]}"#.to_string(),
        // Missing locator.
        r#"{"slug":"brauer:no-loc","locator":"  ","target_formula":"Y","confidence":0.5,"steps":[]}"#.to_string(),
        // Unknown unit must die at the type boundary, not reach the DB.
        r#"{"slug":"brauer:bad-unit","locator":"p. 2","target_formula":"Z","confidence":0.5,"steps":[{"operation":"heat","materials":[{"formula":"A","role":"precursor","quantity":{"value":1.0,"unit":"furlong"}}]}]}"#.to_string(),
    ]
    .join("\n")
}

#[test]
fn loader_rejects_invalid_records_and_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("extracted.jsonl");
    fs::write(&jsonl, fixture_jsonl()).unwrap();
    let db = tmp.path().join("pilot.db");
    let mut store = Store::open(&db).unwrap();

    let stats = load_extracted(&mut store, &jsonl, &source(), "2026-09-21T00:00:00Z").unwrap();

    assert_eq!(stats.inserted, 1, "rejected: {:?}", stats.rejected);
    assert_eq!(stats.rejected.len(), 3, "rejected: {:?}", stats.rejected);
    assert!(stats
        .rejected
        .iter()
        .any(|(s, r)| s == "brauer:bad-conf" && r.contains("confidence")));
    assert!(stats
        .rejected
        .iter()
        .any(|(s, r)| s == "brauer:no-loc" && r.contains("locator")));
    assert!(stats
        .rejected
        .iter()
        .any(|(s, r)| s == "brauer:bad-unit" && r.contains("furlong")));

    let recipes: i64 = store
        .raw()
        .query_row("SELECT COUNT(*) FROM recipe", [], |r| r.get(0))
        .unwrap();
    assert_eq!(recipes, 1);

    // The inserted recipe is complete and honestly provenanced.
    let (conf, loc, meth): (f64, String, String) = store
        .raw()
        .query_row(
            "SELECT p.confidence, p.locator, p.extraction_method FROM provenance p
             JOIN recipe r ON r.provenance_id = p.id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(conf, 0.9);
    assert_eq!(loc, "p. 233");
    assert_eq!(meth, "llm_extracted");

    // Re-run: everything skips, nothing duplicates.
    let stats2 = load_extracted(&mut store, &jsonl, &source(), "2026-09-21T00:00:00Z").unwrap();
    assert_eq!(stats2.inserted, 0);
    assert_eq!(stats2.skipped_existing, 1);
    let recipes: i64 = store
        .raw()
        .query_row("SELECT COUNT(*) FROM recipe", [], |r| r.get(0))
        .unwrap();
    assert_eq!(recipes, 1);
}
