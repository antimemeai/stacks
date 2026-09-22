//! The library pillar: neurotic_library's document catalog + chunk corpora,
//! migrated into a sibling database (`data/library.db`). Deliberately
//! separate from the recipe system of record — different domain, different
//! access patterns (FTS5), same teeth conventions.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

const LIBRARY_SCHEMA_V1: &str = r#"
CREATE TABLE paper (
    sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    filename TEXT NOT NULL,
    path TEXT,
    size_bytes INTEGER,
    registered_at TEXT,
    on_disk INTEGER CHECK (on_disk IS NULL OR on_disk IN (0, 1)),
    doi TEXT,
    arxiv_id TEXT,
    title TEXT,
    authors TEXT,
    year INTEGER,
    abstract TEXT,
    journal TEXT,
    source_url TEXT,
    access TEXT,
    blob_key TEXT,
    blob_synced_at TEXT,
    subfield TEXT,
    tags TEXT,
    original_language TEXT,
    original_script_title TEXT,
    transliterated_title TEXT,
    translation_of TEXT,
    translated_in TEXT,
    soviet_stratum TEXT,
    source_collection TEXT
);

CREATE TABLE paper_enrichment (
    -- No FK: the frozen source contains 1,089 enrichments whose papers are
    -- absent from the catalog; migration preserves them verbatim.
    sha256 TEXT PRIMARY KEY,
    openalex_id TEXT,
    openalex_topics TEXT,
    openalex_concepts TEXT,
    openalex_cited_by INTEGER,
    s2_paper_id TEXT,
    s2_tldr TEXT,
    s2_fields_of_study TEXT,
    s2_influential_citation_count INTEGER,
    unpaywall_oa_status TEXT,
    unpaywall_oa_url TEXT,
    enriched_at TEXT
);

-- Append-only path reconciliation history, imported verbatim. No FK:
-- 100 movements reference sha256s absent from the frozen catalog; history
-- is preserved as-is rather than trimmed.
CREATE TABLE paper_movement (
    id INTEGER PRIMARY KEY,
    sha256 TEXT NOT NULL,
    from_path TEXT,
    to_path TEXT,
    moved_at TEXT,
    reason TEXT,
    UNIQUE (sha256, from_path, to_path, moved_at)
);

CREATE TABLE chunk (
    corpus TEXT NOT NULL CHECK (length(corpus) > 0),
    chunk_id INTEGER NOT NULL,
    sha256 TEXT REFERENCES paper (sha256),
    filename TEXT NOT NULL,
    title TEXT,
    section TEXT,
    text TEXT NOT NULL,
    word_count INTEGER,
    embedding BLOB CHECK (embedding IS NULL OR length(embedding) = 384 * 4),
    UNIQUE (corpus, chunk_id)
);

-- External-content FTS5: the index lives here, the text lives only in
-- `chunk` (no duplication of the ~GB-scale text column). The index is built
-- once after bulk import ('rebuild') since the DB is append-only for now.
CREATE VIRTUAL TABLE chunk_fts USING fts5(text, content=chunk, content_rowid=rowid);

-- Chunks whose filename joined to >1 catalog row (and survived no
-- disambiguation) or to none. The chunk row itself is still imported with
-- sha256 NULL — absence of the join is data.
CREATE TABLE chunk_quarantine (
    id INTEGER PRIMARY KEY,
    corpus TEXT NOT NULL,
    chunk_id INTEGER NOT NULL,
    filename TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('filename_collision', 'orphan')),
    candidates_json TEXT,
    UNIQUE (corpus, chunk_id, reason)
);

CREATE TABLE change_log (
    id INTEGER PRIMARY KEY,
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    at TEXT NOT NULL,
    by TEXT NOT NULL,
    patch_json TEXT NOT NULL
);

-- One row per import run: provenance for the migration itself.
CREATE TABLE import_meta (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_mtime TEXT,
    imported_at TEXT NOT NULL,
    row_counts TEXT NOT NULL
);
"#;

/// One catalog row. `sha256` is identity; `filename` is *not* unique in the
/// source catalog (63 collisions) and `path` is reconciled through
/// `paper_movement` after import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LibraryPaper {
    pub sha256: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_disk: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(default, rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_synced_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subfield: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_script_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transliterated_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soviet_stratum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_collection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PaperEnrichment {
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_topics: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_concepts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openalex_cited_by: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s2_paper_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s2_tldr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s2_fields_of_study: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s2_influential_citation_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpaywall_oa_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpaywall_oa_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enriched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PaperMovement {
    pub sha256: String,
    #[serde(default)]
    pub from_path: Option<String>,
    #[serde(default)]
    pub to_path: Option<String>,
    #[serde(default)]
    pub moved_at: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// One text chunk. `rowid` is the DB key used for keyset pagination and the
/// FTS index; `(corpus, chunk_id)` is the idempotency key from the source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LibraryChunk {
    #[serde(default)]
    pub rowid: i64,
    pub corpus: String,
    pub chunk_id: i64,
    /// NULL when the filename join quarantined or orphaned the chunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_count: Option<i64>,
    /// Embedding blobs are stored but never serialized over the API.
    #[serde(skip)]
    pub embedding: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineReason {
    FilenameCollision,
    Orphan,
}

/// Store handle for the library database. Same conventions as
/// [`crate::store::Store`]: WAL, FK on, numbered migrations, batched
/// transactions for importers.
pub struct LibraryStore {
    conn: Connection,
}

impl LibraryStore {
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
        for (version, sql) in [(1, LIBRARY_SCHEMA_V1)] {
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

/// Idempotent paper insert (INSERT OR IGNORE on the sha256 PK).
/// Returns true when a row was actually inserted.
pub fn insert_paper(conn: &Connection, p: &LibraryPaper) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO paper (sha256, filename, path, size_bytes, registered_at, on_disk,
            doi, arxiv_id, title, authors, year, abstract, journal, source_url, access,
            blob_key, blob_synced_at, subfield, tags, original_language, original_script_title,
            transliterated_title, translation_of, translated_in, soviet_stratum, source_collection)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
        params![
            p.sha256, p.filename, p.path, p.size_bytes, p.registered_at, p.on_disk, p.doi,
            p.arxiv_id, p.title, p.authors, p.year, p.abstract_, p.journal, p.source_url,
            p.access, p.blob_key, p.blob_synced_at, p.subfield, p.tags, p.original_language,
            p.original_script_title, p.transliterated_title, p.translation_of, p.translated_in,
            p.soviet_stratum, p.source_collection,
        ],
    )?;
    Ok(n > 0)
}

pub fn insert_enrichment(conn: &Connection, e: &PaperEnrichment) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO paper_enrichment (sha256, openalex_id, openalex_topics,
            openalex_concepts, openalex_cited_by, s2_paper_id, s2_tldr, s2_fields_of_study,
            s2_influential_citation_count, unpaywall_oa_status, unpaywall_oa_url, enriched_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            e.sha256,
            e.openalex_id,
            e.openalex_topics,
            e.openalex_concepts,
            e.openalex_cited_by,
            e.s2_paper_id,
            e.s2_tldr,
            e.s2_fields_of_study,
            e.s2_influential_citation_count,
            e.unpaywall_oa_status,
            e.unpaywall_oa_url,
            e.enriched_at,
        ],
    )?;
    Ok(n > 0)
}

pub fn insert_movement(conn: &Connection, m: &PaperMovement) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO paper_movement (sha256, from_path, to_path, moved_at, reason)
         VALUES (?1,?2,?3,?4,?5)",
        params![m.sha256, m.from_path, m.to_path, m.moved_at, m.reason],
    )?;
    Ok(n > 0)
}

/// Idempotent chunk insert (INSERT OR IGNORE on UNIQUE(corpus, chunk_id)).
/// Returns the chunk rowid when inserted.
pub fn insert_chunk(conn: &Connection, c: &LibraryChunk) -> Result<Option<i64>, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO chunk (corpus, chunk_id, sha256, filename, title, section, text, word_count, embedding)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            c.corpus, c.chunk_id, c.sha256, c.filename, c.title, c.section, c.text, c.word_count,
            c.embedding,
        ],
    )?;
    Ok(if n > 0 {
        Some(conn.last_insert_rowid())
    } else {
        None
    })
}

pub fn insert_quarantine(
    conn: &Connection,
    corpus: &str,
    chunk_id: i64,
    filename: &str,
    reason: QuarantineReason,
    candidates: &[String],
) -> Result<(), StoreError> {
    let token = match reason {
        QuarantineReason::FilenameCollision => "filename_collision",
        QuarantineReason::Orphan => "orphan",
    };
    conn.execute(
        "INSERT OR IGNORE INTO chunk_quarantine (corpus, chunk_id, filename, reason, candidates_json)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            corpus,
            chunk_id,
            filename,
            token,
            serde_json::to_string(candidates)?
        ],
    )?;
    Ok(())
}

pub fn insert_import_meta(
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
