//! `inproc-status`: live drain monitoring. Queue depth by status, drain
//! rates over 5m/1h/24h windows (rows that left 'queued' with updated_at in
//! the window), oldest queued item, and an ETA at the 24h rate. Read-only
//! connection with busy_timeout — safe alongside the live drain.

use std::collections::BTreeMap;

use serde::Serialize;
use stacks_core::library::LibraryStore;

use crate::ImportError;

const WINDOWS: &[(&str, u64)] = &[
    ("5m", 300),
    ("1h", 3_600),
    ("24h", 86_400),
];

#[derive(Debug, Default, Serialize)]
pub struct WindowRate {
    pub drained: u64,
    pub per_hour: f64,
}

#[derive(Debug, Default, Serialize)]
pub struct QueueStatus {
    pub now: String,
    /// Counts by status (only statuses present appear).
    pub depth: BTreeMap<String, u64>,
    pub queued: u64,
    pub oldest_queued_at: Option<String>,
    /// Rates per window ("5m"/"1h"/"24h").
    pub rates: BTreeMap<String, WindowRate>,
    /// Non-queued rows with NULL/stale updated_at — excluded from windows.
    pub untracked_rows: u64,
    /// ISO8601 ETA for draining the current queue at the 24h rate.
    pub eta_at_24h_rate: Option<String>,
}

pub fn queue_status(store: &LibraryStore) -> Result<QueueStatus, ImportError> {
    let conn = store.raw();
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let now = crate::inproc::format_utc(now_secs);

    let mut depth = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM inproc_queue GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (status, n) = row?;
        depth.insert(status, n as u64);
    }
    let queued = depth.get("queued").copied().unwrap_or(0);

    let oldest_queued_at: Option<String> = conn.query_row(
        "SELECT MIN(enqueued_at) FROM inproc_queue WHERE status = 'queued'",
        [],
        |r| r.get(0),
    )?;

    // Rows that left 'queued' but carry no trustworthy transition timestamp
    // (predate the updated_at discipline) — excluded from the rate windows.
    let untracked_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM inproc_queue
         WHERE status != 'queued' AND updated_at IS NULL",
        [],
        |r| r.get(0),
    )?;

    let mut rates = BTreeMap::new();
    for &(label, secs) in WINDOWS {
        let since = crate::inproc::format_utc(now_secs.saturating_sub(secs));
        let drained: i64 = conn.query_row(
            "SELECT COUNT(*) FROM inproc_queue
             WHERE status != 'queued' AND updated_at IS NOT NULL AND updated_at >= ?1",
            [since],
            |r| r.get(0),
        )?;
        rates.insert(
            label.to_string(),
            WindowRate {
                drained: drained as u64,
                per_hour: drained as f64 * 3600.0 / secs as f64,
            },
        );
    }

    let per_hour_24h = rates.get("24h").map(|r| r.per_hour).unwrap_or(0.0);
    let eta_at_24h_rate = if queued > 0 && per_hour_24h > 0.0 {
        let hours = queued as f64 / per_hour_24h;
        Some(crate::inproc::format_utc(now_secs + (hours * 3600.0) as u64))
    } else {
        None
    };

    Ok(QueueStatus {
        now,
        depth,
        queued,
        oldest_queued_at,
        rates,
        untracked_rows: untracked_rows as u64,
        eta_at_24h_rate,
    })
}

/// Compact human render: depth line, rate table, oldest/ETA.
pub fn render_human(s: &QueueStatus) -> String {
    let mut out = String::new();
    out.push_str(&format!("inproc queue @ {}\n", s.now));
    let depth = if s.depth.is_empty() {
        "  (empty)".to_string()
    } else {
        s.depth
            .iter()
            .map(|(k, v)| format!("  {k:<11} {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    out.push_str(&depth);
    out.push('\n');
    out.push_str("  window   drained  per-hour\n");
    for &(label, _) in WINDOWS {
        if let Some(rate) = s.rates.get(label) {
            out.push_str(&format!(
                "  {label:<8} {:>7}  {:>8.1}\n",
                rate.drained, rate.per_hour
            ));
        }
    }
    out.push_str(&format!(
        "  oldest queued: {}\n",
        s.oldest_queued_at.as_deref().unwrap_or("-")
    ));
    out.push_str(&format!(
        "  ETA @24h rate: {}\n",
        s.eta_at_24h_rate.as_deref().unwrap_or("-")
    ));
    if s.untracked_rows > 0 {
        out.push_str(&format!(
            "  note: {} drained rows lack updated_at and are excluded from rates\n",
            s.untracked_rows
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(store: &LibraryStore, path: &str, status: &str, enqueued: &str, updated: Option<&str>) {
        store
            .raw()
            .execute(
                "INSERT INTO inproc_queue (path, sha256, enqueued_at, status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![path, "ab".repeat(32), enqueued, status, updated],
            )
            .unwrap();
    }

    #[test]
    fn depth_rates_and_nulls() {
        let tmp = tempfile::tempdir().unwrap();
        let store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let t = |ago: u64| crate::inproc::format_utc(now_secs - ago);

        seed(&store, "q1", "queued", &t(10_000), Some(&t(10_000)));
        seed(&store, "q2", "queued", &t(500), Some(&t(500)));
        // drained inside every window
        seed(&store, "d1", "stamped", &t(100_000), Some(&t(60)));
        // drained inside 1h and 24h, not 5m
        seed(&store, "d2", "enriched", &t(200_000), Some(&t(1_000)));
        // drained inside 24h only
        seed(&store, "d3", "dlq", &t(300_000), Some(&t(10_000)));
        // drained before all windows
        seed(&store, "d4", "done", &t(900_000), Some(&t(800_000)));
        // historical row: no updated_at → untracked
        seed(&store, "d5", "stamped", &t(900_000), None);

        let s = queue_status(&store).unwrap();
        assert_eq!(s.queued, 2);
        assert_eq!(s.depth.get("stamped"), Some(&2));
        assert_eq!(s.depth.get("queued"), Some(&2));
        assert_eq!(s.untracked_rows, 1);
        assert_eq!(s.rates["5m"].drained, 1);
        assert_eq!(s.rates["5m"].per_hour, 12.0);
        assert_eq!(s.rates["1h"].drained, 2);
        assert_eq!(s.rates["24h"].drained, 3);
        assert_eq!(s.oldest_queued_at.as_deref(), Some(t(10_000).as_str()));
        // 2 queued / (3 per 24h) = 16 hours out
        assert_eq!(
            s.eta_at_24h_rate.as_deref(),
            Some(crate::inproc::format_utc(now_secs + 16 * 3600).as_str())
        );
        let rendered = render_human(&s);
        assert!(rendered.contains("queued"));
        assert!(rendered.contains("excluded from rates"));
    }

    #[test]
    fn empty_queue_no_eta() {
        let tmp = tempfile::tempdir().unwrap();
        let store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
        let s = queue_status(&store).unwrap();
        assert_eq!(s.queued, 0);
        assert!(s.eta_at_24h_rate.is_none());
        assert!(s.oldest_queued_at.is_none());
        assert!(render_human(&s).contains("(empty)"));
    }
}
