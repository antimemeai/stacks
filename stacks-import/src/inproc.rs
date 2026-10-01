//! Wave inproc: the network-free half of the intake pipeline rework.
//! `enqueue_sources` moves payloads into the intake fan-out and queues them
//! in inproc_queue; `drain_queue` runs the stamp-in bonafides check and
//! lands ready payloads in the corpus; `dlq_*` is the human-review skeleton
//! for dead-lettered items. Replaces the batch triage of intake_triage.rs
//! (which keeps its hardcoded ROOT and stays for the historical audit).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use stacks_core::bonafides::check_bonafides;
use stacks_core::library::{
    upsert_enrichment, upsert_paper_sparse, LibraryDocument, LibraryPaper, LibraryStore,
    PaperEnrichment,
};

use crate::enrich::{resolve_identity, EnrichClient, OpenAlexMapped};
use crate::extract::{self, ExtractedIds, ItemKind};

use crate::ImportError;

pub const DEFAULT_INTAKE_DIR: &str = "/srv/stacks/intake";
pub const DEFAULT_LIBRARY_DB: &str = "/srv/stacks/db/library.db";
pub const DEFAULT_CORPUS_DIR: &str = "/srv/stacks/corpus";

const ARCHIVE_EXTS: &[&str] = &["zip", "tar", "tgz"];

/// Current UTC time as ISO8601 (`YYYY-MM-DDTHH:MM:SSZ`), no chrono.
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_utc(secs)
}

/// Epoch seconds → ISO8601 UTC. The format is fixed-width, so lexicographic
/// string compare is chronological order (used for rate windows).
pub fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod / 60) % 60,
        tod % 60
    )
}

/// Subfield/collection name to directory slug, as NL did: lowercase,
/// spaces and underscores to hyphens.
pub fn slugify(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' || c == '_' { '-' } else { c })
        .collect()
}

pub fn sha256_file(path: &Path) -> Result<String, ImportError> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.ends_with(".tar.gz")
        || name.ends_with(".tar.zst")
        || ARCHIVE_EXTS
            .iter()
            .any(|ext| name.ends_with(&format!(".{ext}")))
}

/// Move src to dst, verifying the destination hash. Same-filesystem rename
/// first; across filesystems fall back to copy + hash verify + remove.
fn move_verified(src: &Path, dst: &Path, sha256: &str) -> Result<(), ImportError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(src, dst).is_err() {
        fs::copy(src, dst)?;
        if sha256_file(dst)? != sha256 {
            let _ = fs::remove_file(dst);
            return Err(ImportError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("hash mismatch after copy to {}", dst.display()),
            )));
        }
        fs::remove_file(src)?;
    } else if sha256_file(dst)? != sha256 {
        return Err(ImportError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("hash mismatch after rename to {}", dst.display()),
        )));
    }
    Ok(())
}

/// Fan-out intake destination: `<intake-dir>/incoming/<sha-prefix>/<sha><ext>`.
fn intake_dest(intake_dir: &Path, sha256: &str, ext: &str) -> PathBuf {
    intake_dir
        .join("incoming")
        .join(&sha256[..2])
        .join(format!("{sha256}{ext}"))
}

fn file_ext(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

#[derive(Debug, Default, Serialize)]
pub struct InprocStats {
    pub files_seen: u64,
    pub archives_extracted: u64,
    pub enqueued: u64,
    /// Same (path, sha256) already queued.
    pub duplicates: u64,
    /// Already present at the intake destination (same sha256): skipped, not re-queued.
    pub already_in_intake: u64,
    pub errors: Vec<String>,
}

fn enqueue_one(
    store: &mut LibraryStore,
    file: &Path,
    intake_dir: &Path,
    stats: &mut InprocStats,
) -> Result<(), ImportError> {
    stats.files_seen += 1;
    let sha256 = sha256_file(file)?;
    let dest = intake_dest(intake_dir, &sha256, &file_ext(file));
    if dest.exists() {
        stats.already_in_intake += 1;
        return Ok(());
    }
    let size = fs::metadata(file)?.len() as i64;
    move_verified(file, &dest, &sha256)?;
    let dest_str = dest.to_string_lossy().to_string();
    let now = now_utc();
    let inserted = store.with_transaction(|conn| {
        let n = conn.execute(
            "INSERT OR IGNORE INTO inproc_queue (path, sha256, size_bytes, enqueued_at, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?4)",
            params![dest_str, sha256, size, now],
        )?;
        Ok::<_, ImportError>(n > 0)
    })?;
    if inserted {
        stats.enqueued += 1;
    } else {
        stats.duplicates += 1;
    }
    Ok(())
}

pub(crate) fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ImportError> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Extract an archive into a fresh temp dir (system tar/unzip; no archive
/// crates in the workspace). Caller holds the TempDir for the walk.
fn extract_archive(archive: &Path) -> Result<tempfile::TempDir, ImportError> {
    let tmp = tempfile::tempdir()?;
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let status = if name.ends_with(".zip") {
        Command::new("unzip")
            .arg("-q")
            .arg(archive)
            .arg("-d")
            .arg(tmp.path())
            .status()?
    } else {
        let mut cmd = Command::new("tar");
        cmd.arg("-xf").arg(archive).arg("-C").arg(tmp.path());
        if name.ends_with(".zst") {
            cmd.arg("--use-compress-program=unzstd");
        }
        cmd.status()?
    };
    if !status.success() {
        return Err(ImportError::Io(std::io::Error::other(format!(
            "archive extraction failed ({status}): {}",
            archive.display()
        ))));
    }
    Ok(tmp)
}

/// Ingest every source (file, dir, or archive) into the intake fan-out and
/// enqueue it. Per-item errors are collected, never fatal.
pub fn enqueue_sources(
    store: &mut LibraryStore,
    sources: &[PathBuf],
    intake_dir: &Path,
) -> Result<InprocStats, ImportError> {
    let mut stats = InprocStats::default();
    for source in sources {
        if is_archive(source) {
            match extract_archive(source) {
                Ok(tmp) => {
                    stats.archives_extracted += 1;
                    let mut files = Vec::new();
                    if let Err(e) = walk_files(tmp.path(), &mut files) {
                        stats.errors.push(format!("{}: {e}", source.display()));
                        continue;
                    }
                    for file in files {
                        if let Err(e) = enqueue_one(store, &file, intake_dir, &mut stats) {
                            stats.errors.push(format!("{}: {e}", file.display()));
                        }
                    }
                }
                Err(e) => stats.errors.push(format!("{}: {e}", source.display())),
            }
        } else if source.is_dir() {
            let mut files = Vec::new();
            match walk_files(source, &mut files) {
                Ok(()) => {
                    for file in files {
                        if let Err(e) = enqueue_one(store, &file, intake_dir, &mut stats) {
                            stats.errors.push(format!("{}: {e}", file.display()));
                        }
                    }
                }
                Err(e) => stats.errors.push(format!("{}: {e}", source.display())),
            }
        } else if source.is_file() {
            if let Err(e) = enqueue_one(store, source, intake_dir, &mut stats) {
                stats.errors.push(format!("{}: {e}", source.display()));
            }
        } else {
            stats.errors.push(format!("{}: no such file or directory", source.display()));
        }
    }
    Ok(stats)
}

#[derive(Debug, Default, Serialize)]
pub struct DrainStats {
    pub processed: u64,
    /// Stamped into corpus/stamped/<subfield>/ as papers.
    pub stamped_papers: u64,
    /// Stamped into corpus/stamped/documents/<category>/ as documents.
    pub stamped_documents: u64,
    /// Sent to the dead-letter queue with a structured reason.
    pub dlq: u64,
    /// Hard per-item failure (io etc.); status='failed'.
    pub failed: u64,
    /// DOIs resolved through the OpenAlex batch pre-pass.
    pub batch_hits: u64,
    /// Phase 2: chunked + embedded + FTS-indexed (status 'enriched').
    pub enriched: u64,
    /// Phase 2: chunks+FTS in, but embedding failed — stays 'stamped'.
    pub embed_failed: u64,
    /// Phase 2: no usable full text — stays 'stamped', nothing indexed.
    pub no_full_text: u64,
    pub chunks_inserted: u64,
    /// Already cataloged as documents — closed out, nothing to review.
    pub done: u64,
}

pub struct DrainOpts {
    /// Force the OpenAlex batch pre-pass even for small queues.
    pub batch_openalex: bool,
    /// Skip phase 2 (chunk/embed/FTS) entirely — for machines without the
    /// embedding model.
    pub no_embed: bool,
}

impl Default for DrainOpts {
    fn default() -> Self {
        Self {
            batch_openalex: false,
            no_embed: false,
        }
    }
}

/// A paper stamped this run, queued for the chunk/embed phase.
struct StampedPaper {
    queue_id: i64,
    sha256: String,
    path: PathBuf,
    /// Corpus name for chunk rows: the subfield slug.
    corpus: String,
}

fn mark_failed(conn: &rusqlite::Connection, id: i64, reason: &str) -> Result<(), ImportError> {
    conn.execute(
        "UPDATE inproc_queue SET status = 'failed', reason = ?2,
            attempts = attempts + 1, updated_at = ?3 WHERE id = ?1",
        params![id, reason, now_utc()],
    )?;
    Ok(())
}

fn mark_dlq(conn: &rusqlite::Connection, id: i64, reason: &serde_json::Value, extra_attempts: u32) -> Result<(), ImportError> {
    conn.execute(
        "UPDATE inproc_queue SET status = 'dlq', reason = ?2,
            attempts = attempts + 1 + ?4, updated_at = ?3 WHERE id = ?1",
        params![id, reason.to_string(), now_utc(), extra_attempts],
    )?;
    Ok(())
}

/// Latest dlq_review assertion for a queue row, if any.
fn latest_assertion(
    conn: &rusqlite::Connection,
    queue_id: i64,
) -> Result<Option<(String, Option<String>, Option<String>)>, ImportError> {
    let row = conn
        .query_row(
            "SELECT asserted_kind, asserted_category, asserted_title
             FROM dlq_review WHERE queue_id = ?1 ORDER BY id DESC LIMIT 1",
            params![queue_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    Ok(row)
}

/// The drain: bonafides fast-path first; everything else goes through
/// extraction → the enrichment fallback ladder (batch OpenAlex first when
/// worthwhile) → stamp-in, or the DLQ with a structured reason. Per-item
/// transactions; a crash leaves the row queued and the next run resumes.
/// Papers stamped this run then go through phase 2 (full-text → chunk →
/// embed → FTS), flipping 'stamped' to 'enriched'. Embedding failure leaves
/// the row 'stamped' — chunks and FTS are still written, so the paper is
/// cataloged and BM25-searchable, just not vector-searchable. Phase 2 also
/// resumes stamped rows from earlier runs that never got chunks (a panic in
/// phase 2 previously stranded every later row of that run).
pub fn drain_queue(
    store: &mut LibraryStore,
    corpus_dir: &Path,
    client: &dyn EnrichClient,
    opts: &DrainOpts,
) -> Result<DrainStats, ImportError> {
    let mut embedder = if opts.no_embed {
        None
    } else {
        crate::embed::FastBatchEmbedder::load(&crate::embed::cache_dir()).ok()
    };
    drain_queue_with_embedder(
        store,
        corpus_dir,
        client,
        embedder
            .as_mut()
            .map(|e| e as &mut dyn crate::embed::BatchEmbedder),
        opts,
    )
}

pub fn drain_queue_with_embedder(
    store: &mut LibraryStore,
    corpus_dir: &Path,
    client: &dyn EnrichClient,
    mut embedder: Option<&mut dyn crate::embed::BatchEmbedder>,
    opts: &DrainOpts,
) -> Result<DrainStats, ImportError> {
    let mut stats = DrainStats::default();
    let rows: Vec<(i64, String, String)> = store
        .raw()
        .prepare("SELECT id, path, sha256 FROM inproc_queue WHERE status = 'queued' ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;

    // Pre-pass: extract identifiers for every item that will need them.
    // Read-only — a crash here leaves the queue untouched.
    let mut extracted: std::collections::HashMap<i64, ExtractedIds> = Default::default();
    for (id, path, sha256) in &rows {
        if !Path::new(path).is_file() || latest_assertion(store.raw(), *id)?.is_some() {
            continue;
        }
        let verdict = check_bonafides(store.raw(), sha256)?;
        if verdict.is_ready() {
            continue;
        }
        // Items that will short-circuit at the record prechecks never reach
        // the ladder — don't let their DOIs into the API batch either.
        let records = prior_records(store.raw(), sha256)?;
        if (verdict.paper_exists && legacy_enriched(store.raw(), sha256)?)
            || nl_exhausted(&records)
        {
            continue;
        }
        if let Ok(ids) = extract::extract(Path::new(path)) {
            extracted.insert(*id, ids);
        }
    }

    // Batch pass over the collected DOIs (forced, or when it's worth it).
    let dois: Vec<String> = {
        let mut v: Vec<String> = extracted
            .values()
            .filter_map(|e| e.doi.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let mut batch: std::collections::HashMap<String, OpenAlexMapped> = Default::default();
    if opts.batch_openalex || dois.len() > 5 {
        batch = client.openalex_batch(&dois);
        stats.batch_hits = batch.len() as u64;
    }

    let mut stamped: Vec<StampedPaper> = Vec::new();
    for (id, path, sha256) in rows {
        stats.processed += 1;
        let ids = extracted.remove(&id);
        if let Err(e) = drain_one(store, id, &path, &sha256, corpus_dir, client, &batch, ids, &mut stats, &mut stamped) {
            let reason = format!("drain error: {e}");
            let _ = store.with_transaction(|conn| mark_failed(conn, id, &reason));
            stats.failed += 1;
        }
    }

    // Phase 2: chunk + embed + FTS for this run's stamped papers, plus any
    // stamped row that has no chunks yet — a panic in chunk_embed_one used to
    // strand every later row of its run permanently, so resume those orphans.
    if !opts.no_embed {
        let this_run: std::collections::HashSet<i64> = stamped.iter().map(|sp| sp.queue_id).collect();
        let orphans: Vec<StampedPaper> = store
            .raw()
            .prepare(
                "SELECT q.id, q.path, q.sha256 FROM inproc_queue q
                 WHERE q.status = 'stamped'
                   AND NOT EXISTS (SELECT 1 FROM chunk c WHERE c.sha256 = q.sha256)
                 ORDER BY q.id",
            )?
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|(id, path, _)| !this_run.contains(id) && Path::new(path).is_file())
            .filter_map(|(id, path, sha256)| {
                let p = PathBuf::from(&path);
                let corpus = p.parent()?.file_name()?.to_str()?.to_string();
                Some(StampedPaper { queue_id: id, sha256, path: p, corpus })
            })
            .collect();
        let all: Vec<&StampedPaper> = stamped.iter().chain(orphans.iter()).collect();
        for sp in all {
            if let Err(e) = chunk_embed_one(
                store,
                sp,
                embedder.as_mut().map(|e| &mut **e as &mut dyn crate::embed::BatchEmbedder),
                &mut stats,
            ) {
                let reason = dlq_reason("chunk-embed-error", serde_json::json!({"error": e.to_string()}));
                let _ = store.with_transaction(|conn| {
                    conn.execute(
                        "UPDATE inproc_queue SET reason = ?2, updated_at = ?3 WHERE id = ?1",
                        params![sp.queue_id, reason.to_string(), now_utc()],
                    )?;
                    Ok::<_, ImportError>(())
                });
                stats.embed_failed += 1;
            }
        }
    }
    Ok(stats)
}

/// Phase 2 for one stamped paper: full text → chunks → embeddings → chunk
/// rows + incremental chunk_fts inserts, all in one transaction.
fn chunk_embed_one(
    store: &mut LibraryStore,
    sp: &StampedPaper,
    embedder: Option<&mut dyn crate::embed::BatchEmbedder>,
    stats: &mut DrainStats,
) -> Result<(), ImportError> {
    let text = extract::full_text(&sp.path)?;
    use crate::chunker::*;
    let mut chunks = if sp.path.extension().is_some_and(|e| e == "md") {
        chunk_markdown(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS)
    } else {
        let c = chunk_structural(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
        if c.is_empty() {
            chunk_recursive(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS)
        } else {
            c
        }
    };
    if chunks.is_empty() {
        let reason = dlq_reason("no-full-text", serde_json::json!({"note": "stamped but not searchable yet"}));
        store.with_transaction(|conn| {
            conn.execute(
                "UPDATE inproc_queue SET reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![sp.queue_id, reason.to_string(), now_utc()],
            )?;
            Ok::<_, ImportError>(())
        })?;
        stats.no_full_text += 1;
        return Ok(());
    }

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let blobs: Option<Vec<Vec<u8>>> = match embedder {
        Some(e) => match e.embed_batch(&texts) {
            Ok(v) if v.len() == chunks.len() => {
                Some(v.iter().map(|v| crate::embed::f32_vec_to_blob(v)).collect())
            }
            _ => None,
        },
        None => None,
    };

    let title: Option<String> = store.raw().query_row(
        "SELECT title FROM paper WHERE sha256 = ?1",
        params![sp.sha256],
        |r| r.get(0),
    )?;
    let filename = sp
        .path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let n_chunks = chunks.len() as u64;
    let embedded = blobs.is_some();
    store.with_transaction(|conn| {
        let mut next_id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(chunk_id), 0) + 1 FROM chunk WHERE corpus = ?1",
            params![sp.corpus],
            |r| r.get(0),
        )?;
        for (i, c) in chunks.iter_mut().enumerate() {
            let row = stacks_core::library::LibraryChunk {
                rowid: 0,
                corpus: sp.corpus.clone(),
                chunk_id: next_id,
                sha256: Some(sp.sha256.clone()),
                filename: filename.clone(),
                title: title.clone(),
                section: Some(std::mem::take(&mut c.section)),
                text: std::mem::take(&mut c.text),
                word_count: Some(c.word_count),
                embedding: blobs.as_ref().map(|b| b[i].clone()),
            };
            next_id += 1;
            if let Some(rowid) = stacks_core::library::insert_chunk(conn, &row)? {
                // External-content FTS5: direct rowid insert keeps
                // chunk_fts in sync incrementally (the migration rebuilt
                // wholesale; new chunks append).
                conn.execute(
                    "INSERT INTO chunk_fts(rowid, text) VALUES (?1, ?2)",
                    params![rowid, row.text],
                )?;
            }
        }
        if embedded {
            conn.execute(
                "UPDATE inproc_queue SET status = 'enriched', reason = NULL, updated_at = ?2
                 WHERE id = ?1",
                params![sp.queue_id, now_utc()],
            )?;
        } else {
            let reason = dlq_reason("embed-failed", serde_json::json!({"note": "chunks+FTS written; vectors missing"}));
            conn.execute(
                "UPDATE inproc_queue SET reason = ?2, updated_at = ?3 WHERE id = ?1",
                params![sp.queue_id, reason.to_string(), now_utc()],
            )?;
        }
        Ok::<_, ImportError>(())
    })?;
    stats.chunks_inserted += n_chunks;
    if embedded {
        stats.enriched += 1;
    } else {
        stats.embed_failed += 1;
    }
    // Shadow pplx-embed pass: best-effort, gated on STACKS_PPLX_URL (so
    // --no-embed skips it too, since we only get here when embedding is on).
    // A pplx failure must never fail the paper — merge a note into the queue
    // row reason and move on; pplx-backfill picks up the gap later.
    if let Some(client) = crate::pplx_embed::HttpPplxClient::from_env() {
        let model = crate::pplx_embed::stamped_model(&client, crate::pplx_embed::DEFAULT_MODEL);
        if let Err(e) = (|| -> Result<(), ImportError> {
            let chunks = crate::pplx_embed::chunks_missing_pplx(
                store.raw(),
                &model,
                &sp.corpus,
                &sp.sha256,
            )?;
            crate::pplx_embed::embed_and_insert(store, &client, &model, &sp.corpus, &chunks)?;
            Ok(())
        })() {
            let note = format!("pplx shadow embed failed: {e}");
            let _ = store.with_transaction(|conn| {
                let existing: Option<String> = conn
                    .query_row(
                        "SELECT reason FROM inproc_queue WHERE id = ?1",
                        params![sp.queue_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                let mut reason = existing
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                    .unwrap_or_else(|| dlq_reason("enriched", serde_json::json!({})));
                if let Some(obj) = reason.as_object_mut() {
                    obj.insert("pplx".to_string(), serde_json::Value::String(note));
                }
                conn.execute(
                    "UPDATE inproc_queue SET reason = ?2, updated_at = ?3 WHERE id = ?1",
                    params![sp.queue_id, reason.to_string(), now_utc()],
                )?;
                Ok::<_, ImportError>(())
            });
        }
    }
    Ok(())
}

fn dlq_reason(code: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut v = serde_json::json!({"stage": "drain", "reason_code": code});
    if let (Some(a), Some(b)) = (v.as_object_mut(), extra.as_object()) {
        a.extend(b.clone());
    }
    v
}

fn stamp_queue_row(conn: &rusqlite::Connection, id: i64, dest: &str, extra_attempts: u32) -> Result<(), ImportError> {
    conn.execute(
        "UPDATE inproc_queue SET status = 'stamped', path = ?2, reason = NULL,
            attempts = attempts + 1 + ?4, updated_at = ?3 WHERE id = ?1",
        params![id, dest, now_utc(), extra_attempts],
    )?;
    Ok(())
}

/// Move the payload under corpus/stamped/<dir-slug>/, hash-verified.
fn stamp_payload(src: &Path, sha256: &str, corpus_dir: &Path, dir_slug: &str) -> Result<PathBuf, ImportError> {
    let dest = corpus_dir
        .join("stamped")
        .join(dir_slug)
        .join(format!("{sha256}{}", file_ext(src)));
    move_verified(src, &dest, sha256)?;
    Ok(dest)
}

/// First topic's subfield from an openalex_topics JSON string.
fn first_topic_subfield(topics_json: Option<&str>) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(topics_json?).ok()?;
    v.as_array()?
        .first()?
        .get("subfield")?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

/// Shared by the bonafides fast path and the legacy-enrichment precheck:
/// resolve a classifiable subfield for a known sha and stamp it. Returns
/// Ok(true) when stamped, Ok(false) when no subfield exists.
fn stamp_known_paper(
    store: &mut LibraryStore,
    id: i64,
    src: &Path,
    sha256: &str,
    corpus_dir: &Path,
    stats: &mut DrainStats,
    stamped: &mut Vec<StampedPaper>,
) -> Result<bool, ImportError> {
    let subfield: Option<String> = store.raw().query_row(
        "SELECT subfield FROM paper WHERE sha256 = ?1",
        params![sha256],
        |r| r.get(0),
    )?;
    let subfield = subfield.filter(|s| !s.trim().is_empty()).or_else(|| {
        store
            .raw()
            .query_row(
                "SELECT openalex_topics FROM paper_enrichment WHERE sha256 = ?1",
                params![sha256],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
            .and_then(|t| first_topic_subfield(Some(&t)))
    });
    let Some(subfield) = subfield else {
        return Ok(false);
    };
    let dest = stamp_payload(src, sha256, corpus_dir, &slugify(&subfield))?;
    let dest_str = dest.to_string_lossy().to_string();
    store.with_transaction(|conn| {
        stamp_queue_row(conn, id, &dest_str, 0)?;
        conn.execute(
            "UPDATE paper SET path = ?2 WHERE sha256 = ?1",
            params![sha256, dest_str],
        )?;
        Ok::<_, ImportError>(())
    })?;
    stats.stamped_papers += 1;
    stamped.push(StampedPaper {
        queue_id: id,
        sha256: sha256.to_string(),
        path: dest,
        corpus: slugify(&subfield),
    });
    Ok(true)
}

/// What the pre-API record checks found for one item.
struct PriorRecords {
    /// nl_ledger (nl_status, journal_events) when the sha256 is known to NL.
    ledger: Option<(String, i64)>,
    /// Already cataloged as a document by the old triage.
    in_document: bool,
}

/// Our own records, consulted before any API call (the standing rule).
fn prior_records(conn: &rusqlite::Connection, sha256: &str) -> Result<PriorRecords, ImportError> {
    let ledger: Option<(String, i64)> = conn
        .query_row(
            "SELECT nl_status, journal_events FROM nl_ledger WHERE sha256 = ?1
             ORDER BY id DESC LIMIT 1",
            params![sha256],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let in_document: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM document WHERE sha256 = ?1)",
        params![sha256],
        |r| r.get(0),
    )?;
    Ok(PriorRecords { ledger, in_document })
}

/// NL already burned its attempts on this file (api_failed, or pending_api
/// with 3+ journal events) — no more API spend, human review.
fn nl_exhausted(records: &PriorRecords) -> bool {
    matches!(&records.ledger,
        Some((status, events))
            if status == "api_failed" || (status == "pending_api" && *events >= 3))
}

/// Legacy paper+enrichment present but bonafides-incomplete: NL already
/// paid the API cost. True when this item should skip the API ladder.
fn legacy_enriched(conn: &rusqlite::Connection, sha256: &str) -> Result<bool, ImportError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM paper WHERE sha256 = ?1)
            AND EXISTS(SELECT 1 FROM paper_enrichment WHERE sha256 = ?1)",
        params![sha256],
        |r| r.get(0),
    )?)
}

fn drain_one(
    store: &mut LibraryStore,
    id: i64,
    path: &str,
    sha256: &str,
    corpus_dir: &Path,
    client: &dyn EnrichClient,
    batch: &std::collections::HashMap<String, OpenAlexMapped>,
    pre: Option<ExtractedIds>,
    stats: &mut DrainStats,
    stamped: &mut Vec<StampedPaper>,
) -> Result<(), ImportError> {
    let src = Path::new(path);
    if !src.is_file() {
        store.with_transaction(|conn| mark_failed(conn, id, "payload missing from intake"))?;
        stats.failed += 1;
        return Ok(());
    }

    // Human assertion wins over re-derivation.
    let assertion = latest_assertion(store.raw(), id)?;
    if let Some((kind, category, title)) = &assertion {
        if kind == "document" {
            return drain_document(
                store, id, src, sha256, corpus_dir, category.as_deref(), title.as_deref(), 0, stats,
            );
        }
        // asserted paper: enrich from the asserted title, then stamp.
        let outcome = resolve_identity(client, None, None, title.as_deref(), batch);
        if outcome.resolved_via.is_none() {
            let reason = dlq_reason(
                "assertion-unresolved",
                serde_json::json!({"asserted_title": title, "attempts": outcome.identity_attempts}),
            );
            store.with_transaction(|conn| mark_dlq(conn, id, &reason, outcome.identity_attempts))?;
            stats.dlq += 1;
            return Ok(());
        }
        return stamp_paper(store, id, src, sha256, corpus_dir, outcome, stats, stamped);
    }

    // Fast path: already stamp-in ready.
    let verdict = check_bonafides(store.raw(), sha256)?;
    if verdict.is_ready() {
        if stamp_known_paper(store, id, src, sha256, corpus_dir, stats, stamped)? {
            return Ok(());
        }
        store.with_transaction(|conn| mark_failed(conn, id, "bonafides ok but no subfield to classify into"))?;
        stats.failed += 1;
        return Ok(());
    }

    // Prechecks against our own records — before any extraction or API call.
    let records = prior_records(store.raw(), sha256)?;

    // 1. NL already paid for enrichment on this sha (paper + enrichment
    //    rows exist but bonafides-incomplete). Never re-spend API calls to
    //    fill gaps: stamp if classifiable, else human review.
    if verdict.paper_exists && legacy_enriched(store.raw(), sha256)? {
        if stamp_known_paper(store, id, src, sha256, corpus_dir, stats, stamped)? {
            return Ok(());
        }
        let reason = dlq_reason(
            "incomplete-legacy-enrichment",
            serde_json::json!({"failed_checks": verdict.failed_checks()}),
        );
        store.with_transaction(|conn| mark_dlq(conn, id, &reason, 0))?;
        stats.dlq += 1;
        return Ok(());
    }

    // 2. NL exhausted its attempts on this file → straight to DLQ.
    if nl_exhausted(&records) {
        let (status, events) = records.ledger.clone().unwrap();
        let reason = dlq_reason(
            "nl-exhausted",
            serde_json::json!({"nl_status": status, "journal_events": events,
                "failed_checks": verdict.failed_checks()}),
        );
        store.with_transaction(|conn| mark_dlq(conn, id, &reason, 0))?;
        stats.dlq += 1;
        return Ok(());
    }

    // Slow path: extraction + enrichment ladder.
    let ids = match pre {
        Some(ids) => ids,
        None => extract::extract(src)?,
    };
    match ids.kind {
        Some(ItemKind::Unsupported) | None => {
            let reason = dlq_reason(
                "unsupported-format",
                serde_json::json!({"failed_checks": verdict.failed_checks()}),
            );
            store.with_transaction(|conn| mark_dlq(conn, id, &reason, 0))?;
            stats.dlq += 1;
        }
        Some(ItemKind::Text) => {
            // Document path needs a human category assertion.
            let reason = dlq_reason("needs-category", serde_json::json!({"kind": "document"}));
            store.with_transaction(|conn| mark_dlq(conn, id, &reason, 0))?;
            stats.dlq += 1;
        }
        Some(ItemKind::Pdf) => {
            let outcome = resolve_identity(
                client,
                ids.doi.as_deref(),
                ids.arxiv_id.as_deref(),
                ids.title.as_deref(),
                batch,
            );
            if outcome.resolved_via.is_none() {
                let code = if ids.doi.is_none() && ids.arxiv_id.is_none()
                    && (ids.title.is_none() || ids.no_text_layer)
                {
                    "no-identifier"
                } else {
                    "enrichment-exhausted"
                };
                // 3. Already cataloged as a document by the old triage:
                //    nothing to review, just close the queue row out.
                if code == "no-identifier" && records.in_document {
                    let reason = dlq_reason("already-document", serde_json::json!({
                        "note": "sha256 already in document; nothing to review",
                    }));
                    store.with_transaction(|conn| {
                        conn.execute(
                            "UPDATE inproc_queue SET status = 'done', reason = ?2,
                                attempts = attempts + 1, updated_at = ?3 WHERE id = ?1",
                            params![id, reason.to_string(), now_utc()],
                        )?;
                        Ok::<_, ImportError>(())
                    })?;
                    stats.done += 1;
                    return Ok(());
                }
                let reason = dlq_reason(
                    code,
                    serde_json::json!({
                        "doi": ids.doi, "arxiv_id": ids.arxiv_id, "title": ids.title,
                        "no_text_layer": ids.no_text_layer,
                        "failed_checks": verdict.failed_checks(),
                        "attempts": outcome.identity_attempts,
                        "nl_status": records.ledger.as_ref().map(|(s, _)| s.as_str()),
                    }),
                );
                store.with_transaction(|conn| mark_dlq(conn, id, &reason, outcome.identity_attempts))?;
                stats.dlq += 1;
                return Ok(());
            }
            stamp_paper(store, id, src, sha256, corpus_dir, outcome, stats, stamped)?;
        }
    }
    Ok(())
}

/// Write paper + enrichment rows from a resolved identity, classify by
/// first-topic subfield, move the payload, flip the queue row.
fn stamp_paper(
    store: &mut LibraryStore,
    id: i64,
    src: &Path,
    sha256: &str,
    corpus_dir: &Path,
    outcome: crate::enrich::FetchOutcome,
    stats: &mut DrainStats,
    stamped: &mut Vec<StampedPaper>,
) -> Result<(), ImportError> {
    let subfield = first_topic_subfield(outcome.enrichment.openalex_topics.as_deref());
    let Some(subfield) = subfield else {
        let reason = dlq_reason(
            "no-subfield",
            serde_json::json!({"resolved_via": outcome.resolved_via, "doi": outcome.biblio.doi}),
        );
        store.with_transaction(|conn| mark_dlq(conn, id, &reason, outcome.identity_attempts))?;
        stats.dlq += 1;
        return Ok(());
    };
    let dest = stamp_payload(src, sha256, corpus_dir, &slugify(&subfield))?;
    let dest_str = dest.to_string_lossy().to_string();
    let filename = dest
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let size = std::fs::metadata(&dest).ok().map(|m| m.len() as i64);
    let paper = LibraryPaper {
        sha256: sha256.to_string(),
        filename,
        path: Some(dest_str.clone()),
        size_bytes: size,
        title: outcome.biblio.title.clone(),
        authors: outcome.biblio.authors.clone(),
        year: outcome.biblio.year,
        abstract_: outcome.biblio.abstract_.clone(),
        journal: outcome.biblio.journal.clone(),
        doi: outcome.biblio.doi.clone(),
        arxiv_id: outcome.biblio.arxiv_id.clone(),
        subfield: Some(subfield),
        registered_at: Some(now_utc()),
        on_disk: Some(true),
        ..empty_paper()
    };
    let enrichment = PaperEnrichment {
        sha256: sha256.to_string(),
        openalex_id: outcome.enrichment.openalex_id.clone(),
        openalex_topics: outcome.enrichment.openalex_topics.clone(),
        openalex_concepts: outcome.enrichment.openalex_concepts.clone(),
        openalex_cited_by: outcome.enrichment.openalex_cited_by,
        s2_paper_id: outcome.enrichment.s2_paper_id.clone(),
        s2_tldr: outcome.enrichment.s2_tldr.clone(),
        s2_fields_of_study: outcome.enrichment.s2_fields_of_study.clone(),
        s2_influential_citation_count: outcome.enrichment.s2_influential_citation_count,
        unpaywall_oa_status: outcome.enrichment.unpaywall_oa_status.clone(),
        unpaywall_oa_url: outcome.enrichment.unpaywall_oa_url.clone(),
        enriched_at: Some(now_utc()),
    };
    let attempts = outcome.identity_attempts;
    store.with_transaction(|conn| {
        upsert_paper_sparse(conn, &paper)?;
        upsert_enrichment(conn, &enrichment)?;
        stamp_queue_row(conn, id, &dest_str, attempts)?;
        Ok::<_, ImportError>(())
    })?;
    stats.stamped_papers += 1;
    stamped.push(StampedPaper {
        queue_id: id,
        sha256: sha256.to_string(),
        path: dest,
        corpus: slugify(paper.subfield.as_deref().unwrap_or("")),
    });
    Ok(())
}

fn empty_paper() -> LibraryPaper {
    serde_json::from_value(serde_json::json!({"sha256": "", "filename": ""})).unwrap()
}

/// Document path: only reachable with an asserted category.
fn drain_document(
    store: &mut LibraryStore,
    id: i64,
    src: &Path,
    sha256: &str,
    corpus_dir: &Path,
    category: Option<&str>,
    title: Option<&str>,
    extra_attempts: u32,
    stats: &mut DrainStats,
) -> Result<(), ImportError> {
    let Some(category) = category.filter(|c| !c.trim().is_empty()) else {
        let reason = dlq_reason("needs-category", serde_json::json!({"kind": "document"}));
        store.with_transaction(|conn| mark_dlq(conn, id, &reason, extra_attempts))?;
        stats.dlq += 1;
        return Ok(());
    };
    let dest = stamp_payload(src, sha256, corpus_dir, &format!("documents/{}", slugify(category)))?;
    let rel = dest
        .strip_prefix(corpus_dir)
        .unwrap_or(&dest)
        .to_string_lossy()
        .to_string();
    let doc = LibraryDocument {
        sha256: sha256.to_string(),
        family: "intake".to_string(),
        kind: "article".to_string(),
        title: title.map(str::to_string).or_else(|| {
            src.file_name().map(|f| f.to_string_lossy().to_string())
        }),
        path: rel,
        location_root: corpus_dir.to_string_lossy().to_string(),
        bytes: std::fs::metadata(&dest).ok().map(|m| m.len() as i64),
        retrieved_at: Some(now_utc()),
        collection: Some(category.to_string()),
        ..empty_document()
    };
    let dest_str = dest.to_string_lossy().to_string();
    store.with_transaction(|conn| {
        stacks_core::library::insert_document(conn, &doc)?;
        stamp_queue_row(conn, id, &dest_str, extra_attempts)?;
        Ok::<_, ImportError>(())
    })?;
    stats.stamped_documents += 1;
    Ok(())
}

fn empty_document() -> LibraryDocument {
    serde_json::from_value(serde_json::json!({
        "sha256": "", "family": "", "kind": "", "path": "", "location_root": ""
    }))
    .unwrap()
}

#[derive(Debug, Serialize)]
pub struct DlqEntry {
    pub id: i64,
    pub path: String,
    pub sha256: String,
    pub status: String,
    pub reason: Option<String>,
    pub attempts: i64,
    pub enqueued_at: String,
    pub updated_at: Option<String>,
}

/// Items needing human review: dead-lettered, failed, or queued with a
/// recorded reason (bonafides not met).
pub fn dlq_list(store: &LibraryStore) -> Result<Vec<DlqEntry>, ImportError> {
    let mut stmt = store.raw().prepare(
        "SELECT id, path, sha256, status, reason, attempts, enqueued_at, updated_at
         FROM inproc_queue
         WHERE status IN ('dlq', 'failed') OR (status = 'queued' AND reason IS NOT NULL)
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DlqEntry {
                id: r.get(0)?,
                path: r.get(1)?,
                sha256: r.get(2)?,
                status: r.get(3)?,
                reason: r.get(4)?,
                attempts: r.get(5)?,
                enqueued_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Record a human assertion about a dead-lettered item and return it to the
/// queue. Classification/enrichment re-entry is a later chunk; for now the
/// dlq_review row is the whole decision record.
pub fn dlq_assert(
    store: &mut LibraryStore,
    queue_id: i64,
    kind: &str,
    category: Option<&str>,
    title: Option<&str>,
    decided_by: &str,
    notes: Option<&str>,
) -> Result<(), ImportError> {
    if !["paper", "document"].contains(&kind) {
        return Err(ImportError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("--kind must be paper|document, got {kind:?}"),
        )));
    }
    store.with_transaction(|conn| {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM inproc_queue WHERE id = ?1)",
            params![queue_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(ImportError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no inproc_queue row with id {queue_id}"),
            )));
        }
        conn.execute(
            "INSERT INTO dlq_review (queue_id, asserted_kind, asserted_category, asserted_title,
                decided_by, decided_at, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![queue_id, kind, category, title, decided_by, now_utc(), notes],
        )?;
        conn.execute(
            "UPDATE inproc_queue SET status = 'queued', reason = NULL, updated_at = ?2
             WHERE id = ?1",
            params![queue_id, now_utc()],
        )?;
        Ok::<_, ImportError>(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stacks_core::library::{insert_document, insert_enrichment, insert_paper, LibraryDocument, LibraryPaper, PaperEnrichment};

    const SHA_CONTENT: &str = "hello intake payload";

    fn sha256_of(content: &str) -> String {
        format!("{:x}", Sha256::digest(content.as_bytes()))
    }

    fn test_store(tmp: &tempfile::TempDir) -> LibraryStore {
        LibraryStore::open(tmp.path().join("library.db")).unwrap()
    }

    fn insert_ready_paper(store: &LibraryStore, sha256: &str) {
        let paper: LibraryPaper = serde_json::from_value(serde_json::json!({
            "sha256": sha256,
            "filename": format!("{sha256}.pdf"),
            "title": "Synthesis and characterization of novel perovskite oxides",
            "doi": "10.1/test",
            "subfield": "Materials Chemistry",
        }))
        .unwrap();
        insert_paper(store.raw(), &paper).unwrap();
        let enr: PaperEnrichment = serde_json::from_value(serde_json::json!({
            "sha256": sha256,
            "openalex_topics": r#"[{"name": "Perovskites", "score": 0.9, "subfield": "Materials Chemistry"}]"#,
            "s2_tldr": "A paper about perovskites.",
            "unpaywall_oa_status": "gold",
        }))
        .unwrap();
        insert_enrichment(store.raw(), &enr).unwrap();
    }

    #[test]
    fn slugify_rules() {
        assert_eq!(slugify("Materials Chemistry"), "materials-chemistry");
        assert_eq!(slugify("condensed_matter physics"), "condensed-matter-physics");
        assert_eq!(slugify("  Padded  "), "padded");
    }

    #[test]
    fn enqueue_walk_move_and_dedupe() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(src_dir.join("nested")).unwrap();
        fs::write(src_dir.join("a.pdf"), SHA_CONTENT).unwrap();
        fs::write(src_dir.join("nested").join("b.txt"), SHA_CONTENT).unwrap();
        let intake = tmp.path().join("intake");
        let mut store = test_store(&tmp);

        let stats = enqueue_sources(&mut store, &[src_dir.clone()], &intake).unwrap();
        assert_eq!(stats.files_seen, 2);
        assert_eq!(stats.enqueued, 2);
        assert!(stats.errors.is_empty());
        assert!(!src_dir.join("a.pdf").exists(), "source moved away");

        let sha = sha256_of(SHA_CONTENT);
        let dest = intake.join("incoming").join(&sha[..2]);
        assert!(dest.join(format!("{sha}.pdf")).exists());
        assert!(dest.join(format!("{sha}.txt")).exists());

        // Re-enqueue same (path, sha256): skipped as already_in_intake.
        let stats = enqueue_sources(&mut store, &[src_dir], &intake).unwrap();
        assert_eq!(stats.files_seen, 0, "sources were moved away");
        assert_eq!(stats.enqueued, 0);

        let queued: i64 = store
            .raw()
            .query_row("SELECT COUNT(*) FROM inproc_queue", [], |r| r.get(0))
            .unwrap();
        assert_eq!(queued, 2);
    }

    #[test]
    fn enqueue_tar_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let payload_dir = tmp.path().join("payload");
        fs::create_dir_all(&payload_dir).unwrap();
        fs::write(payload_dir.join("paper.pdf"), SHA_CONTENT).unwrap();
        let archive = tmp.path().join("bundle.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&payload_dir)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());

        let intake = tmp.path().join("intake");
        let mut store = test_store(&tmp);
        let stats = enqueue_sources(&mut store, &[archive], &intake).unwrap();
        assert_eq!(stats.archives_extracted, 1);
        assert_eq!(stats.enqueued, 1, "errors: {:?}", stats.errors);
    }

    /// Client that resolves nothing — exercises the DLQ paths offline.
    struct NullClient;

    impl crate::enrich::EnrichClient for NullClient {
        fn openalex_batch(&self, _: &[String]) -> std::collections::HashMap<String, crate::enrich::OpenAlexMapped> {
            Default::default()
        }
        fn openalex_doi(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> {
            None
        }
        fn openalex_title_search(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> {
            None
        }
        fn crossref(&self, _: &str) -> Option<crate::enrich::BiblioMeta> {
            None
        }
        fn arxiv(&self, _: &str) -> Option<crate::enrich::BiblioMeta> {
            None
        }
        fn s2(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
            None
        }
        fn unpaywall(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
            None
        }
    }

    fn null_drain(store: &mut LibraryStore, corpus: &Path) -> DrainStats {
        drain_queue_with_embedder(store, corpus, &NullClient, None, &DrainOpts::default()).unwrap()
    }

    /// Deterministic offline embedder: constant 384-dim unit-ish vectors.
    struct StubEmbedder;

    impl crate::embed::BatchEmbedder for StubEmbedder {
        fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts.iter().map(|_| vec![0.25f32; crate::embed::EMBED_DIM]).collect())
        }
    }

    /// Minimal uncompressed-stream PDF the builtin extractor can read.
    fn fake_pdf_with_doi(doi: &str) -> Vec<u8> {
        format!(
            "%PDF-1.4\n1 0 obj << /Length 100 >> stream\nBT /F1 12 Tf (doi: {doi} Journal of Tests) Tj ET\nendstream\nendobj\n%%EOF"
        )
        .into_bytes()
    }

    #[test]
    fn drain_stamps_ready_and_dlqs_unidentified() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("ready.pdf"), SHA_CONTENT).unwrap();
        fs::write(src_dir.join("unready.pdf"), "no catalog row here").unwrap();
        let intake = tmp.path().join("intake");
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        let stats = enqueue_sources(&mut store, &[src_dir], &intake).unwrap();
        assert_eq!(stats.enqueued, 2);

        insert_ready_paper(&store, &sha256_of(SHA_CONTENT));
        let stats = null_drain(&mut store, &corpus);
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.stamped_papers, 1);
        assert_eq!(stats.dlq, 1);
        assert_eq!(stats.failed, 0);

        let sha = sha256_of(SHA_CONTENT);
        let stamped = corpus
            .join("stamped")
            .join("materials-chemistry")
            .join(format!("{sha}.pdf"));
        assert!(stamped.exists());
        assert_eq!(sha256_file(&stamped).unwrap(), sha);
        let paper_path: String = store
            .raw()
            .query_row("SELECT path FROM paper WHERE sha256 = ?1", params![sha], |r| r.get(0))
            .unwrap();
        assert_eq!(paper_path, stamped.to_string_lossy());

        // Unidentified row: DLQ with a structured reason.
        let (status, reason, attempts): (String, String, i64) = store
            .raw()
            .query_row(
                "SELECT status, reason, attempts FROM inproc_queue WHERE sha256 = ?1",
                params![sha256_of("no catalog row here")],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "dlq");
        assert_eq!(attempts, 1);
        let reason: serde_json::Value = serde_json::from_str(&reason).unwrap();
        assert_eq!(reason["reason_code"], "no-identifier");
        assert!(reason["failed_checks"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("paper_exists")));
    }

    #[test]
    fn drain_enriches_pdf_by_doi() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let pdf = fake_pdf_with_doi("10.1234/test.doi");
        fs::write(src_dir.join("newpaper.pdf"), &pdf).unwrap();
        let intake = tmp.path().join("intake");
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &intake).unwrap();

        struct DoiClient;
        impl crate::enrich::EnrichClient for DoiClient {
            fn openalex_batch(&self, _: &[String]) -> std::collections::HashMap<String, crate::enrich::OpenAlexMapped> {
                Default::default()
            }
            fn openalex_doi(&self, doi: &str) -> Option<crate::enrich::OpenAlexMapped> {
                (doi == "10.1234/test.doi").then(|| crate::enrich::OpenAlexMapped {
                    openalex_id: Some("https://openalex.org/W1".into()),
                    doi: Some(doi.into()),
                    title: Some("A genuinely interesting test paper about tests".into()),
                    topics_json: Some(r#"[{"name":"Testing","score":0.9,"subfield":"Materials Chemistry"}]"#.into()),
                    concepts_json: None,
                    cited_by: Some(5),
                })
            }
            fn openalex_title_search(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> { None }
            fn crossref(&self, _: &str) -> Option<crate::enrich::BiblioMeta> {
                Some(crate::enrich::BiblioMeta {
                    authors: Some(r#"[{"given":"Jane","family":"Doe"}]"#.into()),
                    year: Some(2024),
                    journal: Some("J. Tests".into()),
                    ..Default::default()
                })
            }
            fn arxiv(&self, _: &str) -> Option<crate::enrich::BiblioMeta> { None }
            fn s2(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
                Some(crate::enrich::EnrichmentMeta {
                    s2_paper_id: Some("s2-1".into()),
                    s2_tldr: Some("A test.".into()),
                    ..Default::default()
                })
            }
            fn unpaywall(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
                Some(crate::enrich::EnrichmentMeta {
                    unpaywall_oa_status: Some("gold".into()),
                    unpaywall_oa_url: Some("https://x/y.pdf".into()),
                    ..Default::default()
                })
            }
        }

        let stats = drain_queue_with_embedder(&mut store, &corpus, &DoiClient, None, &DrainOpts::default()).unwrap();
        assert_eq!(stats.stamped_papers, 1, "{stats:?}");
        assert_eq!(stats.dlq, 0);
        let sha = sha256_of(std::str::from_utf8(&pdf).unwrap());
        let (title, doi, subfield, year): (String, String, String, i64) = store
            .raw()
            .query_row(
                "SELECT title, doi, subfield, year FROM paper WHERE sha256 = ?1",
                params![sha],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(title, "A genuinely interesting test paper about tests");
        assert_eq!(doi, "10.1234/test.doi");
        assert_eq!(subfield, "Materials Chemistry");
        assert_eq!(year, 2024);
        let (tldr, oa): (String, String) = store
            .raw()
            .query_row(
                "SELECT s2_tldr, unpaywall_oa_status FROM paper_enrichment WHERE sha256 = ?1",
                params![sha],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tldr, "A test.");
        assert_eq!(oa, "gold");
        assert!(corpus
            .join("stamped/materials-chemistry")
            .join(format!("{sha}.pdf"))
            .exists());
    }

    #[test]
    fn drain_unsupported_format_dlqs() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("book.epub"), "fake epub bytes").unwrap();
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &tmp.path().join("intake")).unwrap();
        let stats = null_drain(&mut store, &tmp.path().join("corpus"));
        assert_eq!(stats.dlq, 1);
        let reason: String = store
            .raw()
            .query_row("SELECT reason FROM inproc_queue", [], |r| r.get(0))
            .unwrap();
        assert!(reason.contains("unsupported-format"));
    }

    #[test]
    fn drain_document_assertion_stamps() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("notes.txt"), "plain text notes payload").unwrap();
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &tmp.path().join("intake")).unwrap();

        // No assertion: needs-category.
        let stats = null_drain(&mut store, &corpus);
        assert_eq!(stats.dlq, 1);
        let listed = dlq_list(&store).unwrap();
        assert_eq!(listed.len(), 1);

        // Human asserts: document, category 'field-notes'.
        dlq_assert(&mut store, listed[0].id, "document", Some("field-notes"), None, "patrick", None).unwrap();
        let stats = null_drain(&mut store, &corpus);
        assert_eq!(stats.stamped_documents, 1, "{stats:?}");
        let sha = sha256_of("plain text notes payload");
        let (kind, collection, path): (String, String, String) = store
            .raw()
            .query_row(
                "SELECT kind, collection, path FROM document WHERE sha256 = ?1",
                params![sha],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(kind, "article");
        assert_eq!(collection, "field-notes");
        assert!(corpus.join(&path).exists(), "{}", corpus.join(&path).display());
    }

    #[test]
    fn drain_batch_mode_uses_batch_results() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let pdf = fake_pdf_with_doi("10.5555/batch.me");
        fs::write(src_dir.join("batched.pdf"), &pdf).unwrap();
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &tmp.path().join("intake")).unwrap();

        struct BatchClient {
            individual_calls: std::sync::Mutex<u32>,
        }
        impl crate::enrich::EnrichClient for BatchClient {
            fn openalex_batch(&self, dois: &[String]) -> std::collections::HashMap<String, crate::enrich::OpenAlexMapped> {
                dois.iter()
                    .map(|d| (d.clone(), crate::enrich::OpenAlexMapped {
                        openalex_id: Some("https://openalex.org/Wb".into()),
                        doi: Some(d.clone()),
                        title: Some("Batched paper with a properly long title".into()),
                        topics_json: Some(r#"[{"name":"B","score":0.8,"subfield":"Condensed Matter Physics"}]"#.into()),
                        concepts_json: None,
                        cited_by: None,
                    }))
                    .collect()
            }
            fn openalex_doi(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> {
                *self.individual_calls.lock().unwrap() += 1;
                None
            }
            fn openalex_title_search(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> { None }
            fn crossref(&self, _: &str) -> Option<crate::enrich::BiblioMeta> { None }
            fn arxiv(&self, _: &str) -> Option<crate::enrich::BiblioMeta> { None }
            fn s2(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> { None }
            fn unpaywall(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> { None }
        }
        let client = BatchClient { individual_calls: std::sync::Mutex::new(0) };
        let opts = DrainOpts { batch_openalex: true, ..Default::default() };
        let stats = drain_queue_with_embedder(&mut store, &corpus, &client, None, &opts).unwrap();
        assert_eq!(stats.batch_hits, 1);
        assert_eq!(stats.stamped_papers, 1);
        assert_eq!(*client.individual_calls.lock().unwrap(), 0, "batch hit should skip the individual lookup");
    }

    #[test]
    fn dlq_assert_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("x.pdf"), SHA_CONTENT).unwrap();
        let intake = tmp.path().join("intake");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &intake).unwrap();
        null_drain(&mut store, &tmp.path().join("corpus"));

        let listed = dlq_list(&store).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].reason.is_some());

        dlq_assert(
            &mut store,
            listed[0].id,
            "paper",
            Some("materials-chemistry"),
            Some("Asserted title of the queued paper"),
            "patrick",
            None,
        )
        .unwrap();
        let (status, reason): (String, Option<String>) = store
            .raw()
            .query_row(
                "SELECT status, reason FROM inproc_queue WHERE id = ?1",
                params![listed[0].id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "queued");
        assert!(reason.is_none());
        let reviews: i64 = store
            .raw()
            .query_row("SELECT COUNT(*) FROM dlq_review", [], |r| r.get(0))
            .unwrap();
        assert_eq!(reviews, 1);

        assert!(dlq_assert(&mut store, listed[0].id, "bogus", None, None, "patrick", None).is_err());
        assert!(dlq_assert(&mut store, 9999, "paper", None, None, "patrick", None).is_err());
    }

    /// Phase 2 end-to-end: stamped paper gets chunked, embedded (stub),
    /// FTS-indexed, and flipped to 'enriched'.
    #[test]
    fn drain_chunks_embeds_and_indexes() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let body: Vec<String> = (0..60).map(|i| format!("perovskite synthesis word{i}")).collect();
        let pdf = format!(
            "%PDF-1.4\n1 0 obj << >> stream\nBT (doi: 10.7777/chunk.me {}) Tj ET\nendstream\nendobj\n%%EOF",
            body.join(" ")
        );
        fs::write(src_dir.join("chunkme.pdf"), &pdf).unwrap();
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &tmp.path().join("intake")).unwrap();

        struct C;
        impl crate::enrich::EnrichClient for C {
            fn openalex_batch(&self, _: &[String]) -> std::collections::HashMap<String, crate::enrich::OpenAlexMapped> { Default::default() }
            fn openalex_doi(&self, doi: &str) -> Option<crate::enrich::OpenAlexMapped> {
                Some(crate::enrich::OpenAlexMapped {
                    openalex_id: Some("https://openalex.org/Wc".into()),
                    doi: Some(doi.into()),
                    title: Some("A chunked paper about perovskite synthesis methods".into()),
                    topics_json: Some(r#"[{"name":"P","score":0.9,"subfield":"Materials Chemistry"}]"#.into()),
                    concepts_json: None,
                    cited_by: None,
                })
            }
            fn openalex_title_search(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> { None }
            fn crossref(&self, _: &str) -> Option<crate::enrich::BiblioMeta> { None }
            fn arxiv(&self, _: &str) -> Option<crate::enrich::BiblioMeta> { None }
            fn s2(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> { None }
            fn unpaywall(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> { None }
        }

        let mut emb = StubEmbedder;
        let stats = drain_queue_with_embedder(&mut store, &corpus, &C, Some(&mut emb), &DrainOpts::default()).unwrap();
        assert_eq!(stats.stamped_papers, 1, "{stats:?}");
        assert_eq!(stats.enriched, 1, "{stats:?}");
        assert!(stats.chunks_inserted >= 1);

        let sha = sha256_of(&pdf);
        let (n, emb_len): (i64, i64) = store
            .raw()
            .query_row(
                "SELECT COUNT(*), SUM(LENGTH(embedding)) FROM chunk WHERE sha256 = ?1",
                params![sha],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(n >= 1);
        assert_eq!(emb_len, Some(n * 384 * 4).map(|v| v).unwrap());
        let (crp, section, wc): (String, String, i64) = store
            .raw()
            .query_row(
                "SELECT corpus, section, word_count FROM chunk WHERE sha256 = ?1 LIMIT 1",
                params![sha],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(crp, "materials-chemistry");
        assert!(!section.is_empty());
        assert!(wc >= 25);
        // FTS is searchable.
        let hits: i64 = store
            .raw()
            .query_row(
                "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH 'perovskite'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, n);
        let status: String = store
            .raw()
            .query_row("SELECT status FROM inproc_queue WHERE sha256 = ?1", params![sha], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "enriched");
    }

    /// --no-embed: stamp only, phase 2 skipped entirely.
    #[test]
    fn drain_no_embed_skips_phase2() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("ready.pdf"), SHA_CONTENT).unwrap();
        let corpus = tmp.path().join("corpus");
        let mut store = test_store(&tmp);
        enqueue_sources(&mut store, &[src_dir], &tmp.path().join("intake")).unwrap();
        insert_ready_paper(&store, &sha256_of(SHA_CONTENT));
        let opts = DrainOpts { no_embed: true, ..Default::default() };
        let stats = drain_queue_with_embedder(&mut store, &corpus, &NullClient, None, &opts).unwrap();
        assert_eq!(stats.stamped_papers, 1);
        assert_eq!(stats.enriched, 0);
        let chunks: i64 = store
            .raw()
            .query_row("SELECT COUNT(*) FROM chunk", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunks, 0);
        let status: String = store
            .raw()
            .query_row("SELECT status FROM inproc_queue", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "stamped");
    }

    /// Counting client: proves the prechecks cost zero API calls.
    #[derive(Default)]
    struct CountingClient {
        calls: std::sync::Mutex<u32>,
    }

    impl crate::enrich::EnrichClient for CountingClient {
        fn openalex_batch(&self, _: &[String]) -> std::collections::HashMap<String, crate::enrich::OpenAlexMapped> {
            *self.calls.lock().unwrap() += 1;
            Default::default()
        }
        fn openalex_doi(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> {
            *self.calls.lock().unwrap() += 1;
            None
        }
        fn openalex_title_search(&self, _: &str) -> Option<crate::enrich::OpenAlexMapped> {
            *self.calls.lock().unwrap() += 1;
            None
        }
        fn crossref(&self, _: &str) -> Option<crate::enrich::BiblioMeta> {
            *self.calls.lock().unwrap() += 1;
            None
        }
        fn arxiv(&self, _: &str) -> Option<crate::enrich::BiblioMeta> {
            *self.calls.lock().unwrap() += 1;
            None
        }
        fn s2(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
            *self.calls.lock().unwrap() += 1;
            None
        }
        fn unpaywall(&self, _: &str) -> Option<crate::enrich::EnrichmentMeta> {
            *self.calls.lock().unwrap() += 1;
            None
        }
    }

    fn counting_drain(store: &mut LibraryStore, corpus: &Path, client: &CountingClient) -> DrainStats {
        drain_queue_with_embedder(store, corpus, client, None, &DrainOpts::default()).unwrap()
    }

    fn enqueue_payload(store: &mut LibraryStore, tmp: &tempfile::TempDir, name: &str, content: &[u8]) {
        let src_dir = tmp.path().join(format!("src-{name}"));
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join(name), content).unwrap();
        enqueue_sources(store, &[src_dir], &tmp.path().join("intake")).unwrap();
    }

    fn ledger_row(store: &LibraryStore, path: &str, sha256: &str, status: &str, events: i64) {
        store
            .raw()
            .execute(
                "INSERT INTO nl_ledger (path, sha256, nl_status, journal_events, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![path, sha256, status, events, now_utc()],
            )
            .unwrap();
    }

    /// Precheck 1: legacy partial enrichment (paper+enrichment rows,
    /// bonafides-incomplete, subfield present) stamps without any API call.
    #[test]
    fn drain_legacy_enriched_stamps_without_api() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = test_store(&tmp);
        let content = b"legacy payload with no extractable identifier at all";
        enqueue_payload(&mut store, &tmp, "legacy.pdf", content);
        let sha = sha256_of(std::str::from_utf8(content).unwrap());
        // paper row with subfield, junk-ish enrichment (no tldr/oa) → bonafides fail
        let paper: LibraryPaper = serde_json::from_value(serde_json::json!({
            "sha256": sha, "filename": "legacy.pdf",
            "title": "A real but incompletely enriched legacy paper title",
            "subfield": "Materials Chemistry",
        }))
        .unwrap();
        insert_paper(store.raw(), &paper).unwrap();
        let enr: PaperEnrichment = serde_json::from_value(serde_json::json!({
            "sha256": sha, "openalex_id": "https://openalex.org/Wl",
        }))
        .unwrap();
        insert_enrichment(store.raw(), &enr).unwrap();

        let client = CountingClient::default();
        let stats = counting_drain(&mut store, &tmp.path().join("corpus"), &client);
        assert_eq!(stats.stamped_papers, 1, "{stats:?}");
        assert_eq!(*client.calls.lock().unwrap(), 0, "no API calls for legacy-enriched");
    }

    /// Precheck 1, no subfield → dlq 'incomplete-legacy-enrichment', zero calls.
    #[test]
    fn drain_legacy_enriched_no_subfield_dlqs() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = test_store(&tmp);
        let content = b"another legacy payload without identifiers anywhere";
        enqueue_payload(&mut store, &tmp, "legacy2.pdf", content);
        let sha = sha256_of(std::str::from_utf8(content).unwrap());
        let paper: LibraryPaper = serde_json::from_value(serde_json::json!({
            "sha256": sha, "filename": "legacy2.pdf",
            "title": "A legacy paper that was never fully enriched either",
        }))
        .unwrap();
        insert_paper(store.raw(), &paper).unwrap();
        let enr: PaperEnrichment = serde_json::from_value(serde_json::json!({
            "sha256": sha, "openalex_id": "https://openalex.org/Wl2",
        }))
        .unwrap();
        insert_enrichment(store.raw(), &enr).unwrap();

        let client = CountingClient::default();
        let stats = counting_drain(&mut store, &tmp.path().join("corpus"), &client);
        assert_eq!(stats.dlq, 1);
        assert_eq!(*client.calls.lock().unwrap(), 0);
        let reason: String = store
            .raw()
            .query_row("SELECT reason FROM inproc_queue", [], |r| r.get(0))
            .unwrap();
        assert!(reason.contains("incomplete-legacy-enrichment"));
    }

    /// Precheck 2: nl-exhausted files go straight to DLQ, zero API calls.
    #[test]
    fn drain_nl_exhausted_skips_api() {
        for (status, events, expect_dlq) in [
            ("api_failed", 0, true),
            ("pending_api", 3, true),
            ("pending_api", 1, false),
            ("fallback_staged", 5, false),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let mut store = test_store(&tmp);
            let content = if expect_dlq {
                format!("payload for {status}/{events}").into_bytes()
            } else {
                // DOI-bearing PDF so the ladder actually fires when it proceeds
                fake_pdf_with_doi(&format!("10.9999/{status}.{events}"))
            };
            enqueue_payload(&mut store, &tmp, "nl.pdf", &content);
            let sha = sha256_of(&String::from_utf8_lossy(&content).into_owned());
            ledger_row(&store, "/nl/intake/nl.pdf", &sha, status, events);

            let client = CountingClient::default();
            let stats = counting_drain(&mut store, &tmp.path().join("corpus"), &client);
            if expect_dlq {
                assert_eq!(stats.dlq, 1, "{status}/{events}");
                assert_eq!(*client.calls.lock().unwrap(), 0, "{status}/{events} cost API calls");
                let reason: String = store
                    .raw()
                    .query_row("SELECT reason FROM inproc_queue", [], |r| r.get(0))
                    .unwrap();
                assert!(reason.contains("nl-exhausted"));
            } else {
                // proceeds to the ladder (which fails against the null-ish client)
                assert!(stats.dlq == 1, "{status}/{events}");
                assert!(*client.calls.lock().unwrap() > 0, "{status}/{events} should attempt APIs");
            }
        }
    }

    /// Precheck 3: already a document + no identifier → 'done', not DLQ.
    #[test]
    fn drain_already_document_closes_out() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = test_store(&tmp);
        let content = b"a payload already cataloged as a document long ago";
        enqueue_payload(&mut store, &tmp, "old.pdf", content);
        let sha = sha256_of(std::str::from_utf8(content).unwrap());
        let doc: LibraryDocument = serde_json::from_value(serde_json::json!({
            "sha256": sha, "family": "intake", "kind": "article",
            "path": "intake/old.pdf", "location_root": "/nl",
        }))
        .unwrap();
        insert_document(store.raw(), &doc).unwrap();

        let client = CountingClient::default();
        let stats = counting_drain(&mut store, &tmp.path().join("corpus"), &client);
        assert_eq!(stats.done, 1, "{stats:?}");
        assert_eq!(stats.dlq, 0);
        let (status, reason): (String, String) = store
            .raw()
            .query_row("SELECT status, reason FROM inproc_queue", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(status, "done");
        assert!(reason.contains("already-document"));
    }

    /// Part 3: a wave_staging fallback_copy of an already-known paper costs
    /// zero API calls — the sha256 fast path fires before the ladder.
    #[test]
    fn drain_wave_staging_dupe_costs_zero_api() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = test_store(&tmp);
        enqueue_payload(&mut store, &tmp, "copy.pdf", SHA_CONTENT.as_bytes());
        let sha = sha256_of(SHA_CONTENT);
        insert_ready_paper(&store, &sha);
        ledger_row(&store, "/nl/intake/wave_staging/misc_wave/copy.pdf", &sha, "fallback_copy", 0);

        let client = CountingClient::default();
        let stats = counting_drain(&mut store, &tmp.path().join("corpus"), &client);
        assert_eq!(stats.stamped_papers, 1, "{stats:?}");
        assert_eq!(*client.calls.lock().unwrap(), 0, "wave_staging dupe hit an API");
    }
}
