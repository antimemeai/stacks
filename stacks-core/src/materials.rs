//! The materials pillar: computational-materials corpora (Materials Project,
//! COD, OQMD, TOP4040) in a sibling database (`data/materials.db`).
//!
//! This wave deliberately keeps sources distinct and queryable
//! (`external_key = <source>:<id>`); cross-source identity by
//! formula+spacegroup is a later refinement (see docs/materials-import-report.md).

use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

/// Property unit vocabulary (CHECK'd). Escape hatch `other:<unit>`.
pub const PROPERTY_UNITS: &[&str] = &[
    "eV", "eV/atom", "g/cm3", "GPa", "µB", "1", "%", "S/cm", "pC/N", "µC/cm2", "V", "mA·h/g",
];

const MATERIALS_SCHEMA_V1: &str = r#"
CREATE TABLE material_entry (
    external_key TEXT PRIMARY KEY,
    source TEXT NOT NULL CHECK (source IN ('mp','cod','oqmd','top')),
    source_id TEXT NOT NULL,
    formula TEXT,
    elements TEXT,
    nsites INTEGER,
    spacegroup TEXT,
    spacegroup_number INTEGER,
    crystal_system TEXT,
    cell_json TEXT,
    structure_json TEXT,
    density REAL,
    description TEXT,
    reference_path TEXT,
    imported_at TEXT NOT NULL,
    UNIQUE (source, source_id)
);

CREATE TABLE property (
    id INTEGER PRIMARY KEY,
    external_key TEXT NOT NULL REFERENCES material_entry (external_key) ON DELETE CASCADE,
    collection TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (length(kind) > 0),
    value REAL,
    unit TEXT CHECK (unit IS NULL OR unit IN ('eV','eV/atom','g/cm3','GPa','µB','1','%','S/cm','pC/N','µC/cm2','V','mA·h/g') OR unit LIKE 'other:%'),
    text_value TEXT,
    extra_json TEXT,
    UNIQUE (external_key, collection, kind, value, text_value)
);
CREATE INDEX property_key ON property (external_key);
CREATE INDEX property_kind ON property (kind);

-- FTS over formula + robocrys descriptions (external content).
CREATE VIRTUAL TABLE material_fts USING fts5(formula, description, content=material_entry, content_rowid=rowid);

-- Parse failures land here with the reason; the entry is imported with what
-- parsed cleanly (or skipped entirely when nothing did).
CREATE TABLE material_quarantine (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    source_id TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('parse_failure','empty_payload')),
    detail TEXT,
    UNIQUE (source, source_id, reason)
);

CREATE TABLE change_log (
    id INTEGER PRIMARY KEY,
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    at TEXT NOT NULL,
    by TEXT NOT NULL,
    patch_json TEXT NOT NULL
);

CREATE TABLE import_meta (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_mtime TEXT,
    imported_at TEXT NOT NULL,
    row_counts TEXT NOT NULL
);
"#;

const MATERIALS_SCHEMA_V2: &str = r#"
-- UNIQUE with NULLs never dedupes (NULL != NULL), so the real idempotency
-- fence for properties is this expression index.
CREATE UNIQUE INDEX IF NOT EXISTS property_dedupe ON property
    (external_key, collection, kind, COALESCE(value, -1e308), COALESCE(text_value, ''));
"#;

/// One material record from a computational corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MaterialEntry {
    pub external_key: String,
    pub source: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nsites: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacegroup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacegroup_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crystal_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<serde_json::Value>,
    /// Full structure JSON, only when small (< 64 KB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<f64>,
    /// Robocrys natural-language description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// In-place payload reference (archive path + member).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_path: Option<String>,
    #[serde(default)]
    pub imported_at: String,
}

/// One unit-disciplined property row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Property {
    #[serde(default)]
    pub id: i64,
    pub external_key: String,
    /// Source collection (idempotency key component).
    #[serde(default)]
    pub collection: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Closed vocabulary (`PROPERTY_UNITS`) plus `other:<unit>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

pub struct MaterialsStore {
    conn: Connection,
}

impl MaterialsStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.configure_rw()?;
        store.migrate()?;
        Ok(store)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "query_only", "ON")?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.configure_rw()?;
        store.migrate()?;
        Ok(store)
    }

    fn configure_rw(&self) -> Result<(), StoreError> {
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.pragma_update(None, "busy_timeout", 5000)?;
        Ok(())
    }

    pub fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );",
        )?;
        for (version, sql) in [(1, MATERIALS_SCHEMA_V1), (2, MATERIALS_SCHEMA_V2)] {
            let applied: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |r| r.get(0),
            )?;
            if applied {
                continue;
            }
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql).map_err(|e| StoreError::Migration {
                version,
                reason: e.to_string(),
            })?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn raw(&self) -> &Connection {
        &self.conn
    }

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
}

/// Returns true when a row was inserted (INSERT OR IGNORE on external_key).
pub fn insert_entry(conn: &Connection, e: &MaterialEntry) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO material_entry (external_key, source, source_id, formula, elements,
            nsites, spacegroup, spacegroup_number, crystal_system, cell_json, structure_json,
            density, description, reference_path, imported_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            e.external_key,
            e.source,
            e.source_id,
            e.formula,
            e.elements.as_ref().map(serde_json::to_string).transpose()?,
            e.nsites,
            e.spacegroup,
            e.spacegroup_number,
            e.crystal_system,
            e.cell.as_ref().map(serde_json::to_string).transpose()?,
            e.structure
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            e.density,
            e.description,
            e.reference_path,
            e.imported_at,
        ],
    )?;
    Ok(n > 0)
}

/// Properties dedupe via UNIQUE(external_key, collection, kind, value,
/// text_value); returns true when a row was actually inserted.
pub fn insert_property(conn: &Connection, p: &Property) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO property (external_key, collection, kind, value, unit, text_value, extra_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            p.external_key,
            p.collection,
            p.kind,
            p.value,
            p.unit,
            p.text_value,
            p.extra.as_ref().map(serde_json::to_string).transpose()?,
        ],
    )?;
    Ok(n > 0)
}

pub fn insert_material_quarantine(
    conn: &Connection,
    source: &str,
    source_id: &str,
    reason: &str,
    detail: Option<&str>,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT OR IGNORE INTO material_quarantine (source, source_id, reason, detail)
         VALUES (?1,?2,?3,?4)",
        params![source, source_id, reason, detail],
    )?;
    Ok(())
}

pub fn entry_exists(conn: &Connection, external_key: &str) -> Result<bool, StoreError> {
    conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM material_entry WHERE external_key = ?1)")?
        .query_row([external_key], |r| r.get(0))
        .map_err(Into::into)
}

pub fn insert_materials_meta(
    conn: &Connection,
    source: &str,
    source_path: &str,
    source_mtime: Option<&str>,
    imported_at: &str,
    row_counts: &serde_json::Value,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO import_meta (source, source_path, source_mtime, imported_at, row_counts)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            source,
            source_path,
            source_mtime,
            imported_at,
            serde_json::to_string(row_counts)?,
        ],
    )?;
    Ok(())
}
