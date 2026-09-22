use std::fs;

use stacks_core::materials::*;
use stacks_import::materials_wave::*;
use stacks_import::wave::{WaveCtx, WaveSource};
use tempfile::TempDir;

fn ctx() -> WaveCtx {
    WaveCtx {
        workdir: std::path::PathBuf::new(),
        imported_at: "2026-09-22T00:00:00Z".to_string(),
    }
}

#[test]
fn invalid_property_unit_rejected_by_check() {
    let store = MaterialsStore::open_in_memory().unwrap();
    insert_entry(
        store.raw(),
        &MaterialEntry {
            external_key: "mp:mp-1".to_string(),
            source: "mp".to_string(),
            source_id: "mp-1".to_string(),
            formula: Some("Si".to_string()),
            elements: None,
            nsites: None,
            spacegroup: None,
            spacegroup_number: None,
            crystal_system: None,
            cell: None,
            structure: None,
            density: None,
            description: None,
            reference_path: None,
            imported_at: "2026-09-22".to_string(),
        },
    )
    .unwrap();
    // furlong is not a unit — CHECK must reject it at the DB fence.
    let err = store.raw().execute(
        "INSERT INTO property (external_key, collection, kind, value, unit)
         VALUES ('mp:mp-1', 'x', 'band_gap', 1.0, 'furlong')",
        [],
    );
    assert!(err.is_err(), "invalid unit must violate CHECK: {err:?}");
    // other: escape hatch works.
    store
        .raw()
        .execute(
            "INSERT INTO property (external_key, collection, kind, value, unit)
             VALUES ('mp:mp-1', 'x', 'debye_temperature', 300.0, 'other:K')",
            [],
        )
        .unwrap();
}

#[test]
fn cod_import_quarantines_garbled_and_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("cod.jsonl");
    fs::write(
        &jsonl,
        concat!(
            r#"{"formula":"Mg O3 Si","spacegroup":"P 21 c n","a":6.532,"b":6.0,"c":3.464,"alpha":90.0,"beta":90.0,"gamma":90.0,"volume":135.761,"cod_id":"9003435","member":"cif/9/00/34/9003435.cif"}"#,
            "\n",
            r#"{"cod_id":"7133664","member":"cif/7/13/36/7133664.cif","parse_failed":true,"error":"invalid compressed data"}"#,
            "\n"
        ),
    )
    .unwrap();
    let mut store = MaterialsStore::open(tmp.path().join("m.db")).unwrap();
    let cod = Cod {
        jsonl: jsonl.to_string_lossy().to_string(),
        archive: "cod-cifs-mysql.txz".to_string(),
    };
    let s1 = cod.run(&mut store, &ctx()).unwrap();
    assert_eq!(s1.inserted, 1);
    assert_eq!(s1.rejected, 1);

    // Garbled row: quarantined with the reason, no entry fabricated.
    let (reason, detail): (String, Option<String>) = store
        .raw()
        .query_row(
            "SELECT reason, detail FROM material_quarantine WHERE source_id = '7133664'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(reason, "parse_failure");
    assert!(detail.unwrap().contains("invalid compressed data"));
    let fabricated: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM material_entry WHERE source_id = '7133664'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fabricated, 0);

    // Clean row: formula normalized, cell preserved, reference in place.
    let (formula, cell, reference): (String, String, String) = store
        .raw()
        .query_row(
            "SELECT formula, cell_json, reference_path FROM material_entry WHERE external_key = 'cod:9003435'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(formula, "MgO3Si");
    assert!(cell.contains("6.532"));
    assert!(reference.contains("9003435.cif"));

    // (b) re-import inserts 0.
    let s2 = cod.run(&mut store, &ctx()).unwrap();
    assert_eq!(s2.inserted, 0);
    assert_eq!(s2.skipped_existing, 1);
    assert_eq!(s2.rejected, 1);
}

#[test]
fn mp_summary_entry_and_properties() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("summary");
    fs::create_dir(&dir).unwrap();
    let rec = serde_json::json!({
        "material_id": "mp-13",
        "formula_pretty": "Fe2O3",
        "chemsys": "Fe-O",
        "elements": ["Fe", "O"],
        "nsites": 10,
        "band_gap": 2.1,
        "energy_above_hull": 0.0,
        "formation_energy_per_atom": -1.5,
        "density": 5.24,
        "is_stable": true,
        "ordering": "FM",
        "symmetry": {"symbol": "R-3c", "number": 167, "crystal_system": "Trigonal"},
        "structure": {"lattice": {"a": 5.0, "b": 5.0, "c": 13.7, "alpha": 90.0, "beta": 90.0, "gamma": 120.0, "volume": 300.0}, "sites": []}
    });
    use std::io::Write;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(serde_json::to_string(&rec).unwrap().as_bytes())
        .unwrap();
    fs::write(dir.join("shard.jsonl.gz"), gz.finish().unwrap()).unwrap();

    let mut store = MaterialsStore::open(tmp.path().join("m.db")).unwrap();
    let src = MpSummary {
        dir: dir.to_string_lossy().to_string(),
    };
    let s1 = src.run(&mut store, &ctx()).unwrap();
    assert_eq!(s1.inserted, 1);

    let gap: f64 = store
        .raw()
        .query_row(
            "SELECT value FROM property WHERE external_key = 'mp:mp-13' AND kind = 'band_gap'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!((gap - 2.1).abs() < 1e-12);
    let (sg, sgn, cell): (String, i64, String) = store
        .raw()
        .query_row(
            "SELECT spacegroup, spacegroup_number, cell_json FROM material_entry WHERE external_key = 'mp:mp-13'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(sg, "R-3c");
    assert_eq!(sgn, 167);
    assert!(cell.contains("13.7"));

    // Idempotency: entries AND properties dedupe.
    let s2 = src.run(&mut store, &ctx()).unwrap();
    assert_eq!(s2.inserted, 0);
    let n: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM property WHERE external_key = 'mp:mp-13' AND kind = 'band_gap'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}
