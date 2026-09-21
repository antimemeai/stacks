use std::fs;
use std::path::Path;

use stacks_core::Store;
use stacks_import::{import_inorganic, import_ord};
use tempfile::TempDir;

fn write_fixture(dir: &Path) {
    fs::write(
        dir.join("recipes.jsonl"),
        concat!(
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","synthesis_type":"solid-state","doi":"10.1/x","target_formula":"SrFe12O19","target_name":"Strontium hexaferrite","reaction_string":"6 Fe2O3 + 1 SrCO3 == 1 SrFe12O19","temperature_min":1000.0,"temperature_max":1200.0,"time_min":null,"time_max":null,"atmosphere":"air","has_atmosphere_info":true,"mp_id":"mp-19538"}"#,
            "\n",
            r#"{"recipe_id":"lee_impurity_phase_0","source":"lee_impurity_phase","synthesis_type":"solid-state","doi":"","target_formula":"Ca3Co4O9","target_name":null,"reaction_string":null,"temperature_min":null,"temperature_max":null,"time_min":null,"time_max":null,"atmosphere":null,"has_atmosphere_info":false,"mp_id":"mp-1096877"}"#,
            "\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("precursors.jsonl"),
        concat!(
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","role":"precursor","formula":"Fe2O3","name":"ferric oxide","amount":null}"#,
            "\n",
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","role":"precursor","formula":"SrCO3","name":null,"amount":1.0}"#,
            "\n",
            r#"{"recipe_id":"lee_impurity_phase_0","source":"lee_impurity_phase","role":"precursor","formula":"CaCO3","name":null,"amount":null}"#,
            "\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("operations.jsonl"),
        concat!(
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","step_index":0,"action_type":"Mixing","token":"mixed","temperature_c":null,"time_value":null,"time_units":null,"atmosphere":null,"mixing_device":null,"mixing_media":null}"#,
            "\n",
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","step_index":1,"action_type":"HeatingOperation","token":"calcined","temperature_c":1100.0,"time_value":2.0,"time_units":"hrs","atmosphere":null,"mixing_device":null,"mixing_media":null}"#,
            "\n",
            r#"{"recipe_id":"ceder_solid_state_0","source":"ceder_solid_state","step_index":2,"action_type":"ShapingOperation","token":"pressed","temperature_c":null,"time_value":null,"time_units":null,"atmosphere":null,"mixing_device":null,"mixing_media":null}"#,
            "\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("ord.jsonl"),
        concat!(
            r#"{"reaction_id":"ord-aaa","dataset_id":"ds1","synthesis_type":"organic","reaction_smiles":"CCO>>CC=O","reactants_smiles_json":"[\"CCO\"]","reagents_smiles_json":"[]","solvents_smiles_json":"[\"O\"]","catalysts_smiles_json":"[]","products_smiles_json":"[\"CC=O\"]","temperature_c":80.0,"time_h":3.0,"atmosphere":null,"doi":"10.2/y"}"#,
            "\n",
            r#"{"reaction_id":"ord-bbb","dataset_id":"ds1","synthesis_type":"organic","reaction_smiles":"","reactants_smiles_json":"[\"CCN\"]","reagents_smiles_json":"[]","solvents_smiles_json":"[\"O\"]","catalysts_smiles_json":"[]","products_smiles_json":"[\"CC=O\"]","temperature_c":null,"time_h":null,"atmosphere":null,"doi":""}"#,
            "\n"
        ),
    )
    .unwrap();
}

fn count(store: &Store, table: &str) -> i64 {
    store
        .raw()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn import_both(store: &mut Store, dir: &Path) {
    import_inorganic(store, dir).unwrap();
    import_ord(store, dir).unwrap();
}

#[test]
fn import_is_idempotent_and_dedupes_materials() {
    let tmp = TempDir::new().unwrap();
    write_fixture(tmp.path());
    let db = tmp.path().join("test.db");
    let mut store = Store::open(&db).unwrap();

    import_both(&mut store, tmp.path());
    let before = (
        count(&store, "recipe"),
        count(&store, "material"),
        count(&store, "recipe_step"),
        count(&store, "step_material"),
        count(&store, "provenance"),
    );
    assert_eq!(before.0, 4, "2 inorganic + 2 ORD recipes");

    // Materials: SrFe12O19, Fe2O3, SrCO3, Ca3Co4O9, CaCO3 (inorganic) +
    // CCO, O, CC=O (ord-aaa); ord-bbb adds CCN only (O and CC=O dedupe).
    assert_eq!(before.1, 9, "shared materials must be deduped");

    import_both(&mut store, tmp.path());
    let after = (
        count(&store, "recipe"),
        count(&store, "material"),
        count(&store, "recipe_step"),
        count(&store, "step_material"),
        count(&store, "provenance"),
    );
    assert_eq!(before, after, "re-running the importer must not add rows");
}

#[test]
fn imported_invariants_hold() {
    let tmp = TempDir::new().unwrap();
    write_fixture(tmp.path());
    let db = tmp.path().join("test.db");
    let mut store = Store::open(&db).unwrap();
    import_both(&mut store, tmp.path());

    // Formula-kind materials always carry a formula.
    let bad: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM material WHERE kind = 'formula' AND (formula IS NULL OR formula = '')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);

    // Every imported recipe has a provenance row and an external key.
    let bad: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM recipe r LEFT JOIN provenance p ON p.id = r.provenance_id
             WHERE r.provenance_id IS NULL OR p.id IS NULL OR r.external_key IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0);

    // Operation mapping: calcined → calcine, mixed → mix, pressed → other.
    let ops: Vec<String> = {
        let mut stmt = store
            .raw()
            .prepare("SELECT operation FROM recipe_step ORDER BY ordering")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert!(ops.contains(&"calcine".to_string()));
    assert!(ops.contains(&"mix".to_string()));
    assert!(ops.contains(&"other:pressed".to_string()));

    // Atmosphere: air mapped on the ceder recipe; absent (NULL) on lee.
    let cond: String = store
        .raw()
        .query_row(
            "SELECT rs.conditions_json FROM recipe_step rs
             JOIN recipe r ON r.id = rs.recipe_id
             WHERE r.external_key = 'ceder_solid_state:ceder_solid_state_0' AND rs.ordering = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(cond.contains("\"atmosphere\":\"air\""), "{cond}");

    // Ragged duration unit "hrs" normalized to h on step 2.
    assert!(cond.contains("air"));
    let cond2: String = store
        .raw()
        .query_row(
            "SELECT rs.conditions_json FROM recipe_step rs
             JOIN recipe r ON r.id = rs.recipe_id
             WHERE r.external_key = 'ceder_solid_state:ceder_solid_state_0' AND rs.ordering = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(cond2.contains("\"unit\":\"h\""), "{cond2}");
    assert!(cond2.contains("1100"), "{cond2}");

    // Recipe-level temperature range lands as MinMax on step 1 (which has no
    // temperature of its own).
    assert!(cond.contains("\"kind\":\"min_max\""), "{cond}");
}
