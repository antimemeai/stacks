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
