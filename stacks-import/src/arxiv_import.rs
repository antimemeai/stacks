//! arXiv parquet-sourced import: papers + chunks + incremental FTS for the
//! per-category JSONL shards produced by
//! `maintenance/arxiv-parquet-to-jsonl.py`. Mirrors inproc phase 2
//! (chunk_recursive with the same CHUNK_SIZE/CHUNK_OVERLAP/MIN_WORDS, direct
//! rowid inserts into the external-content chunk_fts) but leaves embeddings
//! NULL — `embed-backfill` (also here) fills them afterwards. Closes with
//! ghost reconciliation: intake stub rows superseded by a real arXiv row get
//! tagged, never deleted.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stacks_core::library::{insert_chunk, insert_paper, LibraryChunk, LibraryPaper, LibraryStore};

use crate::chunker::{chunk_recursive, TextChunk, CHUNK_OVERLAP, CHUNK_SIZE, MIN_WORDS};
use crate::ImportError;

/// Commit granularity inside a category (cs.AI is ~200k papers; one
/// transaction per category would be a multi-GB WAL).
pub const BATCH_PAPERS: usize = 2_000;
pub const EMBED_BATCH: usize = 512;
pub const SOURCE_COLLECTION: &str = "arxiv-parquet";

#[derive(Debug, Deserialize)]
pub struct ArxivRow {
    #[serde(default)]
    pub arxiv_id: Option<String>,
    #[serde(default)]
    pub primary_category: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub categories: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "abstract")]
    pub abstract_: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub has_latex: Option<bool>,
    #[serde(default)]
    pub latex: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct CategoryStat {
    pub papers_inserted: u64,
    pub papers_existing: u64,
    pub rows_rejected_no_id: u64,
    pub chunks_inserted: u64,
    pub fts_rows: u64,
}

#[derive(Debug, Default, Serialize)]
pub struct ArxivLoadStats {
    pub papers_inserted: u64,
    pub papers_existing: u64,
    pub rows_rejected_no_id: u64,
    pub chunks_inserted: u64,
    pub fts_rows: u64,
    pub per_category: BTreeMap<String, CategoryStat>,
    pub ghosts_marked: u64,
    pub elapsed_secs: f64,
}

pub struct ArxivLoadOpts {
    /// Category basenames to load (e.g. ["cs.AI"]); None = every .jsonl.
    pub categories: Option<Vec<String>>,
    pub limit: Option<u64>,
    /// Log a line to stderr every N papers.
    pub stats_every: u64,
    pub skip_ghosts: bool,
}

impl Default for ArxivLoadOpts {
    fn default() -> Self {
        Self {
            categories: None,
            limit: None,
            stats_every: 5_000,
            skip_ghosts: false,
        }
    }
}

pub fn sha256_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// First 4 chars of the date when they parse as an integer year.
pub fn year_from_date(date: Option<&str>) -> Option<i64> {
    let date = date?;
    date.get(..4)?.parse().ok()
}

/// New-style arXiv id (`2301.00001`) embedded in a filename, e.g. an intake
/// stub like `intake/ab/2301.00001.pdf`. Old-style ids (cs0001015) don't
/// appear in intake filenames, so the old shape is deliberately not matched.
pub fn arxiv_id_from_filename(filename: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\d{4}\.\d{4,5})").unwrap());
    re.captures(filename)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn nonblank(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// Chunk text for one row. LaTeX rows chunk recursively; when that yields
/// nothing (pathological source) fall back to the abstract. Abstract-only
/// rows get one chunk when the abstract clears MIN_WORDS; when it doesn't we
/// still insert the whole abstract as a single chunk — it is the paper's
/// only content, so dropping it would make the paper unsearchable (inproc
/// drops tiny texts only because a full payload exists on disk behind them).
fn row_chunks(row: &ArxivRow) -> Vec<TextChunk> {
    let latex_ok = row.has_latex == Some(true);
    if latex_ok {
        if let Some(latex) = nonblank(&row.latex) {
            let chunks = chunk_recursive(latex, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
            if !chunks.is_empty() {
                return chunks;
            }
        }
    }
    let Some(abstract_) = nonblank(&row.abstract_) else {
        return Vec::new();
    };
    let chunks = chunk_recursive(abstract_, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
    if !chunks.is_empty() {
        return chunks;
    }
    vec![TextChunk {
        section: "Abstract".to_string(),
        word_count: abstract_.split_whitespace().count() as i64,
        text: abstract_.to_string(),
    }]
}

fn row_paper(row: &ArxivRow, category: &str, registered_at: &str) -> Option<LibraryPaper> {
    let arxiv_id = nonblank(&row.arxiv_id)?.to_string();
    let latex_ok = row.has_latex == Some(true) && nonblank(&row.latex).is_some();
    let content = if latex_ok {
        row.latex.as_deref().unwrap_or("")
    } else {
        row.abstract_.as_deref().unwrap_or("")
    };
    let ext = if latex_ok { "tex" } else { "txt" };
    Some(LibraryPaper {
        sha256: sha256_text(content),
        filename: format!("{arxiv_id}.{ext}"),
        path: None,
        registered_at: Some(registered_at.to_string()),
        on_disk: Some(false),
        arxiv_id: Some(arxiv_id),
        title: nonblank(&row.title).map(str::to_string),
        year: year_from_date(row.date.as_deref()),
        abstract_: nonblank(&row.abstract_).map(str::to_string),
        access: Some("open".to_string()),
        subfield: Some(
            nonblank(&row.primary_category)
                .unwrap_or(category)
                .to_string(),
        ),
        source_collection: Some(SOURCE_COLLECTION.to_string()),
        ..serde_json::from_value(serde_json::json!({"sha256": "", "filename": ""})).unwrap()
    })
}

/// Progress per category: lines of the shard already consumed. Inline table
/// (import_meta is run-level provenance, not a resumable cursor).
fn ensure_progress_table(store: &LibraryStore) -> Result<(), ImportError> {
    store.raw().execute_batch(
        "CREATE TABLE IF NOT EXISTS arxiv_load_progress (
            category TEXT PRIMARY KEY,
            rows_done INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

fn progress_for(store: &LibraryStore, category: &str) -> Result<u64, ImportError> {
    let n: Option<i64> = store
        .raw()
        .query_row(
            "SELECT rows_done FROM arxiv_load_progress WHERE category = ?1",
            params![category],
            |r| r.get(0),
        )
        .ok();
    Ok(n.unwrap_or(0) as u64)
}

struct BatchState {
    /// Lines consumed from the shard this batch (progress baseline added on write).
    rows_seen: u64,
    /// Next chunk_id per corpus (UNIQUE(corpus, chunk_id) is corpus-global).
    next_chunk_id: std::collections::HashMap<String, i64>,
}

fn process_row(
    conn: &rusqlite::Connection,
    state: &mut BatchState,
    row: &ArxivRow,
    category: &str,
    registered_at: &str,
    stat: &mut CategoryStat,
) -> Result<(), ImportError> {
    let Some(paper) = row_paper(row, category, registered_at) else {
        stat.rows_rejected_no_id += 1;
        return Ok(());
    };
    // INSERT OR IGNORE on the sha256 PK: a paper already imported (rerun, or
    // the same arxiv_id duplicated in another category dir) skips its chunks
    // too — chunk identity is per-corpus, not per-paper, so re-chunking an
    // existing paper would double-insert text under fresh chunk_ids.
    if !insert_paper(conn, &paper)? {
        stat.papers_existing += 1;
        return Ok(());
    }
    stat.papers_inserted += 1;

    let chunks = row_chunks(row);
    if chunks.is_empty() {
        return Ok(());
    }
    let corpus = paper.subfield.clone().unwrap_or_else(|| category.to_string());
    let filename = paper.filename.clone();
    let title = paper.title.clone();
    let next_id = match state.next_chunk_id.get(&corpus) {
        Some(&n) => n,
        None => {
            let n: i64 = conn.query_row(
                "SELECT COALESCE(MAX(chunk_id), -1) + 1 FROM chunk WHERE corpus = ?1",
                params![corpus],
                |r| r.get(0),
            )?;
            state.next_chunk_id.insert(corpus.clone(), n);
            n
        }
    };
    let mut chunk_id = next_id;
    for c in chunks {
        let chunk = LibraryChunk {
            rowid: 0,
            corpus: corpus.clone(),
            chunk_id,
            sha256: Some(paper.sha256.clone()),
            filename: filename.clone(),
            title: title.clone(),
            section: Some(c.section),
            text: c.text,
            word_count: Some(c.word_count),
            embedding: None,
        };
        chunk_id += 1;
        if let Some(rowid) = insert_chunk(conn, &chunk)? {
            // chunk_fts is external-content on chunk(rowid); direct rowid
            // insert keeps it in sync incrementally, as inproc does.
            conn.execute(
                "INSERT INTO chunk_fts(rowid, text) VALUES (?1, ?2)",
                params![rowid, chunk.text],
            )?;
            stat.fts_rows += 1;
        }
        stat.chunks_inserted += 1;
    }
    state.next_chunk_id.insert(corpus, chunk_id);
    Ok(())
}

/// Load one category shard. Resumable: skips `rows_done` leading lines.
fn load_category(
    store: &mut LibraryStore,
    jsonl_path: &Path,
    category: &str,
    opts: &ArxivLoadOpts,
    stats: &mut ArxivLoadStats,
) -> Result<(), ImportError> {
    let mut rows_done = progress_for(store, category)?;
    let reader = BufReader::with_capacity(1 << 20, File::open(jsonl_path)?);
    let mut lines = reader.lines();
    for _ in 0..rows_done {
        if lines.next().transpose()?.is_none() {
            return Ok(());
        }
    }
    let registered_at = crate::inproc::now_utc();
    let mut state = BatchState {
        rows_seen: 0,
        next_chunk_id: Default::default(),
    };
    let mut cat_total = CategoryStat::default();
    let mut last_logged = 0u64;
    loop {
        if opts.limit.is_some_and(|l| {
            stats.papers_inserted + stats.papers_existing + cat_total.papers_inserted
                + cat_total.papers_existing
                >= l
        }) {
            break;
        }
        let baseline = rows_done;
        let mut batch_stat = CategoryStat::default();
        let n = store.with_transaction(|conn| {
            let mut n = 0usize;
            while n < BATCH_PAPERS {
                let Some(line) = lines.next() else { break };
                let line = line?;
                if line.trim().is_empty() {
                    state.rows_seen += 1;
                    continue;
                }
                let row: ArxivRow = serde_json::from_str(&line)?;
                process_row(conn, &mut state, &row, category, &registered_at, &mut batch_stat)?;
                state.rows_seen += 1;
                n += 1;
            }
            conn.execute(
                "INSERT OR REPLACE INTO arxiv_load_progress (category, rows_done) VALUES (?1, ?2)",
                params![category, baseline + state.rows_seen],
            )?;
            Ok::<usize, ImportError>(n)
        })?;
        rows_done += state.rows_seen;
        state.rows_seen = 0;
        cat_total.papers_inserted += batch_stat.papers_inserted;
        cat_total.papers_existing += batch_stat.papers_existing;
        cat_total.rows_rejected_no_id += batch_stat.rows_rejected_no_id;
        cat_total.chunks_inserted += batch_stat.chunks_inserted;
        cat_total.fts_rows += batch_stat.fts_rows;
        let papers = cat_total.papers_inserted + cat_total.papers_existing;
        if opts.stats_every > 0 && papers / opts.stats_every > last_logged {
            last_logged = papers / opts.stats_every;
            eprintln!(
                "arxiv-load {category}: {papers} papers (+{} existing), {} chunks",
                cat_total.papers_existing, cat_total.chunks_inserted,
            );
        }
        if n == 0 {
            break;
        }
    }
    stats.papers_inserted += cat_total.papers_inserted;
    stats.papers_existing += cat_total.papers_existing;
    stats.rows_rejected_no_id += cat_total.rows_rejected_no_id;
    stats.chunks_inserted += cat_total.chunks_inserted;
    stats.fts_rows += cat_total.fts_rows;
    let entry = stats.per_category.entry(category.to_string()).or_default();
    entry.papers_inserted += cat_total.papers_inserted;
    entry.papers_existing += cat_total.papers_existing;
    entry.rows_rejected_no_id += cat_total.rows_rejected_no_id;
    entry.chunks_inserted += cat_total.chunks_inserted;
    entry.fts_rows += cat_total.fts_rows;
    Ok(())
}

/// Ghost reconciliation: intake stub rows (`path LIKE 'intake/%'`,
/// `on_disk=0`) superseded by a real arXiv import get tagged
/// `superseded-by:arxiv`; rows are never deleted. Match by new-style arXiv
/// id in the stub filename, else exact title (LOWER TRIM).
pub fn reconcile_ghosts(store: &mut LibraryStore) -> Result<u64, ImportError> {
    let ghosts: Vec<(String, String, Option<String>, Option<String>)> = store
        .raw()
        .prepare(
            "SELECT sha256, filename, title, tags FROM paper
             WHERE path LIKE 'intake/%' AND on_disk = 0",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let mut marked = 0u64;
    store.with_transaction(|conn| {
        for (sha, filename, title, tags) in &ghosts {
            let hit: Option<String> = arxiv_id_from_filename(filename).and_then(|id| {
                conn.query_row(
                    "SELECT sha256 FROM paper WHERE arxiv_id = ?1 AND source_collection = ?2",
                    params![id, SOURCE_COLLECTION],
                    |r| r.get(0),
                )
                .ok()
            });
            let hit = hit.or_else(|| {
                let title = title.as_deref()?.trim();
                if title.is_empty() {
                    return None;
                }
                conn.query_row(
                    "SELECT sha256 FROM paper
                     WHERE source_collection = ?1 AND LOWER(TRIM(title)) = LOWER(?2)",
                    params![SOURCE_COLLECTION, title],
                    |r| r.get(0),
                )
                .ok()
            });
            if hit.is_none() {
                continue;
            }
            if tags.as_deref().unwrap_or("").contains("superseded-by:arxiv") {
                continue;
            }
            conn.execute(
                "UPDATE paper SET tags = COALESCE(tags, '') || ' superseded-by:arxiv'
                 WHERE sha256 = ?1",
                params![sha],
            )?;
            marked += 1;
        }
        Ok::<_, ImportError>(())
    })?;
    Ok(marked)
}

pub fn arxiv_load(
    store: &mut LibraryStore,
    jsonl_dir: &Path,
    opts: &ArxivLoadOpts,
) -> Result<ArxivLoadStats, ImportError> {
    let t0 = Instant::now();
    ensure_progress_table(store)?;
    let mut shards: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(jsonl_dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(category) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if let Some(wanted) = &opts.categories {
            if !wanted.iter().any(|w| w == category) {
                continue;
            }
        }
        shards.push((category.to_string(), path));
    }
    shards.sort();

    let mut stats = ArxivLoadStats::default();
    for (category, path) in &shards {
        load_category(store, path, category, opts, &mut stats)?;
    }
    if !opts.skip_ghosts {
        stats.ghosts_marked = reconcile_ghosts(store)?;
    }
    stats.elapsed_secs = t0.elapsed().as_secs_f64();
    Ok(stats)
}

// ---------- embed-backfill ----------

#[derive(Debug, Default, Serialize)]
pub struct EmbedBackfillStats {
    pub embedded: u64,
    pub remaining_estimate: u64,
    pub elapsed_secs: f64,
    pub chunks_per_sec: f64,
}

/// Fill `chunk.embedding` (MiniLM 384-dim, NULL so far) in batches. Resumable
/// by construction: every rerun picks the lowest-rowid NULL rows.
pub fn embed_backfill(
    store: &mut LibraryStore,
    embedder: &mut dyn crate::embed::BatchEmbedder,
    corpus: Option<&str>,
    limit: Option<u64>,
) -> Result<EmbedBackfillStats, ImportError> {
    let t0 = Instant::now();
    let mut stats = EmbedBackfillStats::default();
    loop {
        let remaining_budget = limit.map(|l| l.saturating_sub(stats.embedded) as usize);
        if remaining_budget == Some(0) {
            break;
        }
        let page = remaining_budget.unwrap_or(EMBED_BATCH).min(EMBED_BATCH);
        let rows: Vec<(i64, String)> = match corpus {
            Some(c) => store
                .raw()
                .prepare(
                    "SELECT rowid, text FROM chunk WHERE embedding IS NULL AND corpus = ?1
                     ORDER BY rowid LIMIT ?2",
                )?
                .query_map(params![c, page as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?,
            None => store
                .raw()
                .prepare(
                    "SELECT rowid, text FROM chunk WHERE embedding IS NULL
                     ORDER BY rowid LIMIT ?1",
                )?
                .query_map(params![page as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?,
        };
        if rows.is_empty() {
            break;
        }
        let texts: Vec<String> = rows.iter().map(|(_, t)| t.clone()).collect();
        let vecs = embedder
            .embed_batch(&texts)
            .map_err(|e| ImportError::Io(std::io::Error::other(format!("embed batch: {e}"))))?;
        if vecs.len() != rows.len() {
            return Err(ImportError::Io(std::io::Error::other(format!(
                "embedder returned {} vectors for {} texts",
                vecs.len(),
                rows.len()
            ))));
        }
        store.with_transaction(|conn| {
            for ((rowid, _), v) in rows.iter().zip(&vecs) {
                conn.execute(
                    "UPDATE chunk SET embedding = ?2 WHERE rowid = ?1",
                    params![rowid, crate::embed::f32_vec_to_blob(v)],
                )?;
            }
            Ok::<_, ImportError>(())
        })?;
        stats.embedded += rows.len() as u64;
        eprintln!("embed-backfill: {} embedded", stats.embedded);
    }
    stats.remaining_estimate = match corpus {
        Some(c) => store.raw().query_row(
            "SELECT COUNT(*) FROM chunk WHERE embedding IS NULL AND corpus = ?1",
            params![c],
            |r| r.get(0),
        )?,
        None => store.raw().query_row(
            "SELECT COUNT(*) FROM chunk WHERE embedding IS NULL",
            [],
            |r| r.get(0),
        )?,
    };
    stats.elapsed_secs = t0.elapsed().as_secs_f64();
    if stats.elapsed_secs > 0.0 {
        stats.chunks_per_sec = stats.embedded as f64 / stats.elapsed_secs;
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_text_stable() {
        assert_eq!(
            sha256_text(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(sha256_text("abc").len(), 64);
    }

    #[test]
    fn year_from_date_first_four() {
        assert_eq!(year_from_date(Some("2007-05-23")), Some(2007));
        assert_eq!(year_from_date(Some("1999")), Some(1999));
        assert_eq!(year_from_date(Some("abcd-01-01")), None);
        assert_eq!(year_from_date(Some("20")), None);
        assert_eq!(year_from_date(None), None);
        assert_eq!(year_from_date(Some("")), None);
    }

    #[test]
    fn ghost_regex_matches_new_style_only() {
        assert_eq!(
            arxiv_id_from_filename("2301.00001.pdf"),
            Some("2301.00001".to_string())
        );
        assert_eq!(
            arxiv_id_from_filename("intake/ab/0704.1234v2.pdf"),
            Some("0704.1234".to_string())
        );
        assert_eq!(arxiv_id_from_filename("cs0001015.pdf"), None);
        assert_eq!(arxiv_id_from_filename("no-id-here.pdf"), None);
    }

    #[test]
    fn abstract_only_below_min_words_still_chunks() {
        let row: ArxivRow = serde_json::from_value(serde_json::json!({
            "arxiv_id": "2301.00001",
            "primary_category": "cs.AI",
            "abstract": "Short but real abstract.",
            "has_latex": false,
        }))
        .unwrap();
        let chunks = row_chunks(&row);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].section, "Abstract");
    }

    #[test]
    fn latex_row_chunks_from_latex_not_abstract() {
        let words: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let row: ArxivRow = serde_json::from_value(serde_json::json!({
            "arxiv_id": "2301.00002",
            "primary_category": "cs.AI",
            "abstract": "ignored abstract text",
            "has_latex": true,
            "latex": words.join(" "),
        }))
        .unwrap();
        let chunks = row_chunks(&row);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.starts_with("w0"));
    }
}
