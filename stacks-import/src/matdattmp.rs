//! matdattmp acquisition-bay import wave (Stage E). One importer per source,
//! sharing the batch/external-key idempotency scaffolding. Source root is
//! read-only; exports live in data/matdattmp/.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;
use stacks_core::library::{LibraryDocument, LibraryStore};
use stacks_core::store;
use stacks_core::{
    Conditions, ExtractionMethod, PhCondition, Provenance, ProvenanceKind, SynthesisType,
    Temperature,
};
use stacks_core::{
    Material, MaterialKind, MaterialRole, Operation, Operator, Outcome, Quantity, Recipe,
    RecipeStatus, RecipeStep, StepMaterial, Unit,
};
use stacks_core::{Store, StoreError};

use crate::ImportError;

const CREATED_AT: &str = "2026-09-22T00:00:00Z";
const BATCH: usize = 5_000;

#[derive(Debug, Default, serde::Serialize)]
pub struct SourceStat {
    pub seen: u64,
    pub inserted: u64,
    pub skipped_existing: u64,
    pub rejected: u64,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub unmapped_tokens: std::collections::BTreeMap<String, u64>,
}

fn read_lines(path: &Path) -> Result<std::io::Lines<BufReader<File>>, ImportError> {
    Ok(BufReader::with_capacity(1 << 22, File::open(path)?).lines())
}

/// Byte-line iterator with lossy UTF-8 decoding (the sol-gel export has
/// stray non-UTF-8 bytes; ~0.01% of lines, decoded with replacement chars).
struct LossyLines {
    inner: std::io::Split<BufReader<File>>,
}

impl LossyLines {
    fn next_line(&mut self) -> Result<Option<String>, ImportError> {
        match self.inner.next() {
            None => Ok(None),
            Some(bytes) => Ok(Some(String::from_utf8_lossy(&bytes?).into_owned())),
        }
    }
}

fn read_lines_lossy(path: &Path) -> Result<LossyLines, ImportError> {
    Ok(LossyLines {
        inner: BufReader::with_capacity(1 << 22, File::open(path)?).split(b'\n'),
    })
}

/// Dataset-import provenance with optional note (license_status etc.).
fn prov(
    source_dataset: &str,
    doi: Option<String>,
    locator: Option<String>,
    note: Option<String>,
) -> Provenance {
    Provenance {
        id: 0,
        kind: ProvenanceKind::DatasetImport,
        doi,
        source_dataset: Some(source_dataset.to_string()),
        path: None,
        sha256: None,
        locator,
        extractor_version: Some("matdattmp-wave-1".to_string()),
        extraction_method: ExtractionMethod::Structured,
        confidence: 1.0,
        note,
    }
}

/// Name-only material (identity `name:<lowercase>`), for sources that name
/// reagents without formulas.
pub fn name_material(name: &str) -> Material {
    let name = name.trim();
    Material {
        id: 0,
        kind: MaterialKind::Other("name-only".to_string()),
        inchikey: None,
        canonical_smiles: None,
        formula: None,
        composition: None,
        names: vec![name.to_string()],
        cas: None,
        identity: Some(format!("name:{}", name.to_lowercase())),
    }
}

fn base_recipe(
    name: String,
    external_key: String,
    provenance_id: i64,
    synthesis_type: Option<SynthesisType>,
) -> Recipe {
    Recipe {
        id: 0,
        name,
        version: 1,
        status: RecipeStatus::Draft,
        target_material_id: None,
        target_quantity: None,
        synthesis_type,
        narrative: None,
        created_from_recipe_id: None,
        provenance_id: Some(provenance_id),
        created_at: CREATED_AT.to_string(),
        created_by: Some("stacks-import".to_string()),
        supersedes: None,
        external_key: Some(external_key),
        outcome: None,
        outcome_score: None,
    }
}

fn blank(s: &str) -> bool {
    s.trim().is_empty() || s.trim() == "-1"
}

fn parse_f(s: &str) -> Option<f64> {
    let t = s.trim();
    if blank(t) {
        return None;
    }
    t.parse().ok()
}

// ---------- Part A: documents ----------

#[derive(Debug, Deserialize)]
struct ManifestRow {
    title: Option<String>,
    authors: Option<serde_json::Value>,
    year: Option<serde_json::Value>,
    source_url: Option<String>,
    download_url: Option<String>,
    path: String,
    page_count: Option<serde_json::Value>,
    pages: Option<i64>,
    language: Option<String>,
    bytes: Option<i64>,
    sha256: String,
    retrieved_utc: Option<String>,
    ledger: String,
}

fn doc_kind(family: &str, path: &str) -> &'static str {
    let p = path.to_lowercase();
    if p.contains("patent") {
        return "patent";
    }
    if p.contains("thes") {
        return "thesis";
    }
    match family {
        "government" => "report",
        "modern" | "resumed" | "range-retries" => "paper",
        _ => "book",
    }
}

/// Import the matdattmp manifest as document rows (payloads referenced in
/// place; location_root records the absolute root).
pub fn import_documents(
    store: &mut LibraryStore,
    manifest: &Path,
    root: &Path,
) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let root_str = root.to_string_lossy().to_string();
    let mut batch: Vec<LibraryDocument> = Vec::with_capacity(1024);
    for line in read_lines(manifest)? {
        let row: ManifestRow = serde_json::from_str(&line?)?;
        stats.seen += 1;
        let family = row
            .ledger
            .trim_start_matches("ledgers/")
            .trim_end_matches(".jsonl")
            .to_string();
        // companion text layer: <file>_djvu.txt or <file>.txt beside payload
        let payload = root.join(&row.path);
        let stem = payload.with_extension("");
        let text_layer = [
            payload.with_extension("").to_string_lossy().to_string() + "_djvu.txt",
            stem.with_extension("txt").to_string_lossy().to_string(),
        ]
        .into_iter()
        .find(|c| std::path::Path::new(c).exists());
        let pages = row.pages.or_else(|| {
            row.page_count.as_ref().and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            })
        });
        let doc = LibraryDocument {
            sha256: row.sha256,
            family: family.clone(),
            kind: doc_kind(&family, &row.path).to_string(),
            title: row.title,
            authors: row.authors.and_then(|v| match v {
                serde_json::Value::String(s) => Some(s),
                serde_json::Value::Array(a) => Some(
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                ),
                _ => None,
            }),
            year: row.year.and_then(|y| match y {
                serde_json::Value::String(s) => s.trim().parse().ok(),
                serde_json::Value::Number(n) => n.as_i64(),
                _ => None,
            }),
            language: row.language,
            pages,
            source_url: row.source_url,
            download_url: row.download_url,
            path: row.path,
            location_root: root_str.clone(),
            bytes: row.bytes,
            retrieved_at: row.retrieved_utc,
            text_layer_path: text_layer,
            collection: None,
            original_language: None,
            transliterated_title: None,
            soviet_stratum: None,
        };
        batch.push(doc);
        if batch.len() >= 1024 {
            let b = std::mem::take(&mut batch);
            let inserted = store.with_transaction(|conn| {
                let mut n = 0;
                for d in &b {
                    if stacks_core::library::insert_document(conn, d)? {
                        n += 1;
                    }
                }
                Ok::<_, StoreError>(n)
            })?;
            stats.inserted += inserted;
        }
    }
    if !batch.is_empty() {
        let inserted = store.with_transaction(|conn| {
            let mut n = 0;
            for d in &batch {
                if stacks_core::library::insert_document(conn, d)? {
                    n += 1;
                }
            }
            Ok::<_, StoreError>(n)
        })?;
        stats.inserted += inserted;
    }
    stats.skipped_existing = stats.seen - stats.inserted;
    Ok(stats)
}

// ---------- B1: Raccuglia dark reactions ----------

#[derive(Debug, Deserialize)]
struct RaccugliaRow {
    #[serde(rename = "XXXtitle")]
    title: String,
    #[serde(flatten)]
    rest: HashMap<String, String>,
}

/// Import Raccuglia dark-reaction CSVs. Outcome is real-valued and preserved
/// in `outcome_score` exactly; no categorical flattening.
pub fn import_raccuglia(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut line_no = 0u64;
    for file in ["raccuglia_231.jsonl", "raccuglia_232.jsonl"] {
        let mut stream = read_lines(&workdir.join(file))?;
        loop {
            let n = store.with_transaction(|conn| {
                let mut n = 0;
                while n < BATCH {
                    let Some(line) = stream.next() else { break };
                    let row: RaccugliaRow = serde_json::from_str(&line?)?;
                    n += 1;
                    line_no += 1;
                    // Titles repeat within a file (46 dupes): the key is the
                    // line index, the title stays in the name.
                    let key = format!(
                        "raccuglia_dark_reactions:{}:{line_no}",
                        file.trim_end_matches(".jsonl"),
                    );
                    if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                        stats.skipped_existing += 1;
                        continue;
                    }
                    let doi = "10.1038/nature17439".to_string();
                    let pid = store::insert_provenance(conn, &prov(
                        "raccuglia_dark_reactions",
                        Some(doi),
                        Some(format!("{file}:{}", row.title)),
                        Some("dark reactions; real-valued outcome scores preserved".to_string()),
                    ))?;
                    let mut r = base_recipe(
                        format!("dark reaction {} ({}#{line_no})", row.title, file),
                        key,
                        pid,
                        Some(SynthesisType::SolutionBased),
                    );
                    r.outcome_score = parse_f(row.rest.get("outcome").map(String::as_str).unwrap_or(""))
                        .or_else(|| parse_f(row.rest.get("outcome (actual)").map(String::as_str).unwrap_or("")));
                    let recipe_id = store::insert_recipe(conn, &r)?;

                    let temp = parse_f(row.rest.get("temp").map(String::as_str).unwrap_or(""));
                    let time = parse_f(row.rest.get("time").map(String::as_str).unwrap_or(""));
                    let ph = parse_f(row.rest.get("pH").map(String::as_str).unwrap_or(""));
                    let slow = row.rest.get("slowCool").map(String::as_str) == Some("yes");
                    let step = RecipeStep {
                        id: 0,
                        recipe_id,
                        ordering: 1,
                        operation: Operation::Hydrothermal,
                        parameters: serde_json::json!({"slow_cool": slow, "leak": row.rest.get("leak")}),
                        conditions: Conditions {
                            temperature: temp.map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                            duration: time.map(|t| Quantity::exact(t, Unit::Hour)),
                            ph: ph.map(|v| PhCondition { operator: Operator::Eq, value: v }),
                            ..Conditions::default()
                        },
                    };
                    let step_id = store::insert_step(conn, &step)?;

                    for prefix in ["inorg1", "inorg2", "inorg3", "org1", "org2", "oxlike1", "oxlike2"] {
                        let name = row.rest.get(&format!("XXX{prefix}")).map(String::as_str).unwrap_or("");
                        if blank(name) {
                            continue;
                        }
                        let moles = parse_f(row.rest.get(&format!("XXX{prefix}moles")).map(String::as_str).unwrap_or(""));
                        let m = name_material(name);
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        let role = if prefix.starts_with("inorg") {
                            MaterialRole::Precursor
                        } else {
                            MaterialRole::Reactant
                        };
                        store::insert_step_material(conn, &StepMaterial {
                            id: 0,
                            step_id,
                            material_id: mid,
                            role,
                            quantity: moles.map(|v| Quantity::exact(v, Unit::Mol)),
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        })?;
                    }
                    stats.inserted += 1;
                }
                Ok::<_, ImportError>(n)
            })?;
            if n == 0 {
                break;
            }
        }
    }
    Ok(stats)
}

// ---------- B2: RAPID perovskites ----------

/// Import the RAPID perovskite campaign. Crystal scores preserved in
/// `outcome_score`. Per-row organic identity via inchikey; inorganic/acid
/// are campaign stock solutions (identity limitation documented).
pub fn import_rapid(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines(&workdir.join("rapid.jsonl"))?;
    let mut line_no = 0u64;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next() else { break };
                let row: HashMap<String, Option<String>> = serde_json::from_str(&line?)?;
                n += 1;
                let vial = row.get("RunID_vial").cloned().flatten().unwrap_or_default();
                line_no += 1;
                // 831 vial ids repeat (re-measurements); key is the line index.
                let key = format!("rapid_perovskites:{vial}:{line_no}");
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let pid = store::insert_provenance(
                    conn,
                    &prov(
                        "rapid_perovskites",
                        None,
                        Some(vial.clone()),
                        Some("RAPID perovskite campaign (darkreactions)".to_string()),
                    ),
                )?;
                let mut r = base_recipe(
                    format!("perovskite {vial}#{line_no}"),
                    key,
                    pid,
                    Some(SynthesisType::SolutionBased),
                );
                r.outcome_score = row
                    .get("_out_crystalscore")
                    .and_then(|s| s.as_deref())
                    .and_then(parse_f);
                let recipe_id = store::insert_recipe(conn, &r)?;

                let temp = row
                    .get("_rxn_temperatureC_actual_bulk")
                    .and_then(|s| s.as_deref())
                    .and_then(parse_f);
                let rt = row
                    .get("_rxn_reactiontimeS")
                    .and_then(|s| s.as_deref())
                    .and_then(parse_f);
                let step = RecipeStep {
                    id: 0,
                    recipe_id,
                    ordering: 1,
                    operation: Operation::Mix,
                    parameters: serde_json::json!({
                        "stirrate_rpm": row.get("_rxn_stirrateRPM"),
                        "mixingtime1_s": row.get("_rxn_mixingtime1S"),
                        "mixingtime2_s": row.get("_rxn_mixingtime2S"),
                    }),
                    conditions: Conditions {
                        temperature: temp
                            .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                        duration: rt.map(|t| Quantity::exact(t, Unit::Second)),
                        ..Conditions::default()
                    },
                };
                let step_id = store::insert_step(conn, &step)?;

                // organic amine via inchikey
                if let Some(ik) = row
                    .get("_rxn_organic_inchikey")
                    .and_then(|s| s.clone())
                    .filter(|s| !blank(s))
                {
                    let m = Material {
                        id: 0,
                        kind: MaterialKind::Molecule,
                        inchikey: Some(ik.clone()),
                        canonical_smiles: None,
                        formula: None,
                        composition: None,
                        names: vec![],
                        cas: None,
                        identity: Some(format!("inchikey:{ik}")),
                    };
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    let mmol = row
                        .get("_rxn_M_organic")
                        .and_then(|s| s.as_deref())
                        .and_then(parse_f);
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id,
                            material_id: mid,
                            role: MaterialRole::Reactant,
                            quantity: mmol.map(|v| Quantity::exact(v, Unit::Millimol)),
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                // stock solutions: identity at category level only
                for (col, name, role) in [
                    (
                        "_rxn_M_inorganic",
                        "RAPID inorganic stock (per inventory)",
                        MaterialRole::Precursor,
                    ),
                    (
                        "_rxn_M_acid",
                        "RAPID acid stock (per inventory)",
                        MaterialRole::Reactant,
                    ),
                ] {
                    if let Some(mmol) = row.get(col).and_then(|s| s.as_deref()).and_then(parse_f) {
                        let m = name_material(name);
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        store::insert_step_material(
                            conn,
                            &StepMaterial {
                                id: 0,
                                step_id,
                                material_id: mid,
                                role,
                                quantity: Some(Quantity::exact(mmol, Unit::Millimol)),
                                equivalents: None,
                                is_reference: false,
                                optional: false,
                                notes: None,
                            },
                        )?;
                    }
                }
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

// ---------- B3: ZeoSyn zeolites ----------

#[derive(Debug, Deserialize)]
struct ZeoRow {
    doi: Option<String>,
    cryst_time: Option<String>,
    cryst_temp: Option<String>,
    aging_time: Option<String>,
    aging_temp: Option<String>,
    ph: Option<String>,
    osda1: Option<String>,
    osda2: Option<String>,
    osda3: Option<String>,
    product1: Option<String>,
    #[allow(dead_code)]
    product2: Option<String>,
    #[allow(dead_code)]
    product3: Option<String>,
    precursors: Option<String>,
    #[serde(rename = "yield")]
    yield_: Option<String>,
    percent_cryst: Option<String>,
    title: Option<String>,
    osda1_smiles: Option<String>,
    osda2_smiles: Option<String>,
    osda3_smiles: Option<String>,
    #[allow(dead_code)]
    year: Option<String>,
}

/// ZeoSyn hydrothermal zeolite routes. Rows with no doi AND no product are
/// the xlsx blank separators — skipped and counted as rejected.
pub fn import_zeosyn(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines(&workdir.join("zeosyn.jsonl"))?;
    let mut idx = 0u64;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next() else { break };
                let line = line?;
                idx += 1;
                let row: ZeoRow = serde_json::from_str(&line)?;
                let product = row.product1.as_deref().unwrap_or("").trim();
                let doi = row.doi.as_deref().unwrap_or("").trim();
                if product.is_empty() && (doi.is_empty() || doi == "0") {
                    stats.rejected += 1;
                    continue;
                }
                n += 1;
                stats.seen += 1;
                let key = format!("zeosyn:{idx}");
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let pid = store::insert_provenance(
                    conn,
                    &prov(
                        "zeosyn",
                        if doi.is_empty() || doi == "0" {
                            None
                        } else {
                            Some(doi.to_string())
                        },
                        Some(format!("ZEOSYN.xlsx row {idx}")),
                        None,
                    ),
                )?;
                let target_id = if !product.is_empty() {
                    let m = Material {
                        id: 0,
                        kind: MaterialKind::Other("zeolite".to_string()),
                        inchikey: None,
                        canonical_smiles: None,
                        formula: None,
                        composition: None,
                        names: vec![product.to_string()],
                        cas: None,
                        identity: Some(format!("zeolite:{product}")),
                    };
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    Some(mid)
                } else {
                    None
                };
                let mut r = base_recipe(
                    format!("{product} (zeosyn:{idx})"),
                    key,
                    pid,
                    Some(SynthesisType::SolutionBased),
                );
                r.target_material_id = target_id;
                r.narrative = row.title.clone().filter(|t| !t.trim().is_empty());
                r.outcome_score = row
                    .percent_cryst
                    .as_deref()
                    .and_then(parse_f)
                    .map(|v| v / 100.0);
                let recipe_id = store::insert_recipe(conn, &r)?;

                let aging_t = row.aging_temp.as_deref().and_then(parse_f);
                let aging_h = row.aging_time.as_deref().and_then(parse_f);
                let mut ordering = 0;
                if aging_t.is_some() || aging_h.is_some() {
                    ordering += 1;
                    store::insert_step(
                        conn,
                        &RecipeStep {
                            id: 0,
                            recipe_id,
                            ordering,
                            operation: Operation::Other {
                                note: "aging".to_string(),
                            },
                            parameters: serde_json::json!({}),
                            conditions: Conditions {
                                temperature: aging_t.map(|t| {
                                    Temperature::Scalar(Quantity::exact(t, Unit::Celsius))
                                }),
                                duration: aging_h.map(|t| Quantity::exact(t, Unit::Hour)),
                                ..Conditions::default()
                            },
                        },
                    )?;
                }
                ordering += 1;
                let ph = row.ph.as_deref().and_then(parse_f);
                let step = RecipeStep {
                    id: 0,
                    recipe_id,
                    ordering,
                    operation: Operation::Hydrothermal,
                    parameters: serde_json::json!({"yield": row.yield_}),
                    conditions: Conditions {
                        temperature: row
                            .cryst_temp
                            .as_deref()
                            .and_then(parse_f)
                            .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                        duration: row
                            .cryst_time
                            .as_deref()
                            .and_then(parse_f)
                            .map(|t| Quantity::exact(t, Unit::Hour)),
                        ph: ph.map(|v| PhCondition {
                            operator: Operator::Eq,
                            value: v,
                        }),
                        ..Conditions::default()
                    },
                };
                let step_id = store::insert_step(conn, &step)?;

                for name in row.precursors.as_deref().unwrap_or("").split(',') {
                    if blank(name) {
                        continue;
                    }
                    let m = name_material(name);
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id,
                            material_id: mid,
                            role: MaterialRole::Precursor,
                            quantity: None,
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                for (osda, smiles) in [
                    (&row.osda1, &row.osda1_smiles),
                    (&row.osda2, &row.osda2_smiles),
                    (&row.osda3, &row.osda3_smiles),
                ] {
                    let (Some(name), _) = (osda, smiles) else {
                        continue;
                    };
                    if blank(name) {
                        continue;
                    }
                    let sm = smiles.as_deref().unwrap_or("").trim();
                    let m = if !sm.is_empty() {
                        let mut m = crate::molecule_material(sm);
                        m.names = vec![name.trim().to_string()];
                        m
                    } else {
                        name_material(name)
                    };
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id,
                            material_id: mid,
                            role: MaterialRole::Other("osda".to_string()),
                            quantity: None,
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

// ---------- B4: sol-gel corpus (hackingmaterials) ----------

/// The sol-gel JSONL (707 MB) — protocol objects with target, precursors,
/// reagents (role-tagged), and typed operations with min/max time/temp/pH.
/// license_status=uncleared is recorded in every provenance row (no LICENSE
/// in the source repo).
pub fn import_solgel(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines_lossy(&workdir.join("solgel.jsonl"))?;
    let mut idx = 0u64;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next_line()? else {
                    break;
                };
                idx += 1;
                if line.trim().is_empty() {
                    continue;
                }
                let rec: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(r) => r,
                    Err(_) => {
                        stats.rejected += 1;
                        continue;
                    }
                };
                n += 1;
                stats.seen += 1;
                let protocol = &rec["protocol"];
                let doi = protocol["doi"].as_str().map(str::to_string);
                let key = format!("solgel_hackingmaterials:{idx}");
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let pid = store::insert_provenance(
                    conn,
                    &prov(
                        "solgel_hackingmaterials",
                        doi,
                        Some(format!("sol_gel_dataset.jsonl record {idx}")),
                        Some("license_status=uncleared (no LICENSE in source repo)".to_string()),
                    ),
                )?;
                let target_formula = protocol["target"]["material_formula"]
                    .as_str()
                    .unwrap_or("");
                let target_id = if !target_formula.trim().is_empty() {
                    let m = crate::formula_material(target_formula, None);
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    Some(mid)
                } else {
                    None
                };
                let mut r = base_recipe(
                    format!("{target_formula} (solgel:{idx})"),
                    key,
                    pid,
                    Some(SynthesisType::SolutionBased),
                );
                r.target_material_id = target_id;
                let recipe_id = store::insert_recipe(conn, &r)?;

                for (oi, op) in rec["operations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let op_type = op["operation_type"].as_str().unwrap_or("");
                    let operation = match op_type.to_lowercase().as_str() {
                        "mixing" | "dissolution" => Operation::Mix,
                        "heating" => Operation::Heat,
                        "drying" => Operation::Dry,
                        "calcination" => Operation::Calcine,
                        "grinding" | "milling" => Operation::Grind,
                        "washing" => Operation::Wash,
                        "filtering" | "filtration" => Operation::Filter,
                        "sonication" => Operation::Sonicate,
                        "precipitation" => Operation::Precipitate,
                        other => {
                            *stats.unmapped_tokens.entry(other.to_string()).or_default() += 1;
                            Operation::Other {
                                note: other.to_string(),
                            }
                        }
                    };
                    let temp_min = op["temperature"]["min_value"].as_f64();
                    let temp_max = op["temperature"]["max_value"].as_f64();
                    let temperature = match (temp_min, temp_max) {
                        (Some(lo), Some(hi)) if lo != hi => Some(Temperature::MinMax {
                            min: Quantity::exact(lo, Unit::Celsius),
                            max: Quantity::exact(hi, Unit::Celsius),
                        }),
                        (Some(v), _) | (_, Some(v)) => {
                            Some(Temperature::Scalar(Quantity::exact(v, Unit::Celsius)))
                        }
                        _ => None,
                    };
                    let time_min = op["time"]["min_value"].as_f64();
                    let time_max = op["time"]["max_value"].as_f64();
                    let time_unit = op["time"]["unit"].as_str().unwrap_or("");
                    // Bare numbers without units are dropped: coercing them
                    // to a unit would be fabrication.
                    let duration = time_min.or(time_max).and_then(|v| {
                        match time_unit.to_lowercase().as_str() {
                            "h" | "hr" | "hrs" | "hour" | "hours" => Some(Unit::Hour),
                            "min" | "minutes" => Some(Unit::Minute),
                            "s" | "sec" | "seconds" => Some(Unit::Second),
                            "d" | "day" | "days" => Some(Unit::Day),
                            _ => None,
                        }
                        .map(|u| Quantity::exact(v, u))
                    });
                    let ph = op["pH"]["max_value"]
                        .as_f64()
                        .or(op["pH"]["min_value"].as_f64());
                    store::insert_step(
                        conn,
                        &RecipeStep {
                            id: 0,
                            recipe_id,
                            ordering: oi as i64 + 1,
                            operation,
                            parameters: serde_json::json!({
                                "operation_type": op_type,
                                "operation_details": op["operation_details"].as_str(),
                            }),
                            conditions: Conditions {
                                temperature,
                                duration,
                                ph: ph.map(|v| PhCondition {
                                    operator: Operator::Eq,
                                    value: v,
                                }),
                                ..Conditions::default()
                            },
                        },
                    )?;
                }

                let step_ids: Vec<i64> = conn
                    .prepare("SELECT id FROM recipe_step WHERE recipe_id = ?1 ORDER BY ordering")?
                    .query_map([recipe_id], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                let Some(&first_step) = step_ids.first() else {
                    stats.inserted += 1;
                    continue;
                };
                for pre in protocol["metal_precursors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let f = pre["material_formula"].as_str().unwrap_or("");
                    if f.trim().is_empty() {
                        continue;
                    }
                    let m = crate::formula_material(f, None);
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id: first_step,
                            material_id: mid,
                            role: MaterialRole::Precursor,
                            quantity: None,
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                for reagent in protocol["reagents"].as_array().into_iter().flatten() {
                    let role = reagent["reagent_role"].as_str().unwrap_or("");
                    let role = match role {
                        "solvent" => MaterialRole::Solvent,
                        "catalyst" => MaterialRole::Catalyst,
                        other => MaterialRole::Other(other.to_string()),
                    };
                    let smiles = reagent["smiles"].as_str().unwrap_or("");
                    let formula = reagent["reagent_formula"].as_str().unwrap_or("");
                    let m = if !smiles.trim().is_empty() {
                        crate::molecule_material(smiles.trim())
                    } else if !formula.trim().is_empty() {
                        crate::formula_material(formula.trim(), None)
                    } else {
                        continue;
                    };
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id: first_step,
                            material_id: mid,
                            role,
                            quantity: None,
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

// ---------- B5: MOF synthesis conditions ----------

/// MOF synthesis-condition JSON (pandas-split export converted to JSONL).
/// temperature/time carry "Unit" strings with OCR-ish suffixes ("K1.0",
/// "°C1.0"); operation list entries become steps in order.
pub fn import_mof(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines(&workdir.join("mof.jsonl"))?;
    let mut idx = 0u64;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next() else { break };
                let rec: serde_json::Value = serde_json::from_str(&line?)?;
                idx += 1;
                n += 1;
                stats.seen += 1;
                let key = format!("mof_synthesis_condition:{idx}");
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let name = rec["name"].as_str().unwrap_or("").trim().to_string();
                let doi = rec["doi"].as_str().map(str::to_string);
                let pid = store::insert_provenance(
                    conn,
                    &prov(
                        "mof_synthesis_condition",
                        doi,
                        Some(format!("MOF_synthesis_condition.json record {idx}")),
                        None,
                    ),
                )?;
                let target_id = if !name.is_empty() {
                    let m = Material {
                        id: 0,
                        kind: MaterialKind::Other("mof".to_string()),
                        inchikey: None,
                        canonical_smiles: None,
                        formula: None,
                        composition: None,
                        names: vec![name.clone()],
                        cas: None,
                        identity: Some(format!("mof:{}", name.to_lowercase())),
                    };
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    Some(mid)
                } else {
                    None
                };
                let mut r = base_recipe(
                    format!("{name} (mof:{idx})"),
                    key,
                    pid,
                    Some(SynthesisType::SolutionBased),
                );
                r.target_material_id = target_id;
                // yield as outcome_score (0..1 fraction)
                r.outcome_score = rec["property"]["yield"]["Value"]
                    .as_str()
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v / 100.0);
                let recipe_id = store::insert_recipe(conn, &r)?;

                let mut ordering = 0i64;
                for op in rec["operation"].as_array().into_iter().flatten() {
                    ordering += 1;
                    let oname = op["name"].as_str().unwrap_or("");
                    let (operation, mut temperature, mut duration) =
                        match oname.to_lowercase().as_str() {
                            "stir" | "mix" => (Operation::Mix, None, None),
                            "dissolve" => (Operation::Dissolve, None, None),
                            "crystallize" => (Operation::Precipitate, None, None),
                            "heat" | "anneal" => (Operation::Heat, None, None),
                            "dry" => (Operation::Dry, None, None),
                            "wash" => (Operation::Wash, None, None),
                            "filter" => (Operation::Filter, None, None),
                            "sonicate" => (Operation::Sonicate, None, None),
                            "" => (Operation::Hydrothermal, None, None),
                            other => {
                                *stats.unmapped_tokens.entry(other.to_string()).or_default() += 1;
                                (
                                    Operation::Other {
                                        note: other.to_string(),
                                    },
                                    None,
                                    None,
                                )
                            }
                        };
                    for cond in op["condition"].as_array().into_iter().flatten() {
                        let v = cond["Value"].as_str().and_then(|v| v.parse::<f64>().ok());
                        let unit = cond["Unit"].as_str().unwrap_or("");
                        let prop = cond["Property"].as_str().unwrap_or("");
                        match prop {
                            "Temperature" => {
                                temperature = v.map(|x| {
                                    let u = if unit.starts_with('K') {
                                        Unit::Kelvin
                                    } else {
                                        Unit::Celsius
                                    };
                                    Temperature::Scalar(Quantity::exact(x, u))
                                });
                            }
                            "Time" => {
                                duration = v.map(|x| {
                                    let u = if unit.starts_with('d') {
                                        Unit::Day
                                    } else {
                                        Unit::Hour
                                    };
                                    Quantity::exact(x, u)
                                });
                            }
                            _ => {}
                        }
                    }
                    store::insert_step(
                        conn,
                        &RecipeStep {
                            id: 0,
                            recipe_id,
                            ordering,
                            operation,
                            parameters: serde_json::json!({}),
                            conditions: Conditions {
                                temperature,
                                duration,
                                ..Conditions::default()
                            },
                        },
                    )?;
                }
                // No operations parsed: still emit one hydrothermal step so
                // materials have somewhere to attach (MOF default method).
                if ordering == 0 {
                    ordering = 1;
                    let temperature = rec["temperature"]["Value"]
                        .as_str()
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|x| {
                            let unit = rec["temperature"]["Unit"].as_str().unwrap_or("");
                            let u = if unit.starts_with('K') {
                                Unit::Kelvin
                            } else {
                                Unit::Celsius
                            };
                            Temperature::Scalar(Quantity::exact(x, u))
                        });
                    store::insert_step(
                        conn,
                        &RecipeStep {
                            id: 0,
                            recipe_id,
                            ordering,
                            operation: Operation::Hydrothermal,
                            parameters: serde_json::json!({}),
                            conditions: Conditions {
                                temperature,
                                ..Conditions::default()
                            },
                        },
                    )?;
                }
                let first_step: i64 = conn.query_row(
                    "SELECT id FROM recipe_step WHERE recipe_id = ?1 ORDER BY ordering LIMIT 1",
                    [recipe_id],
                    |r| r.get(0),
                )?;
                for (col, role) in [
                    ("M_precursor", MaterialRole::Precursor),
                    ("O_precursor", MaterialRole::Reactant),
                    ("S_precursor", MaterialRole::Solvent),
                ]
                .into_iter()
                {
                    for pre in rec[col].as_array().into_iter().flatten() {
                        let pname = pre["name"].as_str().unwrap_or("").trim();
                        let formula = pre["formula"].as_str().unwrap_or("").trim();
                        if pname.is_empty() && formula.is_empty() {
                            continue;
                        }
                        let m = if !formula.is_empty() {
                            crate::formula_material(formula, Some(pname.to_string()))
                        } else {
                            name_material(pname)
                        };
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        // composition pairs: [["0.52","mmol1.0"],["10","mL1.0"]]
                        let qty = pre["composition"].as_array().and_then(|pairs| {
                            pairs.iter().find_map(|p| {
                                let v = p[0].as_str()?.parse::<f64>().ok()?;
                                let u = p[1].as_str().unwrap_or("");
                                let unit = if u.starts_with("mmol") {
                                    Unit::Millimol
                                } else if u.starts_with("mol") {
                                    Unit::Mol
                                } else if u.starts_with("mL") {
                                    Unit::Milliliter
                                } else if u.starts_with("g") {
                                    Unit::Gram
                                } else {
                                    return None;
                                };
                                Some(Quantity::exact(v, unit))
                            })
                        });
                        store::insert_step_material(
                            conn,
                            &StepMaterial {
                                id: 0,
                                step_id: first_step,
                                material_id: mid,
                                role: role.clone(),
                                quantity: qty,
                                equivalents: None,
                                is_reference: false,
                                optional: false,
                                notes: None,
                            },
                        )?;
                    }
                }
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

// ---------- B6: Ceder 2020-07-13 version upgrade ----------

/// Ceder text-mined-synthesis 20200713 datasets. Dedupe against the existing
/// ceder_solid_state corpus by content hash (the 2020 export has no recipe
/// ids): sha256 of `doi|reaction_string`.
pub fn import_ceder2(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();

    // existing dedupe set
    let mut existing = std::collections::HashSet::new();
    {
        let mut stmt = store.raw().prepare(
            "SELECT p.doi, r.narrative FROM recipe r JOIN provenance p ON p.id = r.provenance_id
             WHERE r.external_key LIKE 'ceder_%'",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (doi, narrative) in rows {
            existing.insert(format!("{:?}", (doi, narrative)));
        }
    }

    for (file, dataset) in [
        ("ceder_ss.json", "ceder_solid_state_20200713"),
        ("ceder_sg.json", "ceder_solgel_20200713"),
    ] {
        let doc: serde_json::Value = serde_json::from_reader(File::open(workdir.join(file))?)?;
        let reactions = doc["reactions"].as_array().cloned().unwrap_or_default();
        let mut stream = reactions.into_iter();
        loop {
            let n = store.with_transaction(|conn| {
                let mut n = 0;
                while n < BATCH {
                    let Some(rec) = stream.next() else { break };
                    n += 1;
                    stats.seen += 1;
                    let doi = rec["doi"].as_str().map(str::to_string);
                    let rxn = rec["reaction_string"].as_str().map(str::to_string);
                    let dedupe = format!("{:?}", (doi.clone(), rxn.clone()));
                    if existing.contains(&dedupe) {
                        stats.skipped_existing += 1;
                        continue;
                    }
                    let hash = format!("{:x}", md5::compute(dedupe.as_bytes()));
                    let key = format!("{dataset}:{hash}");
                    if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                        stats.skipped_existing += 1;
                        continue;
                    }
                    existing.insert(dedupe);
                    let pid = store::insert_provenance(
                        conn,
                        &prov(
                            dataset,
                            doi.clone(),
                            Some(format!("{file}:{hash}")),
                            Some("dataset version 2020-07-13".to_string()),
                        ),
                    )?;
                    let target_formula = rec["target"]["material_formula"].as_str().unwrap_or("");
                    let target_id = if !target_formula.trim().is_empty() {
                        let m = crate::formula_material(target_formula, None);
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        Some(mid)
                    } else {
                        None
                    };
                    let synth = rec["synthesis_type"].as_str().unwrap_or("");
                    let mut r = base_recipe(
                        format!("{target_formula} ({dataset}:{})", &hash[..8]),
                        key,
                        pid,
                        Some(match synth {
                            "sol-gel" | "solution-based" => SynthesisType::SolutionBased,
                            _ => SynthesisType::SolidState,
                        }),
                    );
                    r.target_material_id = target_id;
                    r.narrative = rxn;
                    let recipe_id = store::insert_recipe(conn, &r)?;

                    let mut first_step = None;
                    for (i, op) in rec["operations"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .enumerate()
                    {
                        let token = op["token"].as_str();
                        let action = op["type"].as_str();
                        let operation = crate::map_operation(action, token);
                        if let Operation::Other { note } = &operation {
                            *stats.unmapped_tokens.entry(note.clone()).or_default() += 1;
                        }
                        let c = &op["conditions"];
                        let temp = c["heating_temperature"].as_f64();
                        let time = c["heating_time"].as_f64();
                        let step = RecipeStep {
                            id: 0,
                            recipe_id,
                            ordering: i as i64 + 1,
                            operation,
                            parameters: serde_json::json!({"type": action, "token": token}),
                            conditions: Conditions {
                                temperature: temp.map(|t| {
                                    Temperature::Scalar(Quantity::exact(t, Unit::Celsius))
                                }),
                                duration: time.map(|t| Quantity::exact(t, Unit::Hour)),
                                ..Conditions::default()
                            },
                        };
                        let sid = store::insert_step(conn, &step)?;
                        if first_step.is_none() {
                            first_step = Some(sid);
                        }
                    }
                    let step_id = match first_step {
                        Some(s) => s,
                        None => store::insert_step(
                            conn,
                            &RecipeStep {
                                id: 0,
                                recipe_id,
                                ordering: 1,
                                operation: Operation::Other {
                                    note: "unspecified procedure".to_string(),
                                },
                                parameters: serde_json::json!({}),
                                conditions: Conditions::default(),
                            },
                        )?,
                    };
                    for pre in rec["reaction"]["left_side"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        let f = pre["material"].as_str().unwrap_or("").trim();
                        if f.is_empty() {
                            continue;
                        }
                        let m = crate::formula_material(f, None);
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        let moles = pre["amount"].as_str().and_then(|a| a.parse::<f64>().ok());
                        store::insert_step_material(
                            conn,
                            &StepMaterial {
                                id: 0,
                                step_id,
                                material_id: mid,
                                role: MaterialRole::Precursor,
                                quantity: moles.map(|v| Quantity::exact(v, Unit::Mol)),
                                equivalents: None,
                                is_reference: false,
                                optional: false,
                                notes: None,
                            },
                        )?;
                    }
                    stats.inserted += 1;
                }
                Ok::<_, ImportError>(n)
            })?;
            if n == 0 {
                break;
            }
        }
    }
    Ok(stats)
}

// ---------- B7: Precursor Genome ----------

#[derive(Debug, Deserialize)]
struct PgRow {
    sample_id: String,
    target_compound: Option<String>,
    precursor_formulas: Vec<Option<String>>,
    heating_temperature: Option<f64>,
    heating_time: Option<f64>,
    furnace_name: Option<String>,
    reaction_category: Option<String>,
    category_notes: Option<String>,
    reaction_energy_ev_per_atom: Option<f64>,
}

pub fn import_precursor_genome(
    store: &mut Store,
    workdir: &Path,
) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines(&workdir.join("precursor_genome.jsonl"))?;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next() else { break };
                let row: PgRow = serde_json::from_str(&line?)?;
                n += 1;
                stats.seen += 1;
                let key = format!("precursor_genome:{}", row.sample_id);
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let pid = store::insert_provenance(
                    conn,
                    &prov(
                        "precursor_genome",
                        None,
                        Some(format!("ledger_precursor_genome.json {}", row.sample_id)),
                        None,
                    ),
                )?;
                let target_id = row
                    .target_compound
                    .as_deref()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| {
                        let m = crate::formula_material(t, None);
                        let identity = m.identity.clone().unwrap();
                        ctx.get_or_create_material(conn, &m, &identity)
                            .map(|(id, _)| id)
                    })
                    .transpose()?;
                let mut r = base_recipe(
                    format!(
                        "{} ({})",
                        row.target_compound.as_deref().unwrap_or("unknown"),
                        row.sample_id
                    ),
                    key,
                    pid,
                    Some(SynthesisType::SolidState),
                );
                r.target_material_id = target_id;
                r.outcome = row.reaction_category.as_deref().map(|c| match c {
                    "transformed" => Outcome::Success,
                    c if c.contains("partial") => Outcome::Partial,
                    _ => Outcome::Failed,
                });
                r.narrative = row.category_notes.clone().filter(|s| !s.is_empty());
                let recipe_id = store::insert_recipe(conn, &r)?;
                let step = RecipeStep {
                    id: 0,
                    recipe_id,
                    ordering: 1,
                    operation: Operation::Heat,
                    parameters: serde_json::json!({
                        "furnace": row.furnace_name,
                        "reaction_energy_ev_per_atom": row.reaction_energy_ev_per_atom,
                    }),
                    conditions: Conditions {
                        temperature: row
                            .heating_temperature
                            .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                        duration: row.heating_time.map(|t| Quantity::exact(t, Unit::Minute)),
                        ..Conditions::default()
                    },
                };
                let step_id = store::insert_step(conn, &step)?;
                for f in row.precursor_formulas.iter().flatten() {
                    if f.trim().is_empty() {
                        continue;
                    }
                    let m = crate::formula_material(f, None);
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    store::insert_step_material(
                        conn,
                        &StepMaterial {
                            id: 0,
                            step_id,
                            material_id: mid,
                            role: MaterialRole::Precursor,
                            quantity: None,
                            equivalents: None,
                            is_reference: false,
                            optional: false,
                            notes: None,
                        },
                    )?;
                }
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

// ---------- B8: GPSS + A-Lab ----------

/// GPSS 352-sample campaign: composition + synthesis temperature + XRD
/// outcome weight fractions (kept in step parameters; outcome_score holds
/// the max phase weight fraction).
pub fn import_gpss(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    let mut stream = read_lines(&workdir.join("gpss.jsonl"))?;
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH {
                let Some(line) = stream.next() else { break };
                let rec: serde_json::Value = serde_json::from_str(&line?)?;
                n += 1;
                stats.seen += 1;
                let idx = rec["sample_index"].as_i64().unwrap_or(0);
                let key = format!("gpss_352:{idx}");
                if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                    stats.skipped_existing += 1;
                    continue;
                }
                let composition = rec["composition"].as_str().unwrap_or("").trim();
                let pid = store::insert_provenance(conn, &prov(
                    "gpss_352",
                    None,
                    Some(format!("gpss_data.zip sample {idx}")),
                    Some(format!("provenance: {}", rec["provenance"].as_str().unwrap_or(""))),
                ))?;
                let target_id = if !composition.is_empty() {
                    let m = crate::formula_material(composition, None);
                    let identity = m.identity.clone().unwrap();
                    let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                    Some(mid)
                } else {
                    None
                };
                let mut r = base_recipe(
                    format!("{composition} (gpss:{idx})"),
                    key,
                    pid,
                    Some(SynthesisType::SolidState),
                );
                r.target_material_id = target_id;
                let phases = &rec["xrd_analysis_result"]["weight_fractions_of_each_phases"];
                let max_frac = phases.as_object().and_then(|o| {
                    o.values().filter_map(|v| v.as_f64()).reduce(f64::max)
                });
                r.outcome_score = max_frac;
                let recipe_id = store::insert_recipe(conn, &r)?;
                store::insert_step(conn, &RecipeStep {
                    id: 0,
                    recipe_id,
                    ordering: 1,
                    operation: Operation::Heat,
                    parameters: serde_json::json!({
                        "xrd_weight_fractions": phases,
                        "ionic_conductivity_rt_s_per_cm": rec["ionic_conductivity_room_temperature (S/cm)"],
                    }),
                    conditions: Conditions {
                        temperature: rec["synthesis_temperature"].as_f64()
                            .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                        ..Conditions::default()
                    },
                })?;
                stats.inserted += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(stats)
}

/// A-Lab supporting data, both versions as separate provenance-tagged rows:
/// the original MOESM3 CSV and the corrected version (per the 2025
/// correction, s41586-025-09992-y: Zn2Cr3FeO8 row removed upstream — the
/// acquired CSV already lacks it, so both files are identical in content
/// but distinct in provenance).
pub fn import_alab(store: &mut Store, workdir: &Path) -> Result<SourceStat, ImportError> {
    let mut stats = SourceStat::default();
    let mut ctx = crate::ImportCtx::default();
    for (file, dataset, note) in [
        ("alab.jsonl", "alab_moesm3", "A-Lab SI (20230502 Synthesis Results with Recipes.csv); original 41/58 claim".to_string()),
        ("alab_corrected.jsonl", "alab_moesm3_corrected", "per correction s41586-025-09992-y (36/57 claim); acquired CSV already excludes the removed Zn2Cr3FeO8 row".to_string()),
    ] {
        let mut stream = read_lines(&workdir.join(file))?;
        loop {
            let n = store.with_transaction(|conn| {
                let mut n = 0;
                while n < BATCH {
                    let Some(line) = stream.next() else { break };
                    let rec: serde_json::Value = serde_json::from_str(&line?)?;
                    n += 1;
                    stats.seen += 1;
                    let target = rec["Target"].as_str().unwrap_or("").trim();
                    let key = format!("{dataset}:{target}");
                    if store::recipe_id_by_external_key(conn, &key)?.is_some() {
                        stats.skipped_existing += 1;
                        continue;
                    }
                    let pid = store::insert_provenance(conn, &prov(
                        dataset,
                        Some("10.1038/s41586-023-06734-w".to_string()),
                        Some(format!("{file}:{target}")),
                        Some(note.clone()),
                    ))?;
                    let target_id = if !target.is_empty() {
                        let m = crate::formula_material(target, None);
                        let identity = m.identity.clone().unwrap();
                        let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                        Some(mid)
                    } else {
                        None
                    };
                    let mut r = base_recipe(
                        format!("{target} ({dataset})"),
                        key,
                        pid,
                        Some(SynthesisType::SolidState),
                    );
                    r.target_material_id = target_id;
                    r.outcome = rec["Result"].as_str().map(|s| match s {
                        "Success" | "Success (offline)" => Outcome::Success,
                        "Partial" => Outcome::Partial,
                        _ => Outcome::Failed,
                    });
                    let recipe_id = store::insert_recipe(conn, &r)?;
                    let step = RecipeStep {
                        id: 0,
                        recipe_id,
                        ordering: 1,
                        operation: Operation::Heat,
                        parameters: serde_json::json!({
                            "materials_project_id": rec["Materials Project ID"],
                        }),
                        conditions: Conditions {
                            temperature: rec["Temperature (C)"].as_str().and_then(|v| v.parse::<f64>().ok())
                                .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
                            duration: rec["Duration (hours)"].as_str().and_then(|v| v.parse::<f64>().ok())
                                .map(|t| Quantity::exact(t, Unit::Hour)),
                            ..Conditions::default()
                        },
                    };
                    let step_id = store::insert_step(conn, &step)?;
                    // "828.0 mg CaCO3 + 36.7 mg Fe2O3 + ..."
                    if let Some(pm) = rec["Precursors + Masses"].as_str() {
                        for part in pm.split('+') {
                            let part = part.trim();
                            let mut it = part.rsplitn(2, ' ');
                            let (Some(formula), Some(mass)) = (it.next(), it.next()) else {
                                continue;
                            };
                            let grams = mass
                                .trim()
                                .strip_suffix("mg")
                                .and_then(|v| v.trim().parse::<f64>().ok())
                                .map(|mg| mg / 1000.0);
                            let m = crate::formula_material(formula, None);
                            let identity = m.identity.clone().unwrap();
                            let (mid, _) = ctx.get_or_create_material(conn, &m, &identity)?;
                            store::insert_step_material(conn, &StepMaterial {
                                id: 0,
                                step_id,
                                material_id: mid,
                                role: MaterialRole::Precursor,
                                quantity: grams.map(|g| Quantity::exact(g, Unit::Gram)),
                                equivalents: None,
                                is_reference: false,
                                optional: false,
                                notes: None,
                            })?;
                        }
                    }
                    stats.inserted += 1;
                }
                Ok::<_, ImportError>(n)
            })?;
            if n == 0 {
                break;
            }
        }
    }
    Ok(stats)
}
