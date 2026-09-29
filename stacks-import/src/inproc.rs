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
use stacks_core::library::LibraryStore;

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

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ImportError> {
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
    pub stamped: u64,
    /// Bonafides not met; still queued, reasons recorded, attempts bumped.
    pub not_ready: u64,
    /// Hard per-item failure (io etc.); status='failed'.
    pub failed: u64,
}

fn mark_failed(conn: &rusqlite::Connection, id: i64, reason: &str) -> Result<(), ImportError> {
    conn.execute(
        "UPDATE inproc_queue SET status = 'failed', reason = ?2,
            attempts = attempts + 1, updated_at = ?3 WHERE id = ?1",
        params![id, reason, now_utc()],
    )?;
    Ok(())
}

/// Stamp-in drain: every queued row gets the bonafides check. Ready rows
/// move to `<corpus-dir>/stamped/<subfield-slug>/<sha256><ext>` (hash
/// verified after the move) and flip to 'stamped'; unready rows stay queued
/// with the failed checks recorded as a JSON `reason`.
pub fn drain_queue(store: &mut LibraryStore, corpus_dir: &Path) -> Result<DrainStats, ImportError> {
    let mut stats = DrainStats::default();
    let rows: Vec<(i64, String, String)> = store
        .raw()
        .prepare("SELECT id, path, sha256 FROM inproc_queue WHERE status = 'queued' ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;

    for (id, path, sha256) in rows {
        stats.processed += 1;
        if let Err(e) = drain_one(store, id, &path, &sha256, corpus_dir, &mut stats) {
            let reason = format!("drain error: {e}");
            let _ = store.with_transaction(|conn| mark_failed(conn, id, &reason));
            stats.failed += 1;
        }
    }
    Ok(stats)
}

fn drain_one(
    store: &mut LibraryStore,
    id: i64,
    path: &str,
    sha256: &str,
    corpus_dir: &Path,
    stats: &mut DrainStats,
) -> Result<(), ImportError> {
    let src = Path::new(path);
    if !src.is_file() {
        store.with_transaction(|conn| mark_failed(conn, id, "payload missing from intake"))?;
        stats.failed += 1;
        return Ok(());
    }
    let verdict = check_bonafides(store.raw(), sha256)?;
    if !verdict.is_ready() {
        let reason = serde_json::json!({
            "stage": "bonafides",
            "failed_checks": verdict.failed_checks(),
            "bonafides": verdict,
        })
        .to_string();
        store.with_transaction(|conn| {
            conn.execute(
                "UPDATE inproc_queue SET reason = ?2, attempts = attempts + 1, updated_at = ?3
                 WHERE id = ?1",
                params![id, reason, now_utc()],
            )?;
            Ok::<_, ImportError>(())
        })?;
        stats.not_ready += 1;
        return Ok(());
    }

    let subfield: Option<String> = store.raw().query_row(
        "SELECT subfield FROM paper WHERE sha256 = ?1",
        params![sha256],
        |r| r.get(0),
    )?;
    let Some(subfield) = subfield.filter(|s| !s.trim().is_empty()) else {
        store.with_transaction(|conn| mark_failed(conn, id, "bonafides ok but paper.subfield missing"))?;
        stats.failed += 1;
        return Ok(());
    };
    let dest = corpus_dir
        .join("stamped")
        .join(slugify(&subfield))
        .join(format!("{sha256}{}", file_ext(src)));
    move_verified(src, &dest, sha256)?;
    let dest_str = dest.to_string_lossy().to_string();
    store.with_transaction(|conn| {
        conn.execute(
            "UPDATE inproc_queue SET status = 'stamped', path = ?2, reason = NULL,
                attempts = attempts + 1, updated_at = ?3 WHERE id = ?1",
            params![id, dest_str, now_utc()],
        )?;
        conn.execute(
            "UPDATE paper SET path = ?2 WHERE sha256 = ?1",
            params![sha256, dest_str],
        )?;
        Ok::<_, ImportError>(())
    })?;
    stats.stamped += 1;
    Ok(())
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
    use stacks_core::library::{insert_enrichment, insert_paper, LibraryPaper, PaperEnrichment};

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

    #[test]
    fn drain_stamps_ready_and_records_unready() {
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
        let stats = drain_queue(&mut store, &corpus).unwrap();
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.stamped, 1);
        assert_eq!(stats.not_ready, 1);
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

        // Unready row: still queued, reason records the failed checks.
        let (status, reason, attempts): (String, String, i64) = store
            .raw()
            .query_row(
                "SELECT status, reason, attempts FROM inproc_queue WHERE sha256 = ?1",
                params![sha256_of("no catalog row here")],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "queued");
        assert_eq!(attempts, 1);
        let reason: serde_json::Value = serde_json::from_str(&reason).unwrap();
        assert!(reason["failed_checks"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("paper_exists")));
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
        drain_queue(&mut store, &tmp.path().join("corpus")).unwrap();

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
}
