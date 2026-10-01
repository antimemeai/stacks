use stacks_core::library::{upsert_dataset, verify_datasets, Dataset, LibraryStore};
use tempfile::TempDir;

fn dataset(name: &str, path: &str, root: &str) -> Dataset {
    Dataset {
        id: 0,
        name: name.to_string(),
        path: path.to_string(),
        location_root: root.to_string(),
        size_bytes: None,
        file_count: None,
        dominant_formats: None,
        sha256_status: "none".to_string(),
        description: "fixture".to_string(),
        domains: vec!["test".to_string()],
        status: "registered".to_string(),
        status_note: None,
        provenance: "downloaded".to_string(),
        registered_at: "2026-09-22T00:00:00Z".to_string(),
    }
}

#[test]
fn verify_flags_missing_paths_and_restores() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    std::fs::create_dir(tmp.path().join("real_dir")).unwrap();
    let store = LibraryStore::open(tmp.path().join("l.db")).unwrap();
    upsert_dataset(store.raw(), &dataset("real", "real_dir", &root)).unwrap();
    upsert_dataset(store.raw(), &dataset("gone", "deleted_dir", &root)).unwrap();

    let flipped = verify_datasets(store.raw()).unwrap();
    assert!(
        flipped.iter().any(|f| f.contains("gone -> missing")),
        "missing path must flip: {flipped:?}"
    );
    let status: String = store
        .raw()
        .query_row("SELECT status FROM dataset WHERE name = 'gone'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "missing");
    let status: String = store
        .raw()
        .query_row("SELECT status FROM dataset WHERE name = 'real'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "registered");

    // Path returns -> status restores.
    std::fs::create_dir(tmp.path().join("deleted_dir")).unwrap();
    let flipped = verify_datasets(store.raw()).unwrap();
    assert!(flipped.iter().any(|f| f.contains("gone -> registered")));
}

/// Seed a DB at library schema V8 (dataset table without `provenance`) and
/// confirm opening it applies V9: existing rows default to 'downloaded' and
/// the CHECK fence rejects anything else.
#[test]
fn v9_migration_defaults_existing_rows_and_fences_bogus_values() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("l.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );
            INSERT INTO schema_migrations (version) VALUES (1),(2),(3),(4),(5),(6),(7),(8);
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
            INSERT INTO dataset (name, path, location_root, sha256_status, description, domains, status, registered_at)
                VALUES ('old-payload', 'x', '/tmp', 'none', 'pre-v9 row', '[\"test\"]', 'registered', '2026-09-22T00:00:00Z');",
        )
        .unwrap();
    }
    let store = LibraryStore::open(&db).unwrap();
    let version: i64 = store
        .raw()
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 9);
    let provenance: String = store
        .raw()
        .query_row("SELECT provenance FROM dataset WHERE name = 'old-payload'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(provenance, "downloaded");
    let err = store.raw().execute(
        "INSERT INTO dataset (name, path, location_root, sha256_status, description, domains, status, provenance, registered_at)
         VALUES ('bogus', 'x', '/tmp', 'none', 'bad provenance', '[]', 'registered', 'stolen', '2026-09-30T00:00:00Z')",
        [],
    );
    assert!(err.is_err(), "CHECK must reject bogus provenance: {err:?}");
}

/// Provenance is set on insert and preserved on re-registration of a
/// downloaded-default entry, unless the entry itself declares a non-default
/// provenance.
#[test]
fn upsert_preserves_earned_provenance_unless_entry_declares_it() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let store = LibraryStore::open(tmp.path().join("l.db")).unwrap();

    let mut earned = dataset("oxide", "runs", &root);
    earned.provenance = "earned".to_string();
    upsert_dataset(store.raw(), &earned).unwrap();

    // Re-registering the same name as a plain (downloaded) entry must not
    // clobber the earned provenance.
    upsert_dataset(store.raw(), &dataset("oxide", "runs", &root)).unwrap();
    let provenance: String = store
        .raw()
        .query_row("SELECT provenance FROM dataset WHERE name = 'oxide'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(provenance, "earned");

    // An entry that explicitly declares a non-default provenance wins.
    let mut mixed = dataset("oxide", "runs", &root);
    mixed.provenance = "mixed".to_string();
    upsert_dataset(store.raw(), &mixed).unwrap();
    let provenance: String = store
        .raw()
        .query_row("SELECT provenance FROM dataset WHERE name = 'oxide'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(provenance, "mixed");
}

/// registered_at is a "first registered" stamp: the upsert conflict path
/// must preserve the original value, and registration stamps the actual
/// current time on insert.
#[test]
fn upsert_preserves_registered_at_and_registration_stamps_now() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let mut store = LibraryStore::open(tmp.path().join("l.db")).unwrap();

    let mut first = dataset("payload", "dir", &root);
    first.registered_at = "2026-09-22T00:00:00Z".to_string();
    upsert_dataset(store.raw(), &first).unwrap();

    // Re-register with a different (later) stamp: must not take effect.
    let mut again = dataset("payload", "dir", &root);
    again.registered_at = "2026-09-30T13:43:09Z".to_string();
    upsert_dataset(store.raw(), &again).unwrap();
    let stamped: String = store
        .raw()
        .query_row("SELECT registered_at FROM dataset WHERE name = 'payload'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(stamped, "2026-09-22T00:00:00Z");

    // Fresh registration stamps the actual current time, not a hardcoded date.
    stacks_import::datasets::register_datasets(&mut store).unwrap();
    let now = stacks_import::inproc::now_utc();
    let fresh: String = store
        .raw()
        .query_row("SELECT registered_at FROM dataset WHERE name = 'stacks.db'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        &fresh[..4],
        &now[..4],
        "registered_at must be stamped now ({now}), got {fresh}"
    );
    assert_ne!(fresh, "2026-09-22T00:00:00Z");
}
