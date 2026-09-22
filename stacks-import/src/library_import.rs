//! Migration import of the frozen neurotic_library catalog + chunk corpora
//! into the stacks library database. One-shot migration, but idempotent:
//! papers by sha256 PK, chunks by UNIQUE(corpus, chunk_id), movements by
//! their full row, so re-runs are no-ops.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use stacks_core::library::*;
use stacks_core::StoreError;

use crate::ImportError;

#[derive(Debug, Default, serde::Serialize)]
pub struct CorpusStat {
    pub chunks_inserted: u64,
    pub chunks_existing: u64,
    pub embeddings_bytes: u64,
    pub chunks_with_embedding: u64,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct LibraryImportStats {
    pub papers_inserted: u64,
    pub papers_existing: u64,
    pub enrichments_inserted: u64,
    pub movements_inserted: u64,
    pub movements_existing: u64,
    pub corpora: BTreeMap<String, CorpusStat>,
    /// Duplicate sqlite files skipped (byte-identical md5 to an already
    /// imported file).
    pub duplicate_files_skipped: Vec<String>,
    pub quarantined_filename_collision: u64,
    pub quarantined_orphan: u64,
    pub collision_disambiguated: u64,
}

#[derive(Debug, Deserialize)]
struct ChunkRow {
    id: i64,
    filename: String,
    title: Option<String>,
    section: Option<String>,
    text: String,
    word_count: Option<i64>,
}

/// Corpus name from an embeddings filename: strip the trailing
/// `_YYYYMMDD_HHMM(SS)?.sqlite` stamp.
pub fn corpus_name(filename: &str) -> String {
    let re = Regex::new(r"_\d{8}_\d{4,6}\.sqlite$").unwrap();
    re.replace(filename, "").to_string()
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, ImportError> {
    BufReader::with_capacity(1 << 20, File::open(path)?)
        .lines()
        .map(|l| serde_json::from_str(&l?).map_err(Into::into))
        .collect()
}

type Resolution = (
    Option<String>,
    Option<(QuarantineReason, Vec<String>)>,
    bool,
);

/// Resolve a chunk's filename to a catalog sha256.
/// - exactly one candidate: joined
/// - several: try path-basename + corpus-subfield disambiguation
/// - none / unresolved: quarantine, sha256 NULL
fn resolve_filename(
    filename: &str,
    corpus: &str,
    by_filename: &HashMap<String, Vec<(String, String)>>,
) -> Resolution {
    let Some(candidates) = by_filename.get(filename) else {
        return (None, Some((QuarantineReason::Orphan, vec![])), false);
    };
    if candidates.len() == 1 {
        return (Some(candidates[0].0.clone()), None, false);
    }
    let narrowed: Vec<&(String, String)> = candidates
        .iter()
        .filter(|(_, path)| path.contains(corpus))
        .collect();
    if narrowed.len() == 1 {
        return (Some(narrowed[0].0.clone()), None, true);
    }
    (
        None,
        Some((
            QuarantineReason::FilenameCollision,
            candidates.iter().map(|(s, _)| s.clone()).collect(),
        )),
        false,
    )
}

pub fn import_library(
    store: &mut LibraryStore,
    export_dir: &Path,
    embeddings_dir: &Path,
    imported_at: &str,
) -> Result<LibraryImportStats, ImportError> {
    let mut stats = LibraryImportStats::default();

    // ---- catalog ----
    let papers: Vec<LibraryPaper> = read_jsonl(&export_dir.join("papers.jsonl"))?;
    let mut by_filename: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for p in &papers {
        by_filename
            .entry(p.filename.clone())
            .or_default()
            .push((p.sha256.clone(), p.path.clone().unwrap_or_default()));
    }
    store.with_transaction(|conn| {
        for p in &papers {
            if insert_paper(conn, p)? {
                stats.papers_inserted += 1;
            } else {
                stats.papers_existing += 1;
            }
        }
        Ok::<_, ImportError>(())
    })?;

    let enrichments: Vec<PaperEnrichment> = read_jsonl(&export_dir.join("enrichments.jsonl"))?;
    store.with_transaction(|conn| {
        for e in &enrichments {
            if insert_enrichment(conn, e)? {
                stats.enrichments_inserted += 1;
            }
        }
        Ok::<_, ImportError>(())
    })?;

    let movements: Vec<PaperMovement> = read_jsonl(&export_dir.join("movements.jsonl"))?;
    store.with_transaction(|conn| {
        for m in &movements {
            if insert_movement(conn, m)? {
                stats.movements_inserted += 1;
            } else {
                stats.movements_existing += 1;
            }
        }
        // Reconcile current paths: latest movement wins.
        conn.execute(
            "UPDATE paper SET path = (
                SELECT to_path FROM paper_movement
                WHERE paper_movement.sha256 = paper.sha256 AND to_path IS NOT NULL
                ORDER BY moved_at DESC LIMIT 1)
             WHERE sha256 IN (SELECT sha256 FROM paper_movement)",
            [],
        )?;
        Ok::<_, ImportError>(())
    })?;

    // ---- chunk corpora ----
    let mut files: Vec<PathBuf> = std::fs::read_dir(embeddings_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "sqlite"))
        .collect();
    files.sort();
    let mut seen_md5: HashMap<String, PathBuf> = HashMap::new();

    for path in files {
        let fname = path.file_name().unwrap().to_string_lossy().to_string();
        let digest = format!("{:x}", md5::compute(std::fs::read(&path)?));
        if let Some(first) = seen_md5.get(&digest) {
            stats.duplicate_files_skipped.push(format!(
                "{fname} == {}",
                first.file_name().unwrap().to_string_lossy()
            ));
            continue;
        }
        seen_md5.insert(digest, path.clone());
        let corpus = corpus_name(&fname);
        import_corpus(store, &path, &corpus, &by_filename, &mut stats)?;
    }

    // ---- import provenance ----
    let meta_mtime = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| format!("{t:?}"))
    };
    let row_counts = serde_json::json!({
        "papers": stats.papers_inserted + stats.papers_existing,
        "enrichments": stats.enrichments_inserted,
        "movements": stats.movements_inserted + stats.movements_existing,
        "corpora": stats.corpora,
        "embeddings_dir_mtime": meta_mtime(embeddings_dir),
    });
    store.with_transaction(|conn| {
        insert_import_meta(
            conn,
            "neurotic_library",
            &export_dir.join("papers.jsonl").to_string_lossy(),
            meta_mtime(&export_dir.join("papers.jsonl")).as_deref(),
            imported_at,
            &row_counts,
        )?;
        Ok::<_, StoreError>(())
    })?;

    Ok(stats)
}

fn import_corpus(
    store: &mut LibraryStore,
    path: &Path,
    corpus: &str,
    by_filename: &HashMap<String, Vec<(String, String)>>,
    stats: &mut LibraryImportStats,
) -> Result<(), ImportError> {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.pragma_update(None, "query_only", "ON")?;

    let chunks: Vec<ChunkRow> = conn
        .prepare("SELECT id, filename, title, section, text, word_count FROM chunks")?
        .query_map([], |r| {
            Ok(ChunkRow {
                id: r.get(0)?,
                filename: r.get(1)?,
                title: r.get(2)?,
                section: r.get(3)?,
                text: r.get(4)?,
                word_count: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    // sqlite-vec slab layout (verified against the source corpus): chunk
    // ids are contiguous 1..N in every file and the vector ordinal equals
    // the chunk id; each slab row holds 1024 vectors of 384 float32
    // (vec_chunks_rowids' chunk_id/chunk_offset are batch bookkeeping, not
    // chunk references). Guard the contiguity assumption per file.
    let (min_id, max_id, n_chunks) =
        conn.query_row("SELECT MIN(id), MAX(id), COUNT(*) FROM chunks", [], |r| {
            Ok((
                r.get::<_, Option<i64>>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
    let contiguous = n_chunks > 0 && min_id == Some(1) && max_id == Some(n_chunks);
    let mut slabs: Vec<Vec<u8>> = Vec::new();
    if contiguous {
        let mut stmt =
            conn.prepare("SELECT vectors FROM vec_chunks_vector_chunks00 ORDER BY rowid")?;
        slabs = stmt
            .query_map([], |r| r.get::<_, Vec<u8>>(0))?
            .collect::<Result<_, _>>()?;
    }
    const DIM_BYTES: usize = 384 * 4;
    let embedding_for = |chunk_id: i64| -> Option<Vec<u8>> {
        if !contiguous {
            return None;
        }
        let ordinal = (chunk_id - 1) as usize;
        let slab = slabs.get(ordinal / 1024)?;
        let off = (ordinal % 1024) * DIM_BYTES;
        if off + DIM_BYTES <= slab.len() {
            Some(slab[off..off + DIM_BYTES].to_vec())
        } else {
            None
        }
    };
    drop(conn);

    let corpus_stat = CorpusStat::default();
    let stat = stats
        .corpora
        .entry(corpus.to_string())
        .or_insert(corpus_stat);
    store.with_transaction(|tx| {
        for c in &chunks {
            let embedding = embedding_for(c.id);
            let (sha256, quarantine, disambiguated) =
                resolve_filename(&c.filename, corpus, by_filename);
            if let Some((reason, candidates)) = &quarantine {
                insert_quarantine(tx, corpus, c.id, &c.filename, reason.clone(), candidates)?;
                match reason {
                    QuarantineReason::FilenameCollision => {
                        stats.quarantined_filename_collision += 1
                    }
                    QuarantineReason::Orphan => stats.quarantined_orphan += 1,
                }
            }
            if disambiguated {
                stats.collision_disambiguated += 1;
            }
            let embedding_len = embedding.as_ref().map(|e| e.len() as u64);
            let chunk = LibraryChunk {
                rowid: 0,
                corpus: corpus.to_string(),
                chunk_id: c.id,
                sha256,
                filename: c.filename.clone(),
                title: c.title.clone(),
                section: c.section.clone(),
                text: c.text.clone(),
                word_count: c.word_count,
                embedding,
            };
            if insert_chunk(tx, &chunk)?.is_some() {
                stat.chunks_inserted += 1;
                if let Some(len) = embedding_len {
                    stat.embeddings_bytes += len;
                    stat.chunks_with_embedding += 1;
                }
            } else {
                stat.chunks_existing += 1;
            }
        }
        Ok::<_, ImportError>(())
    })?;
    Ok(())
}

/// Rebuild the external-content FTS index after bulk import.
pub fn rebuild_fts(store: &LibraryStore) -> Result<(), StoreError> {
    store
        .raw()
        .execute_batch("INSERT INTO chunk_fts(chunk_fts) VALUES('rebuild');")?;
    Ok(())
}
