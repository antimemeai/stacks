use std::fs;
use std::path::Path;

use stacks_core::library::LibraryStore;
use stacks_import::library_import::import_library;
use tempfile::TempDir;

/// Create a tiny source-format embeddings sqlite: chunks + vec shadow
/// tables, matching the observed neurotic_library schema.
fn make_corpus_db(path: &Path, chunks: &[(i64, &str, &str)], with_vec: &[i64]) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT, filename TEXT, title TEXT, section TEXT, text TEXT, word_count INTEGER
        );
        CREATE TABLE vec_chunks_vector_chunks00 (rowid PRIMARY KEY, vectors BLOB NOT NULL);
        CREATE TABLE vec_chunks_rowids (rowid INTEGER PRIMARY KEY AUTOINCREMENT, id, chunk_id INTEGER, chunk_offset INTEGER);",
    )
    .unwrap();
    for (id, filename, text) in chunks {
        conn.execute(
            "INSERT INTO chunks (id, filename, text, word_count) VALUES (?1, ?2, ?3, 10)",
            rusqlite::params![id, filename, text],
        )
        .unwrap();
    }
    for id in with_vec {
        // Real sqlite-vec layout: one slab blob of 1024 vectors; rowids in
        // vec_chunks_rowids are 1-based vector ordinals.
        conn.execute(
            "INSERT INTO vec_chunks_rowids (id, chunk_id, chunk_offset) VALUES (NULL, ?1, 0)",
            rusqlite::params![id],
        )
        .unwrap();
    }
    if !with_vec.is_empty() {
        conn.execute(
            "INSERT INTO vec_chunks_vector_chunks00 (rowid, vectors) VALUES (1, zeroblob(?1))",
            rusqlite::params![1536 * with_vec.len()],
        )
        .unwrap();
    }
}

// Fixture catalog:
// - "dup.pdf": two papers, one path inside the corpus subfield dir
//   (disambiguable -> sha a)
// - "harddup.pdf": two papers, neither path inside the corpus dir
//   (unresolvable -> quarantine, sha256 NULL)
// - "solo.pdf": one paper, plus a movement that reconciles its path
fn write_export_fixture(dir: &Path) {
    let (a, b, c, d, e) = (
        "a".repeat(64),
        "b".repeat(64),
        "c".repeat(64),
        "d".repeat(64),
        "e".repeat(64),
    );
    let paper = |sha: &str, filename: &str, path: &str| {
        format!(r#"{{"sha256":"{sha}","filename":"{filename}","path":"{path}"}}"#)
    };
    fs::write(
        dir.join("papers.jsonl"),
        [
            paper(&a, "dup.pdf", "lib/test_corpus/dup.pdf"),
            paper(&b, "dup.pdf", "lib/other_corpus/dup.pdf"),
            paper(&c, "solo.pdf", "lib/test_corpus/solo.pdf"),
            paper(&d, "harddup.pdf", "lib/foo/harddup.pdf"),
            paper(&e, "harddup.pdf", "lib/bar/harddup.pdf"),
        ]
        .join("\n"),
    )
    .unwrap();
    fs::write(
        dir.join("enrichments.jsonl"),
        format!(r#"{{"sha256":"{a}","openalex_cited_by":42}}"#),
    )
    .unwrap();
    fs::write(
        dir.join("movements.jsonl"),
        format!(
            r#"{{"sha256":"{c}","from_path":"old/solo.pdf","to_path":"lib/test_corpus/solo.pdf","moved_at":"2026-06-01 00:00:00+00","reason":"tidy"}}"#
        ),
    )
    .unwrap();
}

fn make_embeddings_dir(tmp: &TempDir) -> std::path::PathBuf {
    let dir = tmp.path().join("embeddings");
    fs::create_dir(&dir).unwrap();
    make_corpus_db(
        &dir.join("test_corpus_20260611_161620.sqlite"),
        &[
            (1, "dup.pdf", "chunk disambiguable"),
            (2, "solo.pdf", "chunk solo"),
            (3, "missing.pdf", "chunk orphan"),
            (4, "harddup.pdf", "chunk unresolvable collision"),
        ],
        &[1, 2, 3, 4],
    );
    dir
}

fn build(tmp: &TempDir) -> (LibraryStore, std::path::PathBuf, std::path::PathBuf) {
    let export = tmp.path().join("export");
    fs::create_dir(&export).unwrap();
    write_export_fixture(&export);
    let embeddings = make_embeddings_dir(tmp);
    let store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    (store, export, embeddings)
}

#[test]
fn filename_collisions_quarantine_and_import_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let (mut store, export, embeddings) = build(&tmp);
    let stats = import_library(&mut store, &export, &embeddings, "2026-09-22T00:00:00Z").unwrap();
    assert_eq!(stats.papers_inserted, 5);
    assert_eq!(stats.corpora["test_corpus"].chunks_inserted, 4);
    assert_eq!(stats.collision_disambiguated, 1);
    assert_eq!(stats.quarantined_filename_collision, 1);
    assert_eq!(stats.quarantined_orphan, 1);

    // (a) the unresolvable collision must NOT be silently joined.
    let (sha, reason): (Option<String>, String) = store
        .raw()
        .query_row(
            "SELECT c.sha256, q.reason FROM chunk c
             JOIN chunk_quarantine q ON q.corpus = c.corpus AND q.chunk_id = c.chunk_id
             WHERE c.filename = 'harddup.pdf'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(sha, None, "collision must not silently join");
    assert_eq!(reason, "filename_collision");

    // The disambiguable collision resolves to the corpus-matching path.
    let resolved: Option<String> = store
        .raw()
        .query_row(
            "SELECT sha256 FROM chunk WHERE filename = 'dup.pdf'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(resolved, Some("a".repeat(64)));

    // Orphan chunk: imported with NULL sha256, quarantined as orphan.
    let orphan: (Option<String>, String) = store
        .raw()
        .query_row(
            "SELECT c.sha256, q.reason FROM chunk c
             JOIN chunk_quarantine q ON q.corpus = c.corpus AND q.chunk_id = c.chunk_id
             WHERE c.filename = 'missing.pdf'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(orphan, (None, "orphan".to_string()));

    // Solo chunk joined.
    let solo: String = store
        .raw()
        .query_row(
            "SELECT sha256 FROM chunk WHERE filename = 'solo.pdf'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(solo, "c".repeat(64));

    // Embeddings preserved (384 float32 = 1536 bytes).
    let blob_len: i64 = store
        .raw()
        .query_row(
            "SELECT length(embedding) FROM chunk WHERE chunk_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(blob_len, 1536);

    // Movement reconciled current path.
    let path: String = store
        .raw()
        .query_row(
            "SELECT path FROM paper WHERE sha256 = ?1",
            ["c".repeat(64)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(path, "lib/test_corpus/solo.pdf");

    // (b) second run: nothing new.
    let stats2 = import_library(&mut store, &export, &embeddings, "2026-09-22T01:00:00Z").unwrap();
    assert_eq!(stats2.papers_inserted, 0);
    assert_eq!(stats2.papers_existing, 5);
    assert_eq!(stats2.corpora["test_corpus"].chunks_inserted, 0);
    assert_eq!(stats2.corpora["test_corpus"].chunks_existing, 4);
    let total_chunks: i64 = store
        .raw()
        .query_row("SELECT COUNT(*) FROM chunk", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total_chunks, 4);
}
