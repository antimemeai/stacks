//! Importer for the chem-recipes bootstrap corpus (4 inorganic sources from
//! `recipes.duckdb` + the ORD organic reaction mirror). Input is JSONL
//! extracted ahead of time with the duckdb CLI, so no duckdb/parquet crates
//! enter the workspace.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;
use stacks_core::store;
use stacks_core::*;
use thiserror::Error;

pub mod chunker;
pub mod datasets;
pub mod embed;
pub mod enrich;
pub mod extract;
pub mod inproc;
pub mod intake_triage;
pub mod library_import;
pub mod matdattmp;
pub mod status;
pub mod migrate_ledger;
pub mod pplx_embed;
pub mod pplx_eval;
pub mod materials_wave;
pub mod verify;
pub mod wave;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Http(String),
}

const BATCH_SIZE: usize = 5_000;
const CREATED_BY: &str = "stacks-import";

#[derive(Debug, Default)]
pub struct SourceStats {
    pub recipes_seen: u64,
    pub recipes_inserted: u64,
    pub recipes_skipped_existing: u64,
    pub materials_created: u64,
    pub materials_reused: u64,
    pub steps_inserted: u64,
    pub step_materials_inserted: u64,
    pub provenance_inserted: u64,
    pub atmosphere_mapped: u64,
    pub atmosphere_other: u64,
    pub atmosphere_absent: u64,
    /// Operation tokens that fell through to `Operation::Other`.
    pub unmapped_operation_tokens: BTreeMap<String, u64>,
    /// Time-unit strings that could not be mapped; the value was dropped.
    pub dropped_time_units: BTreeMap<String, u64>,
    pub amounts_dropped: u64,
}

impl SourceStats {
    fn merge(&mut self, other: &SourceStats) {
        self.recipes_seen += other.recipes_seen;
        self.recipes_inserted += other.recipes_inserted;
        self.recipes_skipped_existing += other.recipes_skipped_existing;
        self.materials_created += other.materials_created;
        self.materials_reused += other.materials_reused;
        self.steps_inserted += other.steps_inserted;
        self.step_materials_inserted += other.step_materials_inserted;
        self.provenance_inserted += other.provenance_inserted;
        self.atmosphere_mapped += other.atmosphere_mapped;
        self.atmosphere_other += other.atmosphere_other;
        self.atmosphere_absent += other.atmosphere_absent;
        self.amounts_dropped += other.amounts_dropped;
        for (k, v) in &other.unmapped_operation_tokens {
            *self.unmapped_operation_tokens.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &other.dropped_time_units {
            *self.dropped_time_units.entry(k.clone()).or_default() += v;
        }
    }
}

#[derive(Debug, Default)]
pub struct ImportStats {
    pub per_source: BTreeMap<String, SourceStats>,
}

#[derive(Debug, Deserialize)]
struct InorganicRecipe {
    recipe_id: String,
    source: String,
    synthesis_type: Option<String>,
    doi: Option<String>,
    target_formula: Option<String>,
    target_name: Option<String>,
    reaction_string: Option<String>,
    temperature_min: Option<f64>,
    temperature_max: Option<f64>,
    time_min: Option<f64>,
    time_max: Option<f64>,
    atmosphere: Option<String>,
    has_atmosphere_info: Option<bool>,
    mp_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Precursor {
    recipe_id: String,
    role: Option<String>,
    formula: Option<String>,
    name: Option<String>,
    amount: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OperationRow {
    recipe_id: String,
    step_index: i64,
    action_type: Option<String>,
    token: Option<String>,
    temperature_c: Option<f64>,
    time_value: Option<f64>,
    time_units: Option<String>,
    atmosphere: Option<String>,
    mixing_device: Option<String>,
    mixing_media: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrdReaction {
    reaction_id: String,
    dataset_id: Option<String>,
    synthesis_type: Option<String>,
    reaction_smiles: Option<String>,
    reactants_smiles_json: Option<String>,
    reagents_smiles_json: Option<String>,
    solvents_smiles_json: Option<String>,
    catalysts_smiles_json: Option<String>,
    products_smiles_json: Option<String>,
    temperature_c: Option<f64>,
    time_h: Option<f64>,
    atmosphere: Option<String>,
    doi: Option<String>,
}

fn blank(s: &Option<String>) -> bool {
    s.as_deref().map(str::trim).unwrap_or("").is_empty()
}

fn nonblank(s: &Option<String>) -> Option<String> {
    if blank(s) {
        None
    } else {
        Some(s.as_ref().unwrap().trim().to_string())
    }
}

/// Map a source token to an Operation. `action_type` is the structured
/// column; the free-text gerund `token` is the fallback and is preserved in
/// the step parameters either way.
pub fn map_operation(action_type: Option<&str>, token: Option<&str>) -> Operation {
    let tok = token.unwrap_or("").to_ascii_lowercase();
    let from_token = match tok.as_str() {
        "heated" | "heating" | "annealed" | "fired" => Some(Operation::Heat),
        "mixed" | "mixing" | "stirred" | "stirring" => Some(Operation::Mix),
        "ground" | "grinding" | "reground" => Some(Operation::Grind),
        "dissolved" => Some(Operation::Dissolve),
        "precipitated" => Some(Operation::Precipitate),
        "filtered" | "filtration" | "filtering" | "filtrated" => Some(Operation::Filter),
        "washed" | "washing" | "rinsed" => Some(Operation::Wash),
        "dried" | "drying" => Some(Operation::Dry),
        "calcined" | "calcination" => Some(Operation::Calcine),
        "milled" | "milling" => Some(Operation::Mill),
        "sonicated" | "sonication" | "ultrasonication" => Some(Operation::Sonicate),
        "crushed" | "crushing" => Some(Operation::Grind),
        _ => None,
    };
    if let Some(op) = from_token {
        return op;
    }
    let at = action_type.unwrap_or("").to_ascii_lowercase();
    match at.as_str() {
        "heatingoperation" => Operation::Heat,
        "mixing" | "mixingoperation" | "solutionmixing" => Operation::Mix,
        "liquidgrinding" => Operation::Grind,
        "dryingoperation" => Operation::Dry,
        _ => Operation::Other {
            note: if tok.is_empty() { at } else { tok },
        },
    }
}

/// Parse a messy free-text atmosphere string. First comma-segment that
/// matches the closed vocabulary wins; the raw string is preserved in
/// `atmosphere_note` whenever it is more specific than the enum.
pub fn parse_atmosphere(raw: &str) -> (Option<Atmosphere>, Option<String>) {
    let mut note = None;
    for seg in raw.split(',') {
        let s = seg.trim().to_ascii_lowercase();
        let atm = match s.as_str() {
            "air" | "ambient" => Some(Atmosphere::Air),
            "ar" | "argon" => Some(Atmosphere::Ar),
            "n2" | "nitrogen" => Some(Atmosphere::N2),
            "h2" | "hydrogen" => Some(Atmosphere::H2),
            "o2" | "oxygen" => Some(Atmosphere::O2),
            "vacuum" => Some(Atmosphere::Vacuum),
            _ => None,
        };
        if let Some(atm) = atm {
            if !s.eq_ignore_ascii_case(raw.trim()) {
                note = Some(raw.trim().to_string());
            }
            return (Some(atm), note);
        }
    }
    let trimmed = raw.trim();
    if trimmed.len() <= 60 {
        (Some(Atmosphere::Other(trimmed.to_string())), None)
    } else {
        (None, Some(trimmed.to_string()))
    }
}

fn map_time_unit(unit: &str) -> Option<Unit> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(Unit::Hour),
        "min" | "minutes" => Some(Unit::Minute),
        "d" | "day" | "days" => Some(Unit::Day),
        "s" | "sec" | "secs" | "seconds" => Some(Unit::Second),
        _ => None,
    }
}

fn range_or_scalar(min: Option<f64>, max: Option<f64>, unit: Unit) -> Option<Quantity> {
    let (lo, hi) = match (min, max) {
        (Some(lo), Some(hi)) if lo <= hi => (lo, hi),
        (Some(v), None) | (None, Some(v)) => (v, v),
        _ => return None,
    };
    if lo == hi {
        Some(Quantity::exact(lo, unit))
    } else {
        Some(Quantity {
            value: hi,
            unit,
            operator: Operator::Range { min: lo, max: hi },
        })
    }
}

fn temperature_condition(min: Option<f64>, max: Option<f64>) -> Option<Temperature> {
    let (lo, hi) = match (min, max) {
        (Some(lo), Some(hi)) if lo <= hi => (lo, hi),
        (Some(v), None) | (None, Some(v)) => (v, v),
        _ => return None,
    };
    if lo == hi {
        Some(Temperature::Scalar(Quantity::exact(lo, Unit::Celsius)))
    } else {
        Some(Temperature::MinMax {
            min: Quantity::exact(lo, Unit::Celsius),
            max: Quantity::exact(hi, Unit::Celsius),
        })
    }
}

/// Shared per-import context: stats plus the material identity cache that
/// makes get-or-create cheap across batch boundaries.
/// Shared material identity cache for importers (get-or-create across
/// batches). Reused by the matdattmp wave importers.
#[derive(Default)]
pub struct ImportCtx {
    stats: ImportStats,
    material_cache: HashMap<(String, String), i64>,
}

impl ImportCtx {
    fn source_mut(&mut self, source: &str) -> &mut SourceStats {
        self.stats.per_source.entry(source.to_string()).or_default()
    }

    /// Returns `(id, created)` — `created` is true when a new row was
    /// inserted rather than found via cache or the identity index.
    pub fn get_or_create_material(
        &mut self,
        conn: &rusqlite::Connection,
        material: &Material,
        identity: &str,
    ) -> Result<(i64, bool), ImportError> {
        let key = (material.kind.as_db_token(), identity.to_string());
        if let Some(&id) = self.material_cache.get(&key) {
            return Ok((id, false));
        }
        if let Some(id) = store::material_id_by_identity(conn, &material.kind, identity)? {
            self.material_cache.insert(key, id);
            return Ok((id, false));
        }
        let id = store::insert_material(conn, material)?;
        self.material_cache.insert(key, id);
        Ok((id, true))
    }
}

struct Jsonl<R: BufRead> {
    lines: std::io::Lines<R>,
}

impl<R: BufRead> Jsonl<R> {
    fn next<T: for<'de> Deserialize<'de>>(&mut self) -> Result<Option<T>, ImportError> {
        match self.lines.next() {
            None => Ok(None),
            Some(line) => Ok(Some(serde_json::from_str(&line?)?)),
        }
    }
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, ImportError> {
    let reader = BufReader::with_capacity(1 << 20, File::open(path)?);
    reader
        .lines()
        .map(|l| serde_json::from_str(&l?).map_err(Into::into))
        .collect()
}

fn parse_smiles_list(json: &Option<String>) -> Vec<String> {
    nonblank(json)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Molecule-kind material keyed by SMILES, for importers.
pub fn molecule_material(smiles: &str) -> Material {
    Material {
        id: 0,
        kind: MaterialKind::Molecule,
        inchikey: None,
        canonical_smiles: Some(smiles.to_string()),
        formula: None,
        composition: None,
        names: vec![],
        cas: None,
        identity: Some(format!("smiles:{smiles}")),
    }
}

/// Formula-kind material with identity key, for importers.
pub fn formula_material(formula: &str, name: Option<String>) -> Material {
    let formula = formula.trim().to_string();
    let names = match name {
        Some(n) if !n.is_empty() && n != formula => vec![n],
        _ => vec![],
    };
    Material {
        id: 0,
        kind: MaterialKind::Formula,
        inchikey: None,
        canonical_smiles: None,
        formula: Some(formula.clone()),
        composition: None,
        names,
        cas: None,
        identity: Some(format!("formula:{formula}")),
    }
}

fn provenance(source_dataset: &str, doi: Option<String>, locator: Option<String>) -> Provenance {
    Provenance {
        id: 0,
        kind: ProvenanceKind::DatasetImport,
        doi,
        source_dataset: Some(source_dataset.to_string()),
        path: None,
        sha256: None,
        locator,
        extractor_version: Some("chem-recipes/duckdb-jsonl".to_string()),
        extraction_method: ExtractionMethod::Structured,
        confidence: 1.0,
        note: None,
    }
}

fn import_one_inorganic(
    conn: &rusqlite::Connection,
    ctx: &mut ImportCtx,
    rec: &InorganicRecipe,
    precursors: &HashMap<String, Vec<Precursor>>,
    operations: &HashMap<String, Vec<OperationRow>>,
    created_at: &str,
) -> Result<(), ImportError> {
    let source = rec.source.clone();
    let external_key = format!("{source}:{}", rec.recipe_id);
    {
        let s = ctx.source_mut(&source);
        s.recipes_seen += 1;
    }
    if store::recipe_id_by_external_key(conn, &external_key)?.is_some() {
        ctx.source_mut(&source).recipes_skipped_existing += 1;
        return Ok(());
    }

    let target_material_id = match nonblank(&rec.target_formula) {
        Some(f) => {
            let m = formula_material(&f, nonblank(&rec.target_name));
            let identity = m.identity.clone().unwrap();
            let (id, created) = ctx.get_or_create_material(conn, &m, &identity)?;
            let s = ctx.source_mut(&source);
            if created {
                s.materials_created += 1;
            } else {
                s.materials_reused += 1;
            }
            Some(id)
        }
        None => None,
    };

    let prov = provenance(&source, nonblank(&rec.doi), nonblank(&rec.mp_id));
    let provenance_id = store::insert_provenance(conn, &prov)?;
    ctx.source_mut(&source).provenance_inserted += 1;

    let target_label = nonblank(&rec.target_formula)
        .or_else(|| nonblank(&rec.target_name))
        .unwrap_or_else(|| "unnamed target".to_string());
    let synthesis_type = nonblank(&rec.synthesis_type).map(|t| match t.as_str() {
        "solid-state" => SynthesisType::SolidState,
        "solution-based" => SynthesisType::SolutionBased,
        "organic" => SynthesisType::Organic,
        other => SynthesisType::Other(other.to_string()),
    });
    let recipe = Recipe {
        id: 0,
        name: format!("{target_label} ({external_key})"),
        version: 1,
        status: RecipeStatus::Draft,
        target_material_id,
        target_quantity: None,
        synthesis_type,
        narrative: nonblank(&rec.reaction_string),
        created_from_recipe_id: None,
        provenance_id: Some(provenance_id),
        created_at: created_at.to_string(),
        created_by: Some(CREATED_BY.to_string()),
        supersedes: None,
        external_key: Some(external_key),
        outcome: None,
        outcome_score: None,
    };
    let recipe_id = store::insert_recipe(conn, &recipe)?;
    ctx.source_mut(&source).recipes_inserted += 1;

    // Recipe-level conditions, used as defaults for every step.
    let recipe_temp = temperature_condition(rec.temperature_min, rec.temperature_max);
    let recipe_duration = range_or_scalar(rec.time_min, rec.time_max, Unit::Hour);
    let (recipe_atm, recipe_atm_note) = if rec.has_atmosphere_info == Some(true) {
        match nonblank(&rec.atmosphere) {
            Some(raw) => {
                let (atm, note) = parse_atmosphere(&raw);
                let s = ctx.source_mut(&source);
                match &atm {
                    Some(Atmosphere::Other(_)) => s.atmosphere_other += 1,
                    Some(_) => s.atmosphere_mapped += 1,
                    None => s.atmosphere_absent += 1,
                }
                (atm, note)
            }
            None => {
                ctx.source_mut(&source).atmosphere_absent += 1;
                (None, None)
            }
        }
    } else {
        ctx.source_mut(&source).atmosphere_absent += 1;
        (None, None)
    };

    let mut ops: Vec<&OperationRow> = operations
        .get(&rec.recipe_id)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    ops.sort_by_key(|o| o.step_index);

    let has_recipe_conditions =
        recipe_temp.is_some() || recipe_duration.is_some() || recipe_atm.is_some();
    let mut steps: Vec<(i64, Operation, serde_json::Value, Conditions)> = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let operation = map_operation(op.action_type.as_deref(), op.token.as_deref());
        if let Operation::Other { note } = &operation {
            *ctx.source_mut(&source)
                .unmapped_operation_tokens
                .entry(note.clone())
                .or_default() += 1;
        }
        let parameters = serde_json::json!({
            "action_type": op.action_type,
            "token": op.token,
            "mixing_device": nonblank(&op.mixing_device),
            "mixing_media": nonblank(&op.mixing_media),
        });
        let duration = match (op.time_value, nonblank(&op.time_units)) {
            (Some(v), Some(u)) => match map_time_unit(&u) {
                Some(unit) => Some(Quantity::exact(v, unit)),
                None => {
                    *ctx.source_mut(&source)
                        .dropped_time_units
                        .entry(u)
                        .or_default() += 1;
                    None
                }
            },
            _ => None,
        };
        let (op_atm, op_atm_note) = match nonblank(&op.atmosphere) {
            Some(raw) => parse_atmosphere(&raw),
            None => (None, None),
        };
        let conditions = Conditions {
            temperature: op
                .temperature_c
                .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius)))
                .or_else(|| recipe_temp.clone()),
            pressure: None,
            duration: duration.or_else(|| recipe_duration.clone()),
            atmosphere: op_atm.or_else(|| recipe_atm.clone()),
            ph: None,
            stirring: None,
            atmosphere_note: op_atm_note.or_else(|| recipe_atm_note.clone()),
        };
        steps.push((i as i64 + 1, operation, parameters, conditions));
    }
    if steps.is_empty() && (has_recipe_conditions || precursors.contains_key(&rec.recipe_id)) {
        steps.push((
            1,
            Operation::Other {
                note: "unspecified procedure".to_string(),
            },
            serde_json::json!({}),
            Conditions {
                temperature: recipe_temp.clone(),
                pressure: None,
                duration: recipe_duration.clone(),
                atmosphere: recipe_atm.clone(),
                ph: None,
                stirring: None,
                atmosphere_note: recipe_atm_note.clone(),
            },
        ));
    }

    let mut first_step_id = None;
    for (ordering, operation, parameters, conditions) in steps {
        let step = RecipeStep {
            id: 0,
            recipe_id,
            ordering,
            operation,
            parameters,
            conditions,
        };
        let step_id = store::insert_step(conn, &step)?;
        ctx.source_mut(&source).steps_inserted += 1;
        if first_step_id.is_none() {
            first_step_id = Some(step_id);
        }
    }

    if let (Some(step_id), Some(pres)) = (first_step_id, precursors.get(&rec.recipe_id)) {
        for pre in pres {
            let Some(formula) = nonblank(&pre.formula) else {
                continue;
            };
            let m = formula_material(&formula, nonblank(&pre.name));
            let identity = m.identity.clone().unwrap();
            let (material_id, created) = ctx.get_or_create_material(conn, &m, &identity)?;
            let s = ctx.source_mut(&source);
            if created {
                s.materials_created += 1;
            } else {
                s.materials_reused += 1;
            }
            if pre.amount.is_some() {
                ctx.source_mut(&source).amounts_dropped += 1;
            }
            let role = match pre.role.as_deref() {
                Some("product") => MaterialRole::Product,
                _ => MaterialRole::Precursor,
            };
            let sm = StepMaterial {
                id: 0,
                step_id,
                material_id,
                role,
                quantity: None,
                equivalents: None,
                is_reference: false,
                optional: false,
                notes: None,
            };
            store::insert_step_material(conn, &sm)?;
            ctx.source_mut(&source).step_materials_inserted += 1;
        }
    }
    Ok(())
}

/// Import the four inorganic sources. `dir` holds `recipes.jsonl`,
/// `precursors.jsonl`, `operations.jsonl` as produced by the duckdb CLI.
pub fn import_inorganic(store: &mut Store, dir: &Path) -> Result<ImportStats, ImportError> {
    let precursors: Vec<Precursor> = read_jsonl(&dir.join("precursors.jsonl"))?;
    let mut precursors_by_recipe: HashMap<String, Vec<Precursor>> = HashMap::new();
    for p in precursors {
        precursors_by_recipe
            .entry(p.recipe_id.clone())
            .or_default()
            .push(p);
    }
    let operations: Vec<OperationRow> = read_jsonl(&dir.join("operations.jsonl"))?;
    let mut operations_by_recipe: HashMap<String, Vec<OperationRow>> = HashMap::new();
    for o in operations {
        operations_by_recipe
            .entry(o.recipe_id.clone())
            .or_default()
            .push(o);
    }

    let reader = BufReader::with_capacity(1 << 20, File::open(dir.join("recipes.jsonl"))?);
    let mut stream = Jsonl {
        lines: reader.lines(),
    };
    let mut ctx = ImportCtx {
        stats: ImportStats::default(),
        material_cache: HashMap::new(),
    };
    let created_at = "2026-09-21T00:00:00Z";
    loop {
        let ctx = &mut ctx;
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH_SIZE {
                let Some(rec) = stream.next::<InorganicRecipe>()? else {
                    break;
                };
                import_one_inorganic(
                    conn,
                    ctx,
                    &rec,
                    &precursors_by_recipe,
                    &operations_by_recipe,
                    created_at,
                )?;
                n += 1;
            }
            Ok::<usize, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(ctx.stats)
}

fn import_one_ord(
    conn: &rusqlite::Connection,
    ctx: &mut ImportCtx,
    rxn: &OrdReaction,
    created_at: &str,
) -> Result<(), ImportError> {
    let source = "ord";
    let external_key = format!("ord:{}", rxn.reaction_id);
    ctx.source_mut(source).recipes_seen += 1;
    if store::recipe_id_by_external_key(conn, &external_key)?.is_some() {
        ctx.source_mut(source).recipes_skipped_existing += 1;
        return Ok(());
    }

    let products = parse_smiles_list(&rxn.products_smiles_json);
    let target_material_id = match products.first() {
        Some(smiles) => {
            let m = molecule_material(smiles);
            let identity = m.identity.clone().unwrap();
            let (id, is_new) = ctx.get_or_create_material(conn, &m, &identity)?;
            let s = ctx.source_mut(source);
            if is_new {
                s.materials_created += 1;
            } else {
                s.materials_reused += 1;
            }
            Some(id)
        }
        None => None,
    };

    let prov = provenance("ord", nonblank(&rxn.doi), nonblank(&rxn.dataset_id));
    let provenance_id = store::insert_provenance(conn, &prov)?;
    ctx.source_mut(source).provenance_inserted += 1;

    let mut label = products
        .first()
        .cloned()
        .unwrap_or_else(|| rxn.reaction_id.clone());
    if label.len() > 80 {
        let mut idx = 80;
        while !label.is_char_boundary(idx) {
            idx -= 1;
        }
        label.truncate(idx);
    }
    let recipe = Recipe {
        id: 0,
        name: format!("{label} ({external_key})"),
        version: 1,
        status: RecipeStatus::Draft,
        target_material_id,
        target_quantity: None,
        synthesis_type: Some(match nonblank(&rxn.synthesis_type).as_deref() {
            Some("organic") | None => SynthesisType::Organic,
            Some(other) => SynthesisType::Other(other.to_string()),
        }),
        narrative: nonblank(&rxn.reaction_smiles),
        created_from_recipe_id: None,
        provenance_id: Some(provenance_id),
        created_at: created_at.to_string(),
        created_by: Some(CREATED_BY.to_string()),
        supersedes: None,
        external_key: Some(external_key),
        outcome: None,
        outcome_score: None,
    };
    let recipe_id = store::insert_recipe(conn, &recipe)?;
    ctx.source_mut(source).recipes_inserted += 1;

    let (atmosphere, atmosphere_note) = match nonblank(&rxn.atmosphere) {
        Some(raw) => {
            let (atm, note) = parse_atmosphere(&raw);
            let s = ctx.source_mut(source);
            match &atm {
                Some(Atmosphere::Other(_)) => s.atmosphere_other += 1,
                Some(_) => s.atmosphere_mapped += 1,
                None => s.atmosphere_absent += 1,
            }
            (atm, note)
        }
        None => {
            ctx.source_mut(source).atmosphere_absent += 1;
            (None, None)
        }
    };
    let step = RecipeStep {
        id: 0,
        recipe_id,
        ordering: 1,
        operation: Operation::Other {
            note: "ord_reaction".to_string(),
        },
        parameters: serde_json::json!({}),
        conditions: Conditions {
            temperature: rxn
                .temperature_c
                .map(|t| Temperature::Scalar(Quantity::exact(t, Unit::Celsius))),
            pressure: None,
            duration: rxn.time_h.map(|h| Quantity::exact(h, Unit::Hour)),
            atmosphere,
            ph: None,
            stirring: None,
            atmosphere_note,
        },
    };
    let step_id = store::insert_step(conn, &step)?;
    ctx.source_mut(source).steps_inserted += 1;

    let groups = [
        (&rxn.reactants_smiles_json, MaterialRole::Reactant),
        (
            &rxn.reagents_smiles_json,
            MaterialRole::Other("reagent".to_string()),
        ),
        (&rxn.solvents_smiles_json, MaterialRole::Solvent),
        (&rxn.catalysts_smiles_json, MaterialRole::Catalyst),
    ];
    let mut created = 0u64;
    let mut reused = 0u64;
    for (json, role) in groups {
        for smiles in parse_smiles_list(json) {
            let m = molecule_material(&smiles);
            let identity = m.identity.clone().unwrap();
            let (material_id, is_new) = ctx.get_or_create_material(conn, &m, &identity)?;
            if is_new {
                created += 1;
            } else {
                reused += 1;
            }
            let sm = StepMaterial {
                id: 0,
                step_id,
                material_id,
                role: role.clone(),
                quantity: None,
                equivalents: None,
                is_reference: false,
                optional: false,
                notes: None,
            };
            store::insert_step_material(conn, &sm)?;
            ctx.source_mut(source).step_materials_inserted += 1;
        }
    }
    for smiles in products.iter().skip(1) {
        let m = molecule_material(smiles);
        let identity = m.identity.clone().unwrap();
        let (material_id, is_new) = ctx.get_or_create_material(conn, &m, &identity)?;
        if is_new {
            created += 1;
        } else {
            reused += 1;
        }
        let sm = StepMaterial {
            id: 0,
            step_id,
            material_id,
            role: MaterialRole::Product,
            quantity: None,
            equivalents: None,
            is_reference: false,
            optional: false,
            notes: None,
        };
        store::insert_step_material(conn, &sm)?;
        ctx.source_mut(source).step_materials_inserted += 1;
    }
    let s = ctx.source_mut(source);
    s.materials_created += created;
    s.materials_reused += reused;
    Ok(())
}

/// Import the ORD parquet mirror rows from `ord.jsonl`.
pub fn import_ord(store: &mut Store, dir: &Path) -> Result<ImportStats, ImportError> {
    let reader = BufReader::with_capacity(1 << 20, File::open(dir.join("ord.jsonl"))?);
    let mut stream = Jsonl {
        lines: reader.lines(),
    };
    let mut ctx = ImportCtx {
        stats: ImportStats::default(),
        material_cache: HashMap::new(),
    };
    let created_at = "2026-09-21T00:00:00Z";
    loop {
        let ctx = &mut ctx;
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < BATCH_SIZE {
                let Some(rxn) = stream.next::<OrdReaction>()? else {
                    break;
                };
                import_one_ord(conn, ctx, &rxn, created_at)?;
                n += 1;
            }
            Ok::<usize, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(ctx.stats)
}

impl ImportStats {
    pub fn merged(&self) -> SourceStats {
        let mut total = SourceStats::default();
        for s in self.per_source.values() {
            total.merge(s);
        }
        total
    }
}

// ---------- LLM-extraction pilot loader ----------

/// One recipe as produced by an LLM extraction pass. Conditions and
/// quantities deserialize directly into the stacks-core model types, so all
/// type teeth (units, operators, enum vocabularies) apply at this boundary.
#[derive(Debug, Deserialize)]
pub struct ExtractedRecipe {
    /// `<book>:<section-slug>`, e.g. `brauer:bf3-m1`. Combined with the
    /// source dataset to form the external key.
    pub slug: String,
    /// Page/section locator inside the source file, e.g. `pp. 219-220`.
    pub locator: String,
    pub target_formula: String,
    #[serde(default)]
    pub target_names: Vec<String>,
    /// Honest 0..=1 extraction confidence. Mandatory and validated.
    pub confidence: f64,
    #[serde(default)]
    pub narrative: Option<String>,
    #[serde(default)]
    pub steps: Vec<ExtractedStep>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedStep {
    pub operation: Operation,
    #[serde(default)]
    pub conditions: Conditions,
    /// Free-text apparatus/procedure detail that has no structured home.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub materials: Vec<ExtractedMaterial>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractedMaterial {
    pub formula: String,
    #[serde(default)]
    pub name: Option<String>,
    pub role: MaterialRole,
    #[serde(default)]
    pub quantity: Option<Quantity>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Static description of the extraction source file.
pub struct ExtractSource {
    pub source_dataset: String,
    pub book: String,
    pub path: String,
    pub sha256: String,
    pub extractor_version: String,
}

#[derive(Debug, Default)]
pub struct LoadStats {
    pub inserted: u64,
    pub skipped_existing: u64,
    /// (slug, reason) for every rejected record.
    pub rejected: Vec<(String, String)>,
}

fn validate_extracted(rec: &ExtractedRecipe) -> Result<(), String> {
    if !(0.0..=1.0).contains(&rec.confidence) {
        return Err(format!("confidence {} outside [0, 1]", rec.confidence));
    }
    if rec.locator.trim().is_empty() {
        return Err("missing locator".to_string());
    }
    if rec.slug.trim().is_empty() {
        return Err("missing slug".to_string());
    }
    if rec.target_formula.trim().is_empty() {
        return Err("missing target formula".to_string());
    }
    Ok(())
}

/// Load an LLM-extraction JSONL file into the store. Idempotent via
/// `<source_dataset>:<slug>` external keys.
pub fn load_extracted(
    store: &mut Store,
    jsonl_path: &Path,
    source: &ExtractSource,
    created_at: &str,
) -> Result<LoadStats, ImportError> {
    let reader = BufReader::with_capacity(1 << 20, File::open(jsonl_path)?);
    let mut ctx = ImportCtx {
        stats: ImportStats::default(),
        material_cache: HashMap::new(),
    };
    let mut stats = LoadStats::default();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)?;
        let slug = value
            .get("slug")
            .and_then(|s| s.as_str())
            .unwrap_or("<unknown>")
            .to_string();
        let rec: ExtractedRecipe = match serde_json::from_value(value) {
            Ok(r) => r,
            Err(e) => {
                stats.rejected.push((slug, format!("model boundary: {e}")));
                continue;
            }
        };
        if let Err(reason) = validate_extracted(&rec) {
            stats.rejected.push((rec.slug.clone(), reason));
            continue;
        }
        let external_key = format!("{}:{}", source.source_dataset, rec.slug);
        if store.recipe_id_by_external_key(&external_key)?.is_some() {
            stats.skipped_existing += 1;
            continue;
        }
        store.with_transaction(|conn| {
            let target = formula_material(&rec.target_formula, None);
            let identity = target.identity.clone().unwrap();
            let mut target = target;
            target.names = rec.target_names.clone();
            let (target_id, _) = ctx.get_or_create_material(conn, &target, &identity)?;

            let prov = Provenance {
                id: 0,
                kind: ProvenanceKind::File,
                doi: None,
                source_dataset: Some(source.source_dataset.clone()),
                path: Some(source.path.clone()),
                sha256: Some(source.sha256.clone()),
                locator: Some(rec.locator.clone()),
                extractor_version: Some(source.extractor_version.clone()),
                extraction_method: ExtractionMethod::LlmExtracted,
                confidence: rec.confidence,
                note: None,
            };
            let provenance_id = store::insert_provenance(conn, &prov)?;

            let recipe = Recipe {
                id: 0,
                name: format!("{} ({external_key})", rec.target_formula),
                version: 1,
                status: RecipeStatus::Draft,
                target_material_id: Some(target_id),
                target_quantity: None,
                synthesis_type: None,
                narrative: rec.narrative.clone(),
                created_from_recipe_id: None,
                provenance_id: Some(provenance_id),
                created_at: created_at.to_string(),
                created_by: Some(CREATED_BY.to_string()),
                supersedes: None,
                external_key: Some(external_key),
                outcome: None,
                outcome_score: None,
            };
            let recipe_id = store::insert_recipe(conn, &recipe)?;

            for (i, step) in rec.steps.iter().enumerate() {
                let parameters = match &step.note {
                    Some(note) => serde_json::json!({"note": note}),
                    None => serde_json::json!({}),
                };
                let rs = RecipeStep {
                    id: 0,
                    recipe_id,
                    ordering: i as i64 + 1,
                    operation: step.operation.clone(),
                    parameters,
                    conditions: step.conditions.clone(),
                };
                let step_id = store::insert_step(conn, &rs)?;
                for m in &step.materials {
                    let material = formula_material(&m.formula, m.name.clone());
                    let identity = material.identity.clone().unwrap();
                    let (material_id, _) =
                        ctx.get_or_create_material(conn, &material, &identity)?;
                    let sm = StepMaterial {
                        id: 0,
                        step_id,
                        material_id,
                        role: m.role.clone(),
                        quantity: m.quantity.clone(),
                        equivalents: None,
                        is_reference: false,
                        optional: false,
                        notes: m.notes.clone(),
                    };
                    store::insert_step_material(conn, &sm)?;
                }
            }
            Ok::<_, ImportError>(())
        })?;
        stats.inserted += 1;
    }
    Ok(stats)
}
