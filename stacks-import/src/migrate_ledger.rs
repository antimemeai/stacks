//! Migration of NL's neuroticd intake ledger into library.db's nl_ledger
//! table (schema v8). The state file's `enriched` map is the authoritative
//! record of NL's ~10k prior intake attempts; the journal contributes
//! per-path event counts and last-event timestamps; wave_staging dirs are
//! recorded as 'fallback_copy' lineage rows. Bulk, one transaction, INSERT
//! OR REPLACE — idempotent re-runs.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;
use stacks_core::library::LibraryStore;

use crate::ImportError;

#[derive(Debug, Default, Serialize)]
pub struct LedgerStats {
    pub ledger_entries: u64,
    pub wave_staging_entries: u64,
    pub sha_resolved: u64,
    pub unresolved: u64,
    pub journal_events_matched: u64,
    pub journal_lines: u64,
}

struct LedgerRow {
    path: String,
    sha256: Option<String>,
    nl_status: String,
    nl_detail: Option<String>,
}

/// Resolve an NL intake path to a catalog sha256: document.path (intake-
/// relative), then intake_triage.path, then a unique paper.filename match.
fn resolve_sha256(conn: &rusqlite::Connection, nl_root: &Path, path: &str) -> Result<Option<String>, ImportError> {
    let rel = Path::new(path)
        .strip_prefix(nl_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    if let Some(sha) = conn
        .query_row(
            "SELECT sha256 FROM document WHERE path = ?1 LIMIT 1",
            params![rel],
            |r| r.get::<_, String>(0),
        )
        .ok()
    {
        return Ok(Some(sha));
    }
    if let Some(sha) = conn
        .query_row(
            "SELECT sha256 FROM intake_triage WHERE path = ?1 AND sha256 IS NOT NULL LIMIT 1",
            params![rel],
            |r| r.get::<_, String>(0),
        )
        .ok()
    {
        return Ok(Some(sha));
    }
    let basename = Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut stmt = conn.prepare("SELECT sha256 FROM paper WHERE filename = ?1")?;
    let shas: Vec<String> = stmt
        .query_map(params![basename], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if shas.len() == 1 {
        return Ok(Some(shas.into_iter().next().unwrap()));
    }
    Ok(None)
}

pub fn migrate_ledger(
    store: &mut LibraryStore,
    state_path: &Path,
    journal_path: Option<&Path>,
    nl_root: &Path,
) -> Result<LedgerStats, ImportError> {
    let mut stats = LedgerStats::default();
    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(state_path)?)?;
    let enriched = state
        .get("enriched")
        .and_then(|e| e.as_object())
        .cloned()
        .unwrap_or_default();

    let mut rows: Vec<LedgerRow> = Vec::new();
    for (path, entry) in &enriched {
        let nl_status = entry
            .get("outcome")
            .and_then(|o| o.as_str())
            .unwrap_or("unknown")
            .to_string();
        let mut detail = entry.clone();
        detail.as_object_mut().map(|d| d.remove("outcome"));
        rows.push(LedgerRow {
            path: path.clone(),
            sha256: None,
            nl_status,
            nl_detail: Some(detail.to_string()),
        });
    }
    stats.ledger_entries = rows.len() as u64;

    // wave_staging: fallback copies of files living elsewhere in intake.
    let staging = nl_root.join("intake/wave_staging");
    if staging.is_dir() {
        let mut waves: Vec<std::path::PathBuf> = std::fs::read_dir(&staging)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        waves.sort();
        for wave in waves {
            let wave_name = wave.file_name().unwrap().to_string_lossy().to_string();
            let mut files: Vec<std::path::PathBuf> = Vec::new();
            crate::inproc::walk_files(&wave, &mut files)?;
            files.sort();
            for file in files {
                rows.push(LedgerRow {
                    path: file.to_string_lossy().to_string(),
                    sha256: None,
                    nl_status: "fallback_copy".to_string(),
                    nl_detail: Some(serde_json::json!({"wave": wave_name}).to_string()),
                });
            }
        }
        stats.wave_staging_entries = rows.len() as u64 - stats.ledger_entries;
    }

    // Journal summary per path (events reference files in their "pdf" key).
    let mut events: HashMap<String, (i64, Option<String>)> = HashMap::new();
    if let Some(journal) = journal_path {
        let known: std::collections::HashSet<&str> =
            rows.iter().map(|r| r.path.as_str()).collect();
        for line in std::fs::read_to_string(journal)?.lines() {
            stats.journal_lines += 1;
            let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(pdf) = event.get("pdf").and_then(|p| p.as_str()) else {
                continue;
            };
            if !known.contains(pdf) {
                continue;
            }
            let ts = event.get("ts").and_then(|t| t.as_str()).map(str::to_string);
            let e = events.entry(pdf.to_string()).or_insert((0, None));
            e.0 += 1;
            if ts.as_deref() > e.1.as_deref() {
                e.1 = ts;
            }
            stats.journal_events_matched += 1;
        }
    }

    let now = crate::inproc::now_utc();
    store.with_transaction(|conn| {
        for row in &mut rows {
            row.sha256 = resolve_sha256(conn, nl_root, &row.path)?;
            let (journal_events, last_event_at) = events
                .get(&row.path)
                .cloned()
                .unwrap_or((0, None));
            conn.execute(
                "INSERT OR REPLACE INTO nl_ledger
                    (path, sha256, nl_status, nl_detail, journal_events, last_event_at, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row.path,
                    row.sha256,
                    row.nl_status,
                    row.nl_detail,
                    journal_events,
                    last_event_at,
                    now
                ],
            )?;
        }
        Ok::<_, ImportError>(())
    })?;
    stats.sha_resolved = rows.iter().filter(|r| r.sha256.is_some()).count() as u64;
    stats.unresolved = rows.len() as u64 - stats.sha_resolved;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stacks_core::library::{insert_document, LibraryDocument};

    #[test]
    fn migrate_ledger_resolves_and_summarizes() {
        let tmp = tempfile::tempdir().unwrap();
        let nl_root = tmp.path().join("nl");
        let staging = nl_root.join("intake/wave_staging/misc_wave");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("copy.pdf"), b"copy").unwrap();

        let state = serde_json::json!({
            "enriched": {
                nl_root.join("intake/foo.pdf").to_string_lossy(): {"outcome": "api_failed", "ts": "2026-06-15T01:45:05", "api_attempts": 3},
                nl_root.join("intake/bar.pdf").to_string_lossy(): {"outcome": "pending_api", "ts": "2026-06-15T01:45:06"},
            }
        });
        let state_path = tmp.path().join("state.json");
        std::fs::write(&state_path, state.to_string()).unwrap();
        let journal = format!(
            "{}\n{}\n{}\n",
            serde_json::json!({"ts": "2026-06-15T01:00:00", "event": "fallback_staged", "pdf": nl_root.join("intake/foo.pdf").to_string_lossy()}),
            serde_json::json!({"ts": "2026-07-10T05:55:59", "event": "fallback_staged", "pdf": nl_root.join("intake/foo.pdf").to_string_lossy()}),
            serde_json::json!({"ts": "2026-07-10T05:56:00", "event": "fallback_staged", "pdf": "/somewhere/else.pdf"}),
        );
        let journal_path = tmp.path().join("journal.jsonl");
        std::fs::write(&journal_path, journal).unwrap();

        let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
        // foo.pdf was triaged into a document → sha resolvable.
        let sha = "ab".repeat(32);
        let doc: LibraryDocument = serde_json::from_value(serde_json::json!({
            "sha256": sha, "family": "intake", "kind": "article",
            "path": "intake/foo.pdf", "location_root": nl_root.to_string_lossy(),
        }))
        .unwrap();
        insert_document(store.raw(), &doc).unwrap();

        let stats = migrate_ledger(&mut store, &state_path, Some(&journal_path), &nl_root).unwrap();
        assert_eq!(stats.ledger_entries, 2);
        assert_eq!(stats.wave_staging_entries, 1);
        assert_eq!(stats.sha_resolved, 1, "{stats:?}");
        assert_eq!(stats.journal_events_matched, 2);

        let (status, events, last): (String, i64, String) = store
            .raw()
            .query_row(
                "SELECT nl_status, journal_events, last_event_at FROM nl_ledger WHERE path LIKE '%foo.pdf'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "api_failed");
        assert_eq!(events, 2);
        assert_eq!(last, "2026-07-10T05:55:59");
        let staging_status: String = store
            .raw()
            .query_row(
                "SELECT nl_status FROM nl_ledger WHERE path LIKE '%wave_staging%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(staging_status, "fallback_copy");

        // Idempotent re-run.
        let stats2 = migrate_ledger(&mut store, &state_path, Some(&journal_path), &nl_root).unwrap();
        let total: i64 = store
            .raw()
            .query_row("SELECT COUNT(*) FROM nl_ledger", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 3);
        assert_eq!(stats2.ledger_entries, 2);
    }
}
