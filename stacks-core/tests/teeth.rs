use stacks_core::*;
use tempfile::NamedTempFile;

fn temp_store() -> (NamedTempFile, Store) {
    let file = NamedTempFile::new().expect("temp db file");
    let store = Store::open(file.path()).expect("open store");
    (file, store)
}

// (a) Type boundary: an unknown unit string must be unrepresentable.
#[test]
fn unknown_unit_rejected_at_type_boundary() {
    let json = r#"{"value": 1.0, "unit": "furlong", "operator": "eq"}"#;
    let result = serde_json::from_str::<Quantity>(json);
    assert!(
        result.is_err(),
        "unknown unit must fail deserialization, got {result:?}"
    );

    let json = r#"{"value": 1.0, "unit": "g"}"#;
    let bare = serde_json::from_str::<serde_json::Value>(json).unwrap();
    assert!(bare.get("unit").is_some());
}

// (a2) Type boundary companion: a quantity with no unit at all fails too —
// there is no path from a bare float to a stored quantity.
#[test]
fn bare_float_quantity_rejected() {
    let result = serde_json::from_str::<Quantity>("1.5");
    assert!(result.is_err());
    let result = serde_json::from_str::<Quantity>(r#"{"value": 1.5}"#);
    assert!(result.is_err(), "missing unit must fail, got {result:?}");
}

// (b) DB boundary: raw SQL bypassing the Rust types must hit the CHECK fence.
#[test]
fn check_constraint_rejects_unknown_unit_via_raw_sql() {
    let (_f, store) = temp_store();
    let err = store.raw().execute(
        "INSERT INTO step_material (step_id, material_id, role, value, unit, operator)
         VALUES (1, 1, 'reactant', 1.0, 'furlong', 'eq')",
        [],
    );
    assert!(
        err.is_err(),
        "unknown unit via raw SQL must violate CHECK, got {err:?}"
    );

    let err = store.raw().execute(
        "INSERT INTO provenance (kind, extraction_method, confidence)
         VALUES ('file', 'structured', 1.7)",
        [],
    );
    assert!(
        err.is_err(),
        "confidence outside 0..=1 must violate CHECK, got {err:?}"
    );

    let err = store.raw().execute(
        "INSERT INTO recipe (name, version, status, created_at)
         VALUES ('x', 1, 'limbo', '2026-09-21T00:00:00Z')",
        [],
    );
    assert!(
        err.is_err(),
        "unknown status must violate CHECK, got {err:?}"
    );

    // Range operator without bounds must violate the paired CHECK.
    let err = store.raw().execute(
        "INSERT INTO step_material (step_id, material_id, role, value, unit, operator)
         VALUES (1, 1, 'reactant', 100.0, '°C', 'range')",
        [],
    );
    assert!(
        err.is_err(),
        "range without bounds must violate CHECK, got {err:?}"
    );
}

// (c) Round-trip: every model type serializes to JSON and back identically.
#[test]
fn json_round_trip_all_model_types() {
    fn round_trip<T>(value: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(*value, back, "round-trip mismatch for {json}");
    }

    round_trip(&Quantity::exact(2.5, Unit::Gram));
    round_trip(&Quantity {
        value: 100.0,
        unit: Unit::Celsius,
        operator: Operator::Range {
            min: 80.0,
            max: 120.0,
        },
    });
    round_trip(&Quantity {
        value: 1.0,
        unit: Unit::Other("scoop".to_string()),
        operator: Operator::Approx,
    });
    round_trip(&Operator::Lt);
    round_trip(&Operator::Range { min: 0.0, max: 1.0 });

    round_trip(&Material {
        id: 7,
        kind: MaterialKind::Formula,
        inchikey: None,
        canonical_smiles: None,
        formula: Some("Fe2O3".to_string()),
        composition: None,
        names: vec!["hematite".to_string(), "iron(III) oxide".to_string()],
        cas: Some("1309-37-1".to_string()),
        identity: None,
    });
    round_trip(&Material {
        id: 0,
        kind: MaterialKind::Other("doped ceramic".to_string()),
        inchikey: None,
        canonical_smiles: None,
        formula: None,
        composition: Some(serde_json::json!({"BaTiO3": 0.9, "CaTiO3": 0.1})),
        names: vec![],
        cas: None,
        identity: None,
    });

    round_trip(&Provenance {
        id: 3,
        kind: ProvenanceKind::File,
        doi: None,
        source_dataset: Some("sciencemadness".to_string()),
        path: Some("/corpus/brauer.pdf".to_string()),
        sha256: Some("ab".repeat(32)),
        locator: Some("p. 1213".to_string()),
        extractor_version: Some("0.1.0".to_string()),
        extraction_method: ExtractionMethod::LlmExtracted,
        confidence: 0.72,
        note: None,
    });

    round_trip(&Recipe {
        id: 11,
        name: "YBCO".to_string(),
        version: 2,
        status: RecipeStatus::Deployed,
        target_material_id: Some(7),
        target_quantity: Some(Quantity::exact(10.0, Unit::Gram)),
        synthesis_type: Some(SynthesisType::SolidState),
        narrative: Some("classic solid-state route".to_string()),
        created_from_recipe_id: Some(10),
        provenance_id: Some(3),
        created_at: "2026-09-21T00:00:00Z".to_string(),
        created_by: Some("patrick".to_string()),
        supersedes: Some(10),
        external_key: None,
        outcome: None,
        outcome_score: None,
    });

    round_trip(&RecipeStep {
        id: 21,
        recipe_id: 11,
        ordering: 1,
        operation: Operation::Calcine,
        parameters: serde_json::json!({"ramp_rate": {"value": 5, "unit": "K/min"}}),
        conditions: Conditions {
            temperature: Some(Temperature::Series {
                points: vec![
                    TemperaturePoint {
                        time: Quantity::exact(0.0, Unit::Hour),
                        temperature: Quantity::exact(25.0, Unit::Celsius),
                    },
                    TemperaturePoint {
                        time: Quantity::exact(12.0, Unit::Hour),
                        temperature: Quantity::exact(950.0, Unit::Celsius),
                    },
                ],
            }),
            pressure: Some(Quantity::exact(1.0, Unit::Atm)),
            duration: Some(Quantity {
                value: 12.0,
                unit: Unit::Hour,
                operator: Operator::Approx,
            }),
            atmosphere: Some(Atmosphere::O2),
            ph: None,
            stirring: None,
            atmosphere_note: Some("flowing".to_string()),
        },
    });

    round_trip(&StepMaterial {
        id: 31,
        step_id: 21,
        material_id: 7,
        role: MaterialRole::Precursor,
        quantity: Some(Quantity::exact(4.0, Unit::Gram)),
        equivalents: Some(1.0),
        is_reference: true,
        optional: false,
        notes: Some("grind before use".to_string()),
    });
    round_trip(&StepMaterial {
        id: 32,
        step_id: 21,
        material_id: 8,
        role: MaterialRole::Other("gettering agent".to_string()),
        quantity: None,
        equivalents: None,
        is_reference: false,
        optional: true,
        notes: None,
    });

    round_trip(&Run {
        id: 41,
        recipe_id: 11,
        recipe_version: 2,
        started_at: Some("2026-09-21T08:00:00Z".to_string()),
        operator: Some("patrick".to_string()),
        yield_quantity: Some(Quantity::exact(8.7, Unit::Gram)),
        conversion: Some(0.95),
        purity: Some(0.99),
        observation: Some("black, dense pellet".to_string()),
    });

    round_trip(&RunStep {
        id: 51,
        run_id: 41,
        recipe_step_id: Some(21),
        ordering: 1,
        operation: Operation::Other {
            note: "intermediate regrind".to_string(),
        },
        actual_parameters: serde_json::json!({}),
        actual_conditions: Conditions {
            ph: Some(PhCondition {
                operator: Operator::Lt,
                value: 2.0,
            }),
            ..Conditions::default()
        },
        started_at: Some("2026-09-21T08:05:00Z".to_string()),
        ended_at: Some("2026-09-21T20:05:00Z".to_string()),
        deviation_notes: Some("held 1 h extra at 950 C".to_string()),
    });

    round_trip(&ChangeLogEntry {
        id: 61,
        entity_kind: "recipe".to_string(),
        entity_id: 11,
        at: "2026-09-21T09:00:00Z".to_string(),
        by: "patrick".to_string(),
        patch: serde_json::json!({"status": {"from": "draft", "to": "deployed"}}),
    });
}

// (c2) Schema contract: JSON Schemas generate from the Rust types.
#[test]
fn json_schemas_generate() {
    let schema = schemars::schema_for!(Quantity);
    let json = serde_json::to_value(&schema).unwrap();
    let unit_schema = &json["definitions"]["Unit"];
    assert!(
        unit_schema.to_string().contains("\"g\""),
        "unit schema must enumerate closed tokens: {unit_schema}"
    );
    let _ = schemars::schema_for!(Recipe);
    let _ = schemars::schema_for!(RecipeStep);
    let _ = schemars::schema_for!(Conditions);
}

// (d) Migrations: apply cleanly to a fresh DB; re-running is a predictable
// no-op.
#[test]
fn migrations_fresh_and_idempotent() {
    let (_f, store) = temp_store();
    assert_eq!(store.schema_version().unwrap(), 5);

    let tables_before: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    store.migrate().unwrap();
    store.migrate().unwrap();
    assert_eq!(store.schema_version().unwrap(), 5);

    let tables_after: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables_before, tables_after);

    let journal_mode: String = store
        .raw()
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(journal_mode, "wal");

    let fk_on: i64 = store
        .raw()
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk_on, 1);
}

// (e) Full recipe: insert and read back with every field intact.
#[test]
fn full_recipe_round_trip_through_storage() {
    let (_f, store) = temp_store();

    let target_material = Material {
        id: 0,
        kind: MaterialKind::Formula,
        inchikey: None,
        canonical_smiles: None,
        formula: Some("YBa2Cu3O7".to_string()),
        composition: None,
        names: vec![
            "YBCO".to_string(),
            "yttrium barium copper oxide".to_string(),
        ],
        cas: None,
        identity: None,
    };
    let solvent_material = Material {
        id: 0,
        kind: MaterialKind::Molecule,
        inchikey: Some("XLYOFNOQVPJJNP-UHFFFAOYSA-N".to_string()),
        canonical_smiles: Some("O".to_string()),
        formula: None,
        composition: None,
        names: vec!["water".to_string()],
        cas: Some("7732-18-5".to_string()),
        identity: None,
    };
    let target_id = store.insert_material(&target_material).unwrap();
    let solvent_id = store.insert_material(&solvent_material).unwrap();

    let provenance = Provenance {
        id: 0,
        kind: ProvenanceKind::DatasetImport,
        doi: Some("10.0000/example".to_string()),
        source_dataset: Some("materials_project".to_string()),
        path: None,
        sha256: None,
        locator: None,
        extractor_version: Some("chem-recipes/1.4.2".to_string()),
        extraction_method: ExtractionMethod::Structured,
        confidence: 1.0,
        note: None,
    };
    let provenance_id = store.insert_provenance(&provenance).unwrap();

    let recipe = Recipe {
        id: 0,
        name: "YBCO solid-state".to_string(),
        version: 1,
        status: RecipeStatus::Draft,
        target_material_id: Some(target_id),
        target_quantity: Some(Quantity {
            value: 10.0,
            unit: Unit::Gram,
            operator: Operator::Approx,
        }),
        synthesis_type: Some(SynthesisType::SolidState),
        narrative: Some("Mix, calcine, regrind, sinter.".to_string()),
        created_from_recipe_id: None,
        provenance_id: Some(provenance_id),
        created_at: "2026-09-21T10:00:00Z".to_string(),
        created_by: Some("stacks-import".to_string()),
        supersedes: None,
        external_key: None,
        outcome: None,
        outcome_score: None,
    };
    let recipe_id = store.insert_recipe(&recipe).unwrap();

    let step = RecipeStep {
        id: 0,
        recipe_id,
        ordering: 1,
        operation: Operation::Calcine,
        parameters: serde_json::json!({"ramp": {"value": 5, "unit": "other:K/min"}}),
        conditions: Conditions {
            temperature: Some(Temperature::MinMax {
                min: Quantity::exact(900.0, Unit::Celsius),
                max: Quantity::exact(950.0, Unit::Celsius),
            }),
            pressure: None,
            duration: Some(Quantity::exact(12.0, Unit::Hour)),
            atmosphere: Some(Atmosphere::O2),
            ph: None,
            stirring: None,
            atmosphere_note: Some("flowing O2".to_string()),
        },
    };
    let step_id = store.insert_step(&step).unwrap();

    let step_material = StepMaterial {
        id: 0,
        step_id,
        material_id: solvent_id,
        role: MaterialRole::Wash,
        quantity: Some(Quantity {
            value: 50.0,
            unit: Unit::Milliliter,
            operator: Operator::Range {
                min: 40.0,
                max: 60.0,
            },
        }),
        equivalents: None,
        is_reference: false,
        optional: true,
        notes: Some("DI water".to_string()),
    };
    store.insert_step_material(&step_material).unwrap();

    let bundle = store
        .load_recipe(recipe_id)
        .unwrap()
        .expect("recipe must load");

    let mut expected_recipe = recipe.clone();
    expected_recipe.id = recipe_id;
    assert_eq!(bundle.recipe, expected_recipe);

    let mut expected_provenance = provenance.clone();
    expected_provenance.id = provenance_id;
    assert_eq!(bundle.provenance, Some(expected_provenance));

    assert_eq!(bundle.steps.len(), 1);
    let mut expected_step = step.clone();
    expected_step.id = step_id;
    assert_eq!(bundle.steps[0].step, expected_step);

    assert_eq!(bundle.steps[0].materials.len(), 1);
    let mut expected_sm = step_material.clone();
    expected_sm.id = bundle.steps[0].materials[0].id;
    assert_eq!(bundle.steps[0].materials[0], expected_sm);
}

// (E) Outcome fields: fractional scores preserved exactly, categorical
// outcome CHECK'd, hydrothermal token round-trips.
#[test]
fn outcome_score_and_hydrothermal_round_trip() {
    let (_f, store) = temp_store();

    let recipe = Recipe {
        id: 0,
        name: "dark reaction fixture".to_string(),
        version: 1,
        status: RecipeStatus::Draft,
        target_material_id: None,
        target_quantity: None,
        synthesis_type: Some(SynthesisType::SolutionBased),
        narrative: None,
        created_from_recipe_id: None,
        provenance_id: None,
        created_at: "2026-09-22T00:00:00Z".to_string(),
        created_by: None,
        supersedes: None,
        external_key: Some("fixture:dark-1".to_string()),
        outcome: Some(Outcome::Partial),
        outcome_score: Some(0.37),
    };
    let rid = store.insert_recipe(&recipe).unwrap();
    let step = RecipeStep {
        id: 0,
        recipe_id: rid,
        ordering: 1,
        operation: Operation::Hydrothermal,
        parameters: serde_json::json!({}),
        conditions: Conditions {
            temperature: Some(Temperature::Scalar(Quantity::exact(180.0, Unit::Celsius))),
            duration: Some(Quantity::exact(24.0, Unit::Hour)),
            ..Conditions::default()
        },
    };
    store.insert_step(&step).unwrap();

    let bundle = store.load_recipe(rid).unwrap().unwrap();
    assert_eq!(bundle.recipe.outcome, Some(Outcome::Partial));
    assert_eq!(
        bundle.recipe.outcome_score,
        Some(0.37),
        "fractional score must round-trip exactly"
    );
    assert_eq!(bundle.steps[0].step.operation, Operation::Hydrothermal);

    // DB fence: invalid outcome token rejected via CHECK.
    let err = store.raw().execute(
        "INSERT INTO recipe (name, version, status, created_at, outcome)
         VALUES ('x', 1, 'draft', 'now', 'sorta-worked')",
        [],
    );
    assert!(err.is_err(), "invalid outcome must violate CHECK: {err:?}");

    // 'hydrothermal' is now inside the CHECK list (migration 5 rebuild).
    store
        .raw()
        .execute(
            "INSERT INTO recipe_step (recipe_id, ordering, operation) VALUES (?1, 9, 'hydrothermal')",
            [rid],
        )
        .unwrap();
    let err = store.raw().execute(
        "INSERT INTO recipe_step (recipe_id, ordering, operation) VALUES (?1, 10, 'autoclave')",
        [rid],
    );
    assert!(err.is_err(), "non-token must still violate CHECK: {err:?}");
}
