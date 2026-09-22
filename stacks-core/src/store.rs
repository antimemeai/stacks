use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::StoreError;
use crate::model::*;
use crate::quantity::{Operator, Quantity, Unit};

/// SQL CHECK mirrors of the Rust-side closed vocabularies — the second
/// fence behind the types. Kept in one place so the duplication is auditable.
const UNIT_CHECK: &str = "('g','kg','mg','mL','L','µL','mol','mmol','°C','K','atm','bar','Pa','mbar','Torr','s','min','h','d')";
const OPERATOR_CHECK: &str = "('eq','approx','lt','gt','lte','gte','range')";

const SCHEMA_V1: &str = r#"
CREATE TABLE material (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('molecule','formula','mixture') OR kind LIKE 'other:%'),
    inchikey TEXT,
    canonical_smiles TEXT,
    formula TEXT,
    composition_json TEXT,
    names_json TEXT NOT NULL,
    cas TEXT
);

CREATE TABLE provenance (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('dataset_import','doi','file','manual')),
    doi TEXT,
    source_dataset TEXT,
    path TEXT,
    sha256 TEXT,
    locator TEXT,
    extractor_version TEXT,
    extraction_method TEXT NOT NULL CHECK (extraction_method IN ('structured','parsed','llm_extracted','manual')),
    confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

CREATE TABLE recipe (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('draft','deployed','retired')),
    target_material_id INTEGER REFERENCES material(id),
    target_value REAL,
    target_unit TEXT,
    target_operator TEXT,
    target_range_min REAL,
    target_range_max REAL,
    synthesis_type TEXT CHECK (synthesis_type IS NULL OR synthesis_type IN ('solid_state','solution_based','organic') OR synthesis_type LIKE 'other:%'),
    narrative TEXT,
    created_from_recipe_id INTEGER REFERENCES recipe(id),
    provenance_id INTEGER REFERENCES provenance(id),
    created_at TEXT NOT NULL,
    created_by TEXT,
    supersedes INTEGER REFERENCES recipe(id),
    UNIQUE (name, version),
    CHECK (target_unit IS NULL OR target_unit IN UNIT_CHECK_TOKENS OR target_unit LIKE 'other:%'),
    CHECK (target_operator IS NULL OR target_operator IN OPERATOR_CHECK_TOKENS),
    CHECK (target_operator IS NULL
        OR (target_operator = 'range' AND target_range_min IS NOT NULL AND target_range_max IS NOT NULL)
        OR (target_operator <> 'range' AND target_range_min IS NULL AND target_range_max IS NULL))
);

CREATE TABLE recipe_step (
    id INTEGER PRIMARY KEY,
    recipe_id INTEGER NOT NULL REFERENCES recipe(id) ON DELETE CASCADE,
    ordering INTEGER NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('heat','mix','grind','dissolve','precipitate','filter','wash','dry','calcine','mill','sonicate') OR operation LIKE 'other:%'),
    parameters_json TEXT NOT NULL DEFAULT '{}',
    conditions_json TEXT NOT NULL DEFAULT '{}',
    -- Wires deferred: DAG edges reserved, unused until plans arrive.
    wire_from_step_id INTEGER REFERENCES recipe_step(id),
    wire_from_output TEXT,
    wire_to_input TEXT
);

CREATE TABLE step_material (
    id INTEGER PRIMARY KEY,
    step_id INTEGER NOT NULL REFERENCES recipe_step(id) ON DELETE CASCADE,
    material_id INTEGER NOT NULL REFERENCES material(id),
    role TEXT NOT NULL CHECK (role IN ('reactant','precursor','solvent','catalyst','product','byproduct','wash') OR role LIKE 'other:%'),
    value REAL,
    unit TEXT,
    operator TEXT,
    range_min REAL,
    range_max REAL,
    equivalents REAL,
    is_reference INTEGER NOT NULL DEFAULT 0 CHECK (is_reference IN (0, 1)),
    optional INTEGER NOT NULL DEFAULT 0 CHECK (optional IN (0, 1)),
    notes TEXT,
    CHECK (unit IS NULL OR unit IN UNIT_CHECK_TOKENS OR unit LIKE 'other:%'),
    CHECK (operator IS NULL OR operator IN OPERATOR_CHECK_TOKENS),
    CHECK (operator IS NULL
        OR (operator = 'range' AND range_min IS NOT NULL AND range_max IS NOT NULL)
        OR (operator <> 'range' AND range_min IS NULL AND range_max IS NULL)),
    CHECK ((value IS NULL) = (unit IS NULL))
);

CREATE TABLE run (
    id INTEGER PRIMARY KEY,
    recipe_id INTEGER NOT NULL REFERENCES recipe(id),
    recipe_version INTEGER NOT NULL,
    started_at TEXT,
    operator TEXT,
    yield_value REAL,
    yield_unit TEXT CHECK (yield_unit IS NULL OR yield_unit IN UNIT_CHECK_TOKENS OR yield_unit LIKE 'other:%'),
    conversion REAL,
    purity REAL,
    observation TEXT,
    CHECK ((yield_value IS NULL) = (yield_unit IS NULL))
);

CREATE TABLE run_step (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES run(id) ON DELETE CASCADE,
    recipe_step_id INTEGER REFERENCES recipe_step(id),
    ordering INTEGER NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('heat','mix','grind','dissolve','precipitate','filter','wash','dry','calcine','mill','sonicate') OR operation LIKE 'other:%'),
    actual_parameters_json TEXT NOT NULL DEFAULT '{}',
    actual_conditions_json TEXT NOT NULL DEFAULT '{}',
    started_at TEXT,
    ended_at TEXT,
    deviation_notes TEXT
);

CREATE TABLE change_log (
    id INTEGER PRIMARY KEY,
    entity_kind TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    at TEXT NOT NULL,
    by TEXT NOT NULL,
    patch_json TEXT NOT NULL
);
"#;

const SCHEMA_V2: &str = r#"
ALTER TABLE material ADD COLUMN identity TEXT;
ALTER TABLE recipe ADD COLUMN external_key TEXT;
CREATE UNIQUE INDEX material_kind_identity ON material (kind, identity) WHERE identity IS NOT NULL;
CREATE UNIQUE INDEX recipe_external_key ON recipe (external_key) WHERE external_key IS NOT NULL;
"#;

const SCHEMA_V3: &str = r#"
CREATE INDEX recipe_step_recipe_id ON recipe_step (recipe_id);
CREATE INDEX step_material_step_id ON step_material (step_id);
CREATE INDEX step_material_material_id ON step_material (material_id);
CREATE INDEX recipe_target_material_id ON recipe (target_material_id);
CREATE INDEX recipe_provenance_id ON recipe (provenance_id);
CREATE INDEX run_step_run_id ON run_step (run_id);
"#;

const SCHEMA_V4: &str = r#"
ALTER TABLE recipe ADD COLUMN outcome TEXT CHECK (outcome IS NULL OR outcome IN ('success','partial','failed'));
ALTER TABLE recipe ADD COLUMN outcome_score REAL;
ALTER TABLE provenance ADD COLUMN note TEXT;
"#;

const SCHEMA_V5: &str = r#"
-- Extend the operation CHECK vocabulary with 'hydrothermal'. SQLite cannot
-- alter CHECK constraints, so rebuild the two tables carrying it.
CREATE TABLE recipe_step_new (
    id INTEGER PRIMARY KEY,
    recipe_id INTEGER NOT NULL REFERENCES recipe(id) ON DELETE CASCADE,
    ordering INTEGER NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('heat','mix','grind','dissolve','precipitate','filter','wash','dry','calcine','mill','sonicate','hydrothermal') OR operation LIKE 'other:%'),
    parameters_json TEXT NOT NULL DEFAULT '{}',
    conditions_json TEXT NOT NULL DEFAULT '{}',
    wire_from_step_id INTEGER REFERENCES recipe_step_new(id),
    wire_from_output TEXT,
    wire_to_input TEXT
);
INSERT INTO recipe_step_new (id, recipe_id, ordering, operation, parameters_json, conditions_json, wire_from_step_id, wire_from_output, wire_to_input)
    SELECT id, recipe_id, ordering, operation, parameters_json, conditions_json, wire_from_step_id, wire_from_output, wire_to_input FROM recipe_step;
DROP TABLE recipe_step;
ALTER TABLE recipe_step_new RENAME TO recipe_step;
CREATE INDEX recipe_step_recipe_id ON recipe_step (recipe_id);

CREATE TABLE run_step_new (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES run(id) ON DELETE CASCADE,
    recipe_step_id INTEGER REFERENCES recipe_step(id),
    ordering INTEGER NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('heat','mix','grind','dissolve','precipitate','filter','wash','dry','calcine','mill','sonicate','hydrothermal') OR operation LIKE 'other:%'),
    actual_parameters_json TEXT NOT NULL DEFAULT '{}',
    actual_conditions_json TEXT NOT NULL DEFAULT '{}',
    started_at TEXT,
    ended_at TEXT,
    deviation_notes TEXT
);
INSERT INTO run_step_new SELECT * FROM run_step;
DROP TABLE run_step;
ALTER TABLE run_step_new RENAME TO run_step;
CREATE INDEX run_step_run_id ON run_step (run_id);
"#;

fn schema_v1() -> String {
    SCHEMA_V1
        .replace("UNIT_CHECK_TOKENS", UNIT_CHECK)
        .replace("OPERATOR_CHECK_TOKENS", OPERATOR_CHECK)
}

/// A recipe with its provenance and fully populated step tree, as read back
/// from the store.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeBundle {
    pub recipe: Recipe,
    pub provenance: Option<Provenance>,
    pub steps: Vec<StepBundle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepBundle {
    pub step: RecipeStep,
    pub materials: Vec<StepMaterial>,
}

/// Handle to the SQLite system of record (WAL mode, foreign keys on).
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating if needed) a database file and apply pending
    /// migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    /// Open an existing database read-only (`PRAGMA query_only=ON`), no
    /// migrations. This is the API's access mode: the record file is never
    /// written by readers.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "query_only", "ON")?;
        Ok(Self { conn })
    }

    /// In-memory store, for tests and scratch work.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    fn configure(&self) -> Result<(), StoreError> {
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.pragma_update(None, "busy_timeout", 5000)?;
        Ok(())
    }

    /// Apply pending numbered migrations. Idempotent: a migration recorded in
    /// `schema_migrations` is never re-run.
    pub fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );",
        )?;
        for (version, sql) in [
            (1, schema_v1()),
            (2, SCHEMA_V2.to_string()),
            (3, SCHEMA_V3.to_string()),
            (4, SCHEMA_V4.to_string()),
            (5, SCHEMA_V5.to_string()),
        ] {
            let applied: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |r| r.get(0),
            )?;
            if applied {
                continue;
            }
            if version == 5 {
                self.conn.pragma_update(None, "foreign_keys", "OFF")?;
            }
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(&sql).map_err(|e| StoreError::Migration {
                version,
                reason: e.to_string(),
            })?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )?;
            tx.commit()?;
            if version == 5 {
                self.conn.pragma_update(None, "foreign_keys", "ON")?;
            }
        }
        Ok(())
    }

    /// Highest applied migration version (0 before any migration).
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        let v: Option<i64> = self
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap_or(Some(0));
        Ok(v.unwrap_or(0))
    }

    /// Raw access for boundary tests and future low-level tooling.
    pub fn raw(&self) -> &Connection {
        &self.conn
    }

    pub fn insert_material(&self, m: &Material) -> Result<i64, StoreError> {
        insert_material(&self.conn, m)
    }

    pub fn insert_provenance(&self, p: &Provenance) -> Result<i64, StoreError> {
        insert_provenance(&self.conn, p)
    }

    pub fn insert_recipe(&self, r: &Recipe) -> Result<i64, StoreError> {
        insert_recipe(&self.conn, r)
    }

    pub fn insert_step(&self, s: &RecipeStep) -> Result<i64, StoreError> {
        insert_step(&self.conn, s)
    }

    pub fn insert_step_material(&self, sm: &StepMaterial) -> Result<i64, StoreError> {
        insert_step_material(&self.conn, sm)
    }

    pub fn insert_run(&self, r: &Run) -> Result<i64, StoreError> {
        insert_run(&self.conn, r)
    }

    pub fn insert_run_step(&self, s: &RunStep) -> Result<i64, StoreError> {
        insert_run_step(&self.conn, s)
    }

    pub fn append_change_log(&self, entry: &ChangeLogEntry) -> Result<i64, StoreError> {
        append_change_log(&self.conn, entry)
    }

    pub fn material_id_by_identity(
        &self,
        kind: &MaterialKind,
        identity: &str,
    ) -> Result<Option<i64>, StoreError> {
        material_id_by_identity(&self.conn, kind, identity)
    }

    pub fn recipe_id_by_external_key(&self, key: &str) -> Result<Option<i64>, StoreError> {
        recipe_id_by_external_key(&self.conn, key)
    }

    /// Run `f` inside a single transaction; commit on success, roll back on
    /// error. Bulk importers use this for batched commits.
    pub fn with_transaction<T, E>(
        &mut self,
        f: impl FnOnce(&Connection) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<StoreError>,
    {
        let tx = self.conn.transaction().map_err(|e| E::from(e.into()))?;
        let out = f(&tx)?;
        tx.commit().map_err(|e| E::from(e.into()))?;
        Ok(out)
    }

    pub fn get_material(&self, id: i64) -> Result<Option<Material>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, kind, inchikey, canonical_smiles, formula, composition_json, names_json, cas, identity
                 FROM material WHERE id = ?1",
                [id],
                |row| {
                    Ok(Material {
                        id: row.get(0)?,
                        kind: MaterialKind::from_db_token(&row.get::<_, String>(1)?)
                            .map_err(corrupt)?,
                        inchikey: row.get(2)?,
                        canonical_smiles: row.get(3)?,
                        formula: row.get(4)?,
                        composition: row
                            .get::<_, Option<String>>(5)?
                            .map(|s| serde_json::from_str(&s))
                            .transpose()
                            .map_err(corrupt)?,
                        names: serde_json::from_str(&row.get::<_, String>(6)?)
                            .map_err(corrupt)?,
                        cas: row.get(7)?,
                        identity: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_provenance(&self, id: i64) -> Result<Option<Provenance>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, kind, doi, source_dataset, path, sha256, locator, extractor_version, extraction_method, confidence, note
                 FROM provenance WHERE id = ?1",
                [id],
                |row| {
                    Ok(Provenance {
                        id: row.get(0)?,
                        kind: ProvenanceKind::from_db_token(&row.get::<_, String>(1)?)
                            .map_err(corrupt)?,
                        doi: row.get(2)?,
                        source_dataset: row.get(3)?,
                        path: row.get(4)?,
                        sha256: row.get(5)?,
                        locator: row.get(6)?,
                        extractor_version: row.get(7)?,
                        extraction_method: ExtractionMethod::from_db_token(
                            &row.get::<_, String>(8)?,
                        )
                        .map_err(corrupt)?,
                        confidence: row.get(9)?,
                        note: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Load a recipe with its provenance, steps (ordered), and each step's
    /// materials.
    pub fn load_recipe(&self, id: i64) -> Result<Option<RecipeBundle>, StoreError> {
        let recipe = self
            .conn
            .query_row(
                "SELECT id, name, version, status, target_material_id,
                    target_value, target_unit, target_operator, target_range_min, target_range_max,
                    synthesis_type, narrative, created_from_recipe_id, provenance_id,
                    created_at, created_by, supersedes, external_key, outcome, outcome_score
                 FROM recipe WHERE id = ?1",
                [id],
                |row| {
                    Ok(Recipe {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        version: row.get(2)?,
                        status: RecipeStatus::from_db_token(&row.get::<_, String>(3)?)
                            .map_err(corrupt)?,
                        target_material_id: row.get(4)?,
                        target_quantity: read_quantity(row, 5, 6, 7, 8, 9)?,
                        synthesis_type: row
                            .get::<_, Option<String>>(10)?
                            .map(|t| SynthesisType::from_db_token(&t))
                            .transpose()
                            .map_err(corrupt)?,
                        narrative: row.get(11)?,
                        created_from_recipe_id: row.get(12)?,
                        provenance_id: row.get(13)?,
                        created_at: row.get(14)?,
                        created_by: row.get(15)?,
                        supersedes: row.get(16)?,
                        external_key: row.get(17)?,
                        outcome: row
                            .get::<_, Option<String>>(18)?
                            .map(|t| Outcome::from_db_token(&t))
                            .transpose()
                            .map_err(corrupt)?,
                        outcome_score: row.get(19)?,
                    })
                },
            )
            .optional()?;
        let Some(recipe) = recipe else {
            return Ok(None);
        };

        let provenance = match recipe.provenance_id {
            Some(pid) => self.get_provenance(pid)?,
            None => None,
        };

        let mut step_stmt = self.conn.prepare(
            "SELECT id, recipe_id, ordering, operation, parameters_json, conditions_json
             FROM recipe_step WHERE recipe_id = ?1 ORDER BY ordering",
        )?;
        let steps = step_stmt
            .query_map([recipe.id], |row| {
                Ok(RecipeStep {
                    id: row.get(0)?,
                    recipe_id: row.get(1)?,
                    ordering: row.get(2)?,
                    operation: Operation::from_db_token(&row.get::<_, String>(3)?)
                        .map_err(corrupt)?,
                    parameters: serde_json::from_str(&row.get::<_, String>(4)?).map_err(corrupt)?,
                    conditions: serde_json::from_str(&row.get::<_, String>(5)?).map_err(corrupt)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut mat_stmt = self.conn.prepare(
            "SELECT id, step_id, material_id, role, value, unit, operator, range_min, range_max,
                equivalents, is_reference, optional, notes
             FROM step_material WHERE step_id = ?1 ORDER BY id",
        )?;
        let mut bundles = Vec::with_capacity(steps.len());
        for step in steps {
            let materials = mat_stmt
                .query_map([step.id], |row| {
                    Ok(StepMaterial {
                        id: row.get(0)?,
                        step_id: row.get(1)?,
                        material_id: row.get(2)?,
                        role: MaterialRole::from_db_token(&row.get::<_, String>(3)?)
                            .map_err(corrupt)?,
                        quantity: read_quantity(row, 4, 5, 6, 7, 8)?,
                        equivalents: row.get(9)?,
                        is_reference: row.get(10)?,
                        optional: row.get(11)?,
                        notes: row.get(12)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            bundles.push(StepBundle { step, materials });
        }

        Ok(Some(RecipeBundle {
            recipe,
            provenance,
            steps: bundles,
        }))
    }
}

/// Connection-level insert: the form bulk importers call inside
/// [`Store::with_transaction`] batches.
pub fn insert_material(conn: &Connection, m: &Material) -> Result<i64, StoreError> {
    let sql = "INSERT INTO material (kind, inchikey, canonical_smiles, formula, composition_json, names_json, cas, identity)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
    conn.prepare_cached(sql)?.execute(params![
        m.kind.as_db_token(),
        m.inchikey,
        m.canonical_smiles,
        m.formula,
        m.composition
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?,
        serde_json::to_string(&m.names)?,
        m.cas,
        m.identity,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_provenance(conn: &Connection, p: &Provenance) -> Result<i64, StoreError> {
    let sql = "INSERT INTO provenance (kind, doi, source_dataset, path, sha256, locator, extractor_version, extraction_method, confidence, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
    conn.prepare_cached(sql)?.execute(params![
        p.kind.as_db_token(),
        p.doi,
        p.source_dataset,
        p.path,
        p.sha256,
        p.locator,
        p.extractor_version,
        p.extraction_method.as_db_token(),
        p.confidence,
        p.note,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_recipe(conn: &Connection, r: &Recipe) -> Result<i64, StoreError> {
    let (value, unit, operator, range_min, range_max) = quantity_parts(&r.target_quantity);
    let sql = "INSERT INTO recipe (name, version, status, target_material_id,
            target_value, target_unit, target_operator, target_range_min, target_range_max,
            synthesis_type, narrative, created_from_recipe_id, provenance_id,
            created_at, created_by, supersedes, external_key, outcome, outcome_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)";
    conn.prepare_cached(sql)?.execute(params![
        r.name,
        r.version,
        r.status.as_db_token(),
        r.target_material_id,
        value,
        unit,
        operator,
        range_min,
        range_max,
        r.synthesis_type.as_ref().map(SynthesisType::as_db_token),
        r.narrative,
        r.created_from_recipe_id,
        r.provenance_id,
        r.created_at,
        r.created_by,
        r.supersedes,
        r.external_key,
        r.outcome.as_ref().map(Outcome::as_db_token),
        r.outcome_score,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_step(conn: &Connection, s: &RecipeStep) -> Result<i64, StoreError> {
    let sql =
        "INSERT INTO recipe_step (recipe_id, ordering, operation, parameters_json, conditions_json)
         VALUES (?1, ?2, ?3, ?4, ?5)";
    conn.prepare_cached(sql)?.execute(params![
        s.recipe_id,
        s.ordering,
        s.operation.as_db_token(),
        serde_json::to_string(&s.parameters)?,
        serde_json::to_string(&s.conditions)?,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_step_material(conn: &Connection, sm: &StepMaterial) -> Result<i64, StoreError> {
    let (value, unit, operator, range_min, range_max) = quantity_parts(&sm.quantity);
    let sql = "INSERT INTO step_material (step_id, material_id, role, value, unit, operator,
            range_min, range_max, equivalents, is_reference, optional, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)";
    conn.prepare_cached(sql)?.execute(params![
        sm.step_id,
        sm.material_id,
        sm.role.as_db_token(),
        value,
        unit,
        operator,
        range_min,
        range_max,
        sm.equivalents,
        sm.is_reference,
        sm.optional,
        sm.notes,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_run(conn: &Connection, r: &Run) -> Result<i64, StoreError> {
    let (yield_value, yield_unit) = match &r.yield_quantity {
        Some(q) => (Some(q.value), Some(q.unit.as_token())),
        None => (None, None),
    };
    let sql = "INSERT INTO run (recipe_id, recipe_version, started_at, operator,
            yield_value, yield_unit, conversion, purity, observation)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    conn.prepare_cached(sql)?.execute(params![
        r.recipe_id,
        r.recipe_version,
        r.started_at,
        r.operator,
        yield_value,
        yield_unit,
        r.conversion,
        r.purity,
        r.observation,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_run_step(conn: &Connection, s: &RunStep) -> Result<i64, StoreError> {
    let sql = "INSERT INTO run_step (run_id, recipe_step_id, ordering, operation,
            actual_parameters_json, actual_conditions_json, started_at, ended_at, deviation_notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    conn.prepare_cached(sql)?.execute(params![
        s.run_id,
        s.recipe_step_id,
        s.ordering,
        s.operation.as_db_token(),
        serde_json::to_string(&s.actual_parameters)?,
        serde_json::to_string(&s.actual_conditions)?,
        s.started_at,
        s.ended_at,
        s.deviation_notes,
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn append_change_log(conn: &Connection, entry: &ChangeLogEntry) -> Result<i64, StoreError> {
    let sql = "INSERT INTO change_log (entity_kind, entity_id, at, by, patch_json)
         VALUES (?1, ?2, ?3, ?4, ?5)";
    conn.prepare_cached(sql)?.execute(params![
        entry.entity_kind,
        entry.entity_id,
        entry.at,
        entry.by,
        serde_json::to_string(&entry.patch)?,
    ])?;
    Ok(conn.last_insert_rowid())
}

/// Lookup by dedup key (the `(kind, identity)` partial unique index).
pub fn material_id_by_identity(
    conn: &Connection,
    kind: &MaterialKind,
    identity: &str,
) -> Result<Option<i64>, StoreError> {
    conn.prepare_cached("SELECT id FROM material WHERE kind = ?1 AND identity = ?2")?
        .query_row(params![kind.as_db_token(), identity], |r| r.get(0))
        .optional()
        .map_err(Into::into)
}

/// Lookup by deterministic import key (the `external_key` partial unique
/// index).
pub fn recipe_id_by_external_key(conn: &Connection, key: &str) -> Result<Option<i64>, StoreError> {
    conn.prepare_cached("SELECT id FROM recipe WHERE external_key = ?1")?
        .query_row([key], |r| r.get(0))
        .optional()
        .map_err(Into::into)
}

fn corrupt(e: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(StoreError::CorruptRow(e.to_string())),
    )
}

type QuantityParts = (
    Option<f64>,
    Option<String>,
    Option<&'static str>,
    Option<f64>,
    Option<f64>,
);

fn quantity_parts(q: &Option<Quantity>) -> QuantityParts {
    match q {
        Some(q) => {
            let (op, range_min, range_max) = match &q.operator {
                Operator::Range { min, max } => ("range", Some(*min), Some(*max)),
                simple => (simple.db_token(), None, None),
            };
            (
                Some(q.value),
                Some(q.unit.as_token()),
                Some(op),
                range_min,
                range_max,
            )
        }
        None => (None, None, None, None, None),
    }
}

fn read_quantity(
    row: &rusqlite::Row,
    value: usize,
    unit: usize,
    operator: usize,
    range_min: usize,
    range_max: usize,
) -> Result<Option<Quantity>, rusqlite::Error> {
    let (Some(value), Some(unit), Some(op)) = (
        row.get::<_, Option<f64>>(value)?,
        row.get::<_, Option<String>>(unit)?,
        row.get::<_, Option<String>>(operator)?,
    ) else {
        return Ok(None);
    };
    let operator = match op.as_str() {
        "eq" => Operator::Eq,
        "approx" => Operator::Approx,
        "lt" => Operator::Lt,
        "gt" => Operator::Gt,
        "lte" => Operator::Lte,
        "gte" => Operator::Gte,
        "range" => Operator::Range {
            min: row
                .get::<_, Option<f64>>(range_min)?
                .ok_or_else(|| corrupt("range operator without bounds"))?,
            max: row
                .get::<_, Option<f64>>(range_max)?
                .ok_or_else(|| corrupt("range operator without bounds"))?,
        },
        other => return Err(corrupt(format!("unknown operator token {other:?}"))),
    };
    Ok(Some(Quantity {
        value,
        unit: Unit::from_token(&unit).map_err(corrupt)?,
        operator,
    }))
}
