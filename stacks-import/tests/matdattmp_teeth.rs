use std::fs;

use stacks_core::library::LibraryStore;
use stacks_core::Store;
use stacks_import::matdattmp::*;
use tempfile::TempDir;

fn workdir(tmp: &TempDir) -> std::path::PathBuf {
    let dir = tmp.path().join("work");
    fs::create_dir(&dir).unwrap();
    // raccuglia: one row with a fractional outcome score
    fs::write(
        dir.join("raccuglia_231.jsonl"),
        r#"{"XXXtitle":"T1","XXXinorg1":"potassium vanadium trioxide","XXXinorg1mass":"0.1549","XXXinorg1moles":"0.0011","XXXinorg2":"-1","XXXinorg3":"-1","XXXorg1":"1,3-diaminopropane","XXXorg1moles":"0.0091","XXXorg2":"-1","XXXoxlike1":"-1","XXXoxlike2":"-1","temp":"110","time":"24","slowCool":"yes","pH":"3","leak":"no","outcome":"0.37"}"#.to_string() + "\n",
    )
    .unwrap();
    fs::write(dir.join("raccuglia_232.jsonl"), "").unwrap();
    // alab
    fs::write(
        dir.join("alab.jsonl"),
        r#"{"Target":"Na3Ca18Fe(PO4)14","Result":"Success","Materials Project ID":"mp-725491","Precursors + Masses":"828.0 mg CaCO3 + 36.7 mg Fe2O3","Temperature (C)":"700","Duration (hours)":"4"}"#.to_string() + "\n",
    )
    .unwrap();
    fs::write(
        dir.join("alab_corrected.jsonl"),
        fs::read_to_string(dir.join("alab.jsonl")).unwrap(),
    )
    .unwrap();
    // zeosyn: one real row + one blank-separator row
    fs::write(
        dir.join("zeosyn.jsonl"),
        "{\"doi\":\"10.1/z\",\"cryst_time\":\"168\",\"cryst_temp\":\"175\",\"aging_time\":\"\",\"aging_temp\":\"\",\"ph\":\"\",\"osda1\":\"N-methylsparteinium\",\"osda1_smiles\":\"C[N+]12CCCC[C@@H]1[C@H]1C[C@@H](C2)[C@@H]2CCCCN2C1\",\"product1\":\"SSZ-24\",\"precursors\":\"tetraethylorthosilicate, HF\",\"percent_cryst\":\"85\",\"title\":\"SSZ-24 prep\"}\n{\"doi\":\"0\",\"product1\":\"\"}\n",
    )
    .unwrap();
    // gpss
    fs::write(
        dir.join("gpss.jsonl"),
        r#"{"sample_index":0,"composition":"Li2MgCl4","synthesis_temperature":450,"xrd_analysis_result":{"weight_fractions_of_each_phases":{"spinel (Fd-3m)":0.9616,"LiCl (Fm-3m)":0.0384}},"provenance":"human"}"#.to_string() + "\n",
    )
    .unwrap();
    // precursor genome
    fs::write(
        dir.join("precursor_genome.jsonl"),
        r#"{"sample_id":"PG_0001","target_compound":"Ag2Al","precursor_formulas":["Ag2O","Al(OH)3"],"heating_temperature":200.0,"heating_time":240.0,"furnace_name":"BFT_box_b","reaction_category":"partially_transformed","category_notes":"mixed phases"}"#.to_string() + "\n",
    )
    .unwrap();
    // mof
    fs::write(
        dir.join("mof.jsonl"),
        r#"{"name":"MOF-X","symbol":"9","M_precursor":[{"name":"NiCl2","composition":[["0.52","mmol1.0"]],"formula":"NiCl2"}],"O_precursor":[],"S_precursor":[{"name":"benzonitrile","composition":[["0.61","mL1.0"]]}],"temperature":{"Value":"453.15","Unit":"K1.0"},"time":[null,null],"operation":[{"name":"stir","condition":[{"Value":"0.5","Unit":"h1.0","Property":"Time"}]}],"property":{"yield":{"Value":"75","Unit":"%1.0"}},"doi":"10.1021/cg900315h","metadata":{}}"#.to_string() + "\n",
    )
    .unwrap();
    // ceder2: one new, one duplicate of existing
    fs::write(
        dir.join("ceder_ss.json"),
        r#"{"release_date":"2020-07-13","reactions":[{"synthesis_type":"solid-state","target":{"material_formula":"Li4Ti5O12"},"reaction_string":"2 Li2CO3 + 5 TiO2 == 1 Li4Ti5O12 + 2 CO2","doi":"10.1149/1.1383553","operations":[{"type":"HeatingOperation","token":"heated","conditions":{"heating_temperature":800.0,"heating_time":12.0}}],"reaction":{"left_side":[{"material":"Li2CO3","amount":"2"},{"material":"TiO2","amount":"5"}]}},{"synthesis_type":"solid-state","target":{"material_formula":"Fe2O3"},"reaction_string":"dup","doi":"10.dup","operations":[],"reaction":{"left_side":[]}}]}"#,
    )
    .unwrap();
    fs::write(
        dir.join("ceder_sg.json"),
        r#"{"release_date":"2020-07-13","reactions":[]}"#,
    )
    .unwrap();
    dir
}

#[test]
fn matdattmp_outcomes_and_idempotency() {
    let tmp = TempDir::new().unwrap();
    let work = workdir(&tmp);
    let mut store = Store::open(tmp.path().join("stacks.db")).unwrap();

    // Seed an "existing" ceder recipe that the 2020 dataset duplicates.
    let prov = stacks_core::Provenance {
        id: 0,
        kind: stacks_core::ProvenanceKind::DatasetImport,
        doi: Some("10.dup".to_string()),
        source_dataset: Some("ceder_solid_state".to_string()),
        path: None,
        sha256: None,
        locator: None,
        extractor_version: None,
        extraction_method: stacks_core::ExtractionMethod::Structured,
        confidence: 1.0,
        note: None,
    };
    let pid = store.insert_provenance(&prov).unwrap();
    let existing = stacks_core::Recipe {
        id: 0,
        name: "Fe2O3 old".to_string(),
        version: 1,
        status: stacks_core::RecipeStatus::Draft,
        target_material_id: None,
        target_quantity: None,
        synthesis_type: None,
        narrative: Some("dup".to_string()),
        created_from_recipe_id: None,
        provenance_id: Some(pid),
        created_at: "2026-01-01".to_string(),
        created_by: None,
        supersedes: None,
        external_key: Some("ceder_solid_state:ceder_solid_state_0".to_string()),
        outcome: None,
        outcome_score: None,
    };
    store.insert_recipe(&existing).unwrap();

    let s_rac = import_raccuglia(&mut store, &work).unwrap();
    assert_eq!(s_rac.inserted, 1);
    let s_alab = import_alab(&mut store, &work).unwrap();
    assert_eq!(s_alab.inserted, 2, "both A-Lab provenance versions");
    let s_zeo = import_zeosyn(&mut store, &work).unwrap();
    assert_eq!(s_zeo.inserted, 1);
    assert_eq!(s_zeo.rejected, 1, "blank separator row rejected");
    let s_gpss = import_gpss(&mut store, &work).unwrap();
    assert_eq!(s_gpss.inserted, 1);
    let s_pg = import_precursor_genome(&mut store, &work).unwrap();
    assert_eq!(s_pg.inserted, 1);
    let s_mof = import_mof(&mut store, &work).unwrap();
    assert_eq!(s_mof.inserted, 1);
    let s_ced = import_ceder2(&mut store, &work).unwrap();
    assert_eq!(s_ced.inserted, 1, "new 2020 record in");
    assert_eq!(s_ced.skipped_existing, 1, "content-identical dup skipped");

    // Fractional outcome score round-trips exactly through the store.
    let rid = store
        .recipe_id_by_external_key("raccuglia_dark_reactions:raccuglia_231:1")
        .unwrap()
        .unwrap();
    let bundle = store.load_recipe(rid).unwrap().unwrap();
    assert_eq!(bundle.recipe.outcome_score, Some(0.37));
    assert_eq!(bundle.recipe.outcome, None, "no boolean flattening");
    assert_eq!(
        bundle.steps[0].step.operation,
        stacks_core::Operation::Hydrothermal
    );
    assert_eq!(
        bundle.steps[0].step.conditions.ph.as_ref().unwrap().value,
        3.0
    );

    // A-Lab: quantified precursors parsed from "828.0 mg CaCO3".
    let rid = store
        .recipe_id_by_external_key("alab_moesm3:Na3Ca18Fe(PO4)14")
        .unwrap()
        .unwrap();
    let bundle = store.load_recipe(rid).unwrap().unwrap();
    assert_eq!(bundle.recipe.outcome, Some(stacks_core::Outcome::Success));
    let q = bundle.steps[0].materials[0].quantity.clone().unwrap();
    assert!((q.value - 0.828).abs() < 1e-9, "mg -> g conversion");

    // precursor-genome partial outcome category preserved.
    let rid = store
        .recipe_id_by_external_key("precursor_genome:PG_0001")
        .unwrap()
        .unwrap();
    assert_eq!(
        store.load_recipe(rid).unwrap().unwrap().recipe.outcome,
        Some(stacks_core::Outcome::Partial)
    );

    // MOF: K unit honored; yield -> outcome_score fraction.
    let rid = store
        .recipe_id_by_external_key("mof_synthesis_condition:1")
        .unwrap()
        .unwrap();
    let bundle = store.load_recipe(rid).unwrap().unwrap();
    assert_eq!(bundle.recipe.outcome_score, Some(0.75));

    // (b) idempotency: every importer re-run inserts 0.
    for f in [
        import_raccuglia as fn(&mut Store, &std::path::Path) -> _,
        import_alab,
        import_zeosyn,
        import_gpss,
        import_precursor_genome,
        import_mof,
        import_ceder2,
    ] {
        let s = f(&mut store, &work).unwrap();
        assert_eq!(s.inserted, 0, "re-run must insert nothing");
    }
}

#[test]
fn document_sha256_does_not_collide_with_paper() {
    let tmp = TempDir::new().unwrap();
    let mut lib = LibraryStore::open(tmp.path().join("library.db")).unwrap();

    // A paper row with the same sha256 already exists.
    let sha = "f".repeat(64);
    stacks_core::library::insert_paper(
        lib.raw(),
        &stacks_core::library::LibraryPaper {
            sha256: sha.clone(),
            filename: "x.pdf".to_string(),
            path: None,
            size_bytes: None,
            registered_at: None,
            on_disk: None,
            doi: None,
            arxiv_id: None,
            title: Some("existing paper".to_string()),
            authors: None,
            year: None,
            abstract_: None,
            journal: None,
            source_url: None,
            access: None,
            blob_key: None,
            blob_synced_at: None,
            subfield: None,
            tags: None,
            original_language: None,
            original_script_title: None,
            transliterated_title: None,
            translation_of: None,
            translated_in: None,
            soviet_stratum: None,
            source_collection: None,
        },
    )
    .unwrap();

    let manifest = tmp.path().join("manifest.jsonl");
    fs::write(
        &manifest,
        format!(
            r#"{{"title":"A Soviet Handbook","authors":"X","year":"1974","source_url":"u","download_url":"d","path":"acquisitions/government/x.pdf","language":"English","bytes":100,"sha256":"{sha}","retrieved_utc":"2026-09-22","ledger":"ledgers/government.jsonl","pages":10}}"#
        ) + "\n",
    )
    .unwrap();
    let stats = import_documents(&mut lib, &manifest, tmp.path()).unwrap();
    assert_eq!(stats.inserted, 1);

    // Both rows coexist; the paper row is untouched.
    let paper_title: String = lib
        .raw()
        .query_row("SELECT title FROM paper WHERE sha256 = ?1", [&sha], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(paper_title, "existing paper");
    let (kind, root): (String, String) = lib
        .raw()
        .query_row(
            "SELECT kind, location_root FROM document WHERE sha256 = ?1",
            [&sha],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(kind, "report");
    assert!(!root.is_empty());

    // Idempotent re-run.
    let stats2 = import_documents(&mut lib, &manifest, tmp.path()).unwrap();
    assert_eq!(stats2.inserted, 0);
    assert_eq!(stats2.skipped_existing, 1);
}
