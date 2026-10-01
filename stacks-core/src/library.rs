//! The library pillar: neurotic_library's document catalog + chunk corpora,
//! migrated into a sibling database (`data/library.db`). Deliberately
//! separate from the recipe system of record — different domain, different
//! access patterns (FTS5), same teeth conventions.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

const LIBRARY_SCHEMA_V11: &str = r#"
-- Original filename at enqueue time. Intake moves payloads to sha-named
-- blobs, so without this the drain has no title fallback when PDF metadata
-- and first-page text are junk. NULL for rows enqueued before V11 (the
-- recover-orig-filenames maintenance script backfills them).
ALTER TABLE inproc_queue ADD COLUMN orig_filename TEXT;
"#;

const LIBRARY_SCHEMA_V10: &str = r#"
-- Shadow embeddings from pplx-embed-v2 (contextual, 2048-dim, native int8).
-- Parallel to chunk.embedding (MiniLM 384-dim f32); the `model` column pins
-- the exact HF revision because the preview's embeddings must not mix with
-- any later release. `embedding` is one int8 byte per dimension.
CREATE TABLE chunk_embedding_pplx (
    corpus TEXT NOT NULL,
    chunk_id INTEGER NOT NULL,
    model TEXT NOT NULL,
    dims INTEGER NOT NULL,
    embedding BLOB NOT NULL CHECK (length(embedding) = dims),
    embedded_at TEXT NOT NULL,
    UNIQUE (corpus, chunk_id, model)
);
CREATE INDEX chunk_embedding_pplx_model ON chunk_embedding_pplx (model);
"#;

const LIBRARY_SCHEMA_V9: &str = r#"
-- Dataset provenance: whether a payload was acquired externally
-- ('downloaded'), computed here ('earned'), or contains both ('mixed').
-- Existing rows are all downloaded; the DEFAULT keeps them valid under the
-- CHECK (SQLite evaluates CHECKs on existing rows when the column is added).
ALTER TABLE dataset ADD COLUMN provenance TEXT NOT NULL DEFAULT 'downloaded'
    CHECK (provenance IN ('downloaded','earned','mixed'));
"#;

const LIBRARY_SCHEMA_V8: &str = r#"
-- Wave ledger: NL's neuroticd intake ledger (embeddings/.neuroticd_state.json
-- + journal), the authoritative record of ~10k prior intake attempts. The
-- drain consults it before any API call so NL-exhausted files never cost
-- another attempt. wave_staging fallback copies are recorded as
-- nl_status='fallback_copy' for lineage.
CREATE TABLE nl_ledger (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    sha256 TEXT,
    nl_status TEXT NOT NULL,
    nl_detail TEXT,
    journal_events INTEGER DEFAULT 0,
    last_event_at TEXT,
    imported_at TEXT NOT NULL,
    UNIQUE (path)
);
CREATE INDEX nl_ledger_sha256 ON nl_ledger (sha256);
"#;

const LIBRARY_SCHEMA_V7: &str = r#"
-- Wave inproc: the intake pipeline rework. inproc_queue is the generic
-- work queue for the whole pipeline (stamp-in, extract, enrich, classify):
-- status/reason/attempts are reused by the later stages. dlq_review records
-- human decisions on dead-letter items and returns them to the queue.
CREATE TABLE inproc_queue (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER,
    enqueued_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','processing','stamped','enriched','dlq','done','failed')),
    reason TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT,
    UNIQUE (path, sha256)
);
CREATE INDEX inproc_queue_status ON inproc_queue (status);

CREATE TABLE dlq_review (
    id INTEGER PRIMARY KEY,
    queue_id INTEGER NOT NULL REFERENCES inproc_queue (id),
    asserted_kind TEXT CHECK (asserted_kind IN ('paper','document')),
    asserted_category TEXT,
    asserted_title TEXT,
    decided_by TEXT NOT NULL,
    decided_at TEXT NOT NULL,
    notes TEXT
);
"#;

const LIBRARY_SCHEMA_V6: &str = r#"
-- Wave I: the dataset registry — every data payload on the host, registered
-- in place, discoverable without moving bytes.
CREATE TABLE dataset (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    location_root TEXT NOT NULL,
    size_bytes INTEGER,
    file_count INTEGER,
    dominant_formats TEXT,
    sha256_status TEXT NOT NULL CHECK (sha256_status IN ('none','sidecars','hashed')),
    description TEXT NOT NULL,
    domains TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('registered','migrated','preserved_original','queryable','extraction_queue','missing')),
    status_note TEXT,
    registered_at TEXT NOT NULL
);
CREATE INDEX dataset_status ON dataset (status);
"#;

const LIBRARY_SCHEMA_V5: &str = r#"
-- Soviet-specific fields on documents (they lived only on paper before).
ALTER TABLE document ADD COLUMN original_language TEXT;
ALTER TABLE document ADD COLUMN transliterated_title TEXT;
ALTER TABLE document ADD COLUMN soviet_stratum TEXT;
"#;

const LIBRARY_SCHEMA_V4: &str = r#"
-- kind vocabulary gains 'article'. Rebuild (SQLite cannot alter CHECK).
-- Learned the hard way: OR IGNORE makes CHECK violations silent skips.
ALTER TABLE document RENAME TO document_v3;
CREATE TABLE document (
    sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    family TEXT NOT NULL CHECK (length(family) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('book','report','patent','thesis','paper','dataset-paper','archive','article')),
    title TEXT,
    authors TEXT,
    year INTEGER,
    language TEXT,
    pages INTEGER,
    source_url TEXT,
    download_url TEXT,
    path TEXT NOT NULL,
    location_root TEXT NOT NULL,
    bytes INTEGER,
    retrieved_at TEXT,
    text_layer_path TEXT,
    collection TEXT
);
INSERT INTO document (sha256, family, kind, title, authors, year, language, pages,
    source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path, collection)
SELECT sha256, family, kind, title, authors, year, language, pages,
    source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path, collection
FROM document_v3;
DROP TABLE document_v3;
CREATE INDEX document_family ON document (family);
CREATE INDEX document_kind ON document (kind);
CREATE INDEX document_collection ON document (collection);
"#;

const LIBRARY_SCHEMA_V3: &str = r#"
-- Wave G: collection tags (lib_ussr topic dirs, intake buckets) and the
-- intake triage audit table. 'archive' joins the kind vocabulary, so the
-- document table is rebuilt (SQLite cannot alter CHECK constraints; the
-- table is small).
ALTER TABLE document RENAME TO document_v2;
CREATE TABLE document (
    sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    family TEXT NOT NULL CHECK (length(family) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('book','report','patent','thesis','paper','dataset-paper','archive','article')),
    -- NOTE (v4): 'article' added after OR IGNORE silently dropped a lib_ussr txt (CHECK violations are ignorable under INSERT OR IGNORE!)
    title TEXT,
    authors TEXT,
    year INTEGER,
    language TEXT,
    pages INTEGER,
    source_url TEXT,
    download_url TEXT,
    path TEXT NOT NULL,
    location_root TEXT NOT NULL,
    bytes INTEGER,
    retrieved_at TEXT,
    text_layer_path TEXT,
    collection TEXT
);
INSERT INTO document (sha256, family, kind, title, authors, year, language, pages,
    source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path, collection)
SELECT sha256, family, kind, title, authors, year, language, pages,
    source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path, NULL
FROM document_v2;
DROP TABLE document_v2;
CREATE INDEX document_family ON document (family);
CREATE INDEX document_kind ON document (kind);
CREATE INDEX document_collection ON document (collection);

-- Every triage decision is a row: the auditable artifact of wave G.
CREATE TABLE intake_triage (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    decision TEXT NOT NULL CHECK (decision IN ('imported','personal','archive','duplicate','skipped')),
    rule TEXT NOT NULL,
    reason TEXT,
    sha256 TEXT
);
"#;

const LIBRARY_SCHEMA_V2: &str = r#"
-- Documents: books, reports, patents, theses (the `paper` table is
-- paper-shaped; these acquisition families are not papers). Payloads are
-- referenced in place, never copied; location_root holds the absolute
-- source root so a future move is one UPDATE.
CREATE TABLE document (
    sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    family TEXT NOT NULL CHECK (length(family) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('book','report','patent','thesis','paper','dataset-paper')),
    title TEXT,
    authors TEXT,
    year INTEGER,
    language TEXT,
    pages INTEGER,
    source_url TEXT,
    download_url TEXT,
    path TEXT NOT NULL,
    location_root TEXT NOT NULL,
    bytes INTEGER,
    retrieved_at TEXT,
    text_layer_path TEXT
);
CREATE INDEX document_family ON document (family);
CREATE INDEX document_kind ON document (kind);
"#;

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
        for (version, sql) in [
            (1, LIBRARY_SCHEMA_V1),
            (2, LIBRARY_SCHEMA_V2),
            (3, LIBRARY_SCHEMA_V3),
            (4, LIBRARY_SCHEMA_V4),
            (5, LIBRARY_SCHEMA_V5),
            (6, LIBRARY_SCHEMA_V6),
            (7, LIBRARY_SCHEMA_V7),
            (8, LIBRARY_SCHEMA_V8),
            (9, LIBRARY_SCHEMA_V9),
            (10, LIBRARY_SCHEMA_V10),
            (11, LIBRARY_SCHEMA_V11),
        ] {
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

/// Sparse upsert for enrichment-written paper rows: INSERT OR IGNORE first,
/// then UPDATE only the fields the caller actually knows (None never
/// clobbers an existing value).
pub fn upsert_paper_sparse(conn: &Connection, p: &LibraryPaper) -> Result<(), StoreError> {
    insert_paper(conn, p)?;
    let cols: [(&str, Option<Box<dyn rusqlite::ToSql>>); 11] = [
        ("title", p.title.clone().map(|v| Box::new(v) as _)),
        ("authors", p.authors.clone().map(|v| Box::new(v) as _)),
        ("year", p.year.map(|v| Box::new(v) as _)),
        ("abstract", p.abstract_.clone().map(|v| Box::new(v) as _)),
        ("journal", p.journal.clone().map(|v| Box::new(v) as _)),
        ("doi", p.doi.clone().map(|v| Box::new(v) as _)),
        ("arxiv_id", p.arxiv_id.clone().map(|v| Box::new(v) as _)),
        ("subfield", p.subfield.clone().map(|v| Box::new(v) as _)),
        ("path", p.path.clone().map(|v| Box::new(v) as _)),
        ("size_bytes", p.size_bytes.map(|v| Box::new(v) as _)),
        ("access", p.access.clone().map(|v| Box::new(v) as _)),
    ];
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(p.sha256.clone())];
    for (col, val) in cols.into_iter().flat_map(|(c, v)| v.map(|v| (c, v))) {
        sets.push(format!("{col} = ?{}", values.len() + 1));
        values.push(val);
    }
    if !sets.is_empty() {
        conn.execute(
            &format!("UPDATE paper SET {} WHERE sha256 = ?1", sets.join(", ")),
            rusqlite::params_from_iter(values.iter()),
        )?;
    }
    Ok(())
}

/// Sparse upsert for paper_enrichment: INSERT OR IGNORE first, then UPDATE
/// only non-NULL fields (enriched_at included by the caller when wanted).
pub fn upsert_enrichment(conn: &Connection, e: &PaperEnrichment) -> Result<(), StoreError> {
    insert_enrichment(conn, e)?;
    let cols: [(&str, Option<Box<dyn rusqlite::ToSql>>); 11] = [
        ("openalex_id", e.openalex_id.clone().map(|v| Box::new(v) as _)),
        ("openalex_topics", e.openalex_topics.clone().map(|v| Box::new(v) as _)),
        ("openalex_concepts", e.openalex_concepts.clone().map(|v| Box::new(v) as _)),
        ("openalex_cited_by", e.openalex_cited_by.map(|v| Box::new(v) as _)),
        ("s2_paper_id", e.s2_paper_id.clone().map(|v| Box::new(v) as _)),
        ("s2_tldr", e.s2_tldr.clone().map(|v| Box::new(v) as _)),
        ("s2_fields_of_study", e.s2_fields_of_study.clone().map(|v| Box::new(v) as _)),
        ("s2_influential_citation_count", e.s2_influential_citation_count.map(|v| Box::new(v) as _)),
        ("unpaywall_oa_status", e.unpaywall_oa_status.clone().map(|v| Box::new(v) as _)),
        ("unpaywall_oa_url", e.unpaywall_oa_url.clone().map(|v| Box::new(v) as _)),
        ("enriched_at", e.enriched_at.clone().map(|v| Box::new(v) as _)),
    ];
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(e.sha256.clone())];
    for (col, val) in cols.into_iter().flat_map(|(c, v)| v.map(|v| (c, v))) {
        sets.push(format!("{col} = ?{}", values.len() + 1));
        values.push(val);
    }
    if !sets.is_empty() {
        conn.execute(
            &format!(
                "UPDATE paper_enrichment SET {} WHERE sha256 = ?1",
                sets.join(", ")
            ),
            rusqlite::params_from_iter(values.iter()),
        )?;
    }
    Ok(())
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

/// Shadow pplx-embed embedding for a chunk: int8 bytes, `model` pinned to the
/// exact HF revision. INSERT OR IGNORE keeps backfill reruns idempotent.
pub fn insert_chunk_embedding_pplx(
    conn: &Connection,
    corpus: &str,
    chunk_id: i64,
    model: &str,
    embedding: &[u8],
) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO chunk_embedding_pplx
            (corpus, chunk_id, model, dims, embedding, embedded_at)
         VALUES (?1,?2,?3,?4,?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![corpus, chunk_id, model, embedding.len() as i64, embedding],
    )?;
    Ok(n > 0)
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

/// Acquisition-bay document (matdattmp manifest row).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LibraryDocument {
    pub sha256: String,
    /// Acquisition ledger family (historical, government, patents, …).
    pub family: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Path relative to `location_root`.
    pub path: String,
    /// Absolute source root at import time.
    pub location_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_layer_path: Option<String>,
    /// Collection tag (lib_ussr topic dir, intake bucket).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transliterated_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soviet_stratum: Option<String>,
}

pub fn insert_document(conn: &Connection, d: &LibraryDocument) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO document (sha256, family, kind, title, authors, year, language,
            pages, source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path,
            collection, original_language, transliterated_title, soviet_stratum)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![
            d.sha256, d.family, d.kind, d.title, d.authors, d.year, d.language, d.pages,
            d.source_url, d.download_url, d.path, d.location_root, d.bytes, d.retrieved_at,
            d.text_layer_path, d.collection, d.original_language, d.transliterated_title,
            d.soviet_stratum,
        ],
    )?;
    Ok(n > 0)
}

/// One registered dataset (Wave I registry). Descriptions are
/// curator-written (see docs/dataset-registry-report.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Dataset {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    /// Path relative to `location_root`.
    pub path: String,
    pub location_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_count: Option<i64>,
    /// JSON array of dominant extensions with counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_formats: Option<Vec<(String, u64)>>,
    /// none | sidecars | hashed
    pub sha256_status: String,
    pub description: String,
    /// Domain tags.
    pub domains: Vec<String>,
    /// registered | migrated | preserved_original | queryable |
    /// extraction_queue | missing
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_note: Option<String>,
    /// downloaded | earned | mixed. Defaults to downloaded; set on insert,
    /// preserved on upsert unless the entry declares a non-default value.
    #[serde(default = "default_provenance")]
    pub provenance: String,
    #[serde(default)]
    pub registered_at: String,
}

fn default_provenance() -> String {
    "downloaded".to_string()
}

pub fn upsert_dataset(conn: &Connection, d: &Dataset) -> Result<bool, StoreError> {
    let n = conn.execute(
        "INSERT INTO dataset (name, path, location_root, size_bytes, file_count, dominant_formats,
            sha256_status, description, domains, status, status_note, provenance, registered_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(name) DO UPDATE SET path=excluded.path,
            location_root=excluded.location_root, size_bytes=excluded.size_bytes,
            file_count=excluded.file_count, dominant_formats=excluded.dominant_formats,
            sha256_status=excluded.sha256_status, description=excluded.description,
            domains=excluded.domains, status_note=excluded.status_note,
            status=CASE WHEN dataset.status='missing' THEN 'missing' ELSE excluded.status END,
            provenance=CASE WHEN excluded.provenance='downloaded' THEN dataset.provenance
                ELSE excluded.provenance END",
        params![
            d.name,
            d.path,
            d.location_root,
            d.size_bytes,
            d.file_count,
            d.dominant_formats
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            d.sha256_status,
            d.description,
            serde_json::to_string(&d.domains)?,
            d.status,
            d.status_note,
            d.provenance,
            d.registered_at,
        ],
    )?;
    Ok(n > 0)
}

/// Verify pass: flip registered datasets whose path vanished to
/// status='missing' (and back when the path returns). Returns flipped names.
pub fn verify_datasets(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT name, path, location_root, status FROM dataset")?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    let mut flipped = Vec::new();
    for (name, path, root, status) in rows {
        let full = Path::new(&root).join(&path);
        let exists = full.exists();
        if !exists && status != "missing" {
            conn.execute(
                "UPDATE dataset SET status = 'missing' WHERE name = ?1",
                [&name],
            )?;
            flipped.push(format!("{name} -> missing"));
        } else if exists && status == "missing" {
            conn.execute(
                "UPDATE dataset SET status = 'registered' WHERE name = ?1",
                [&name],
            )?;
            flipped.push(format!("{name} -> registered (path returned)"));
        }
    }
    Ok(flipped)
}
