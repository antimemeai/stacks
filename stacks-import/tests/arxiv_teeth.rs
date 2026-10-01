use std::fs;
use std::path::Path;

use rusqlite::params;
use stacks_core::library::{insert_paper, LibraryPaper, LibraryStore};
use stacks_import::arxiv_import::{
    arxiv_load, embed_backfill, ArxivLoadOpts, SOURCE_COLLECTION,
};
use stacks_import::embed::{BatchEmbedder, EMBED_DIM};
use tempfile::TempDir;

fn words(n: usize) -> String {
    (0..n).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")
}

/// Deterministic offline embedder (same shape as the real MiniLM vectors).
struct StubEmbedder;

impl BatchEmbedder for StubEmbedder {
    fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        Ok(texts.iter().map(|_| vec![0.25f32; EMBED_DIM]).collect())
    }
}

/// Fixture shards:
/// cs.AI.jsonl: latex multi-chunk paper, abstract-only paper, NULL-title paper,
///   ghost-matching paper (2301.00005), giant latex paper (> 5 * CHUNK_SIZE words).
/// cs.LG.jsonl: duplicate of the latex paper (same arxiv_id + latex → same sha).
fn write_fixture(dir: &Path) {
    let latex_normal = format!(
        "\\section{{Intro}} {} zanzibar \\section{{Body}} {}",
        words(300),
        words(300)
    );
    let latex_giant = words(2500); // > 5 * CHUNK_SIZE (400)
    let cs_ai = [
        serde_json::json!({
            "arxiv_id": "2301.00001", "primary_category": "cs.AI",
            "categories": "cs.AI", "title": "A latex paper about zanzibar",
            "abstract": "abstract text", "date": "2023-01-01",
            "source_kind": "latex", "has_latex": true, "latex": latex_normal,
        }),
        serde_json::json!({
            "arxiv_id": "2301.00002", "primary_category": "cs.AI",
            "categories": "cs.AI", "title": "Abstract only paper",
            "abstract": format!("abstract only {}", words(40)),
            "date": "2022-06-15", "source_kind": "abstract", "has_latex": false,
            "latex": null,
        }),
        serde_json::json!({
            "arxiv_id": "2301.00003", "primary_category": "cs.AI",
            "categories": "cs.AI", "title": null,
            "abstract": format!("no title here {}", words(30)),
            "date": "2021-03-04", "source_kind": "abstract", "has_latex": false,
            "latex": null,
        }),
        serde_json::json!({
            "arxiv_id": "2301.00005", "primary_category": "cs.AI",
            "categories": "cs.AI", "title": "The ghost paper for real",
            "abstract": format!("ghost real {}", words(30)),
            "date": "2023-01-05", "source_kind": "abstract", "has_latex": false,
            "latex": null,
        }),
        serde_json::json!({
            "arxiv_id": "2301.00006", "primary_category": "cs.AI",
            "categories": "cs.AI", "title": "Giant latex paper",
            "abstract": "giant", "date": "2024-11-20",
            "source_kind": "latex", "has_latex": true, "latex": latex_giant,
        }),
    ];
    let cs_lg = [serde_json::json!({
        // Same arxiv_id + same latex as the cs.AI latex paper → same sha256,
        // must be counted as existing and produce no new chunks.
        "arxiv_id": "2301.00001", "primary_category": "cs.LG",
        "categories": "cs.AI cs.LG", "title": "A latex paper about zanzibar",
        "abstract": "abstract text", "date": "2023-01-01",
        "source_kind": "latex", "has_latex": true, "latex": latex_normal,
    })];
    fs::write(
        dir.join("cs.AI.jsonl"),
        cs_ai.iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    fs::write(
        dir.join("cs.LG.jsonl"),
        cs_lg.iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
}

/// Ghost fixtures: three intake stubs, all superseded by arXiv 2301.00005.
/// - f64: on_disk=0 (flag short-circuits, no stat call) → marked
/// - e64: on_disk=1 but payload absent under the temp nl-root → marked
/// - d64: on_disk=1 and payload present under the temp nl-root → NOT marked
fn insert_ghosts(store: &LibraryStore, nl_root: &Path) {
    let ghost = |sha: &str, filename: &str, path: &str, on_disk: bool| {
        let p: LibraryPaper = serde_json::from_value(serde_json::json!({
            "sha256": sha,
            "filename": filename,
            "path": path,
            "on_disk": on_disk,
            "title": "stub title from intake",
        }))
        .unwrap();
        insert_paper(store.raw(), &p).unwrap();
    };
    ghost(
        &"f".repeat(64),
        "2301.00005.pdf",
        "intake/ab/2301.00005.pdf",
        false,
    );
    ghost(
        &"e".repeat(64),
        "2301.00005v2.pdf",
        "intake/cd/2301.00005v2.pdf",
        true,
    );
    ghost(
        &"d".repeat(64),
        "2301.00005.pdf",
        "intake/exists/2301.00005.pdf",
        true,
    );
    let present = nl_root.join("intake/exists");
    fs::create_dir_all(&present).unwrap();
    fs::write(present.join("2301.00005.pdf"), b"stub payload").unwrap();
}

fn counts(store: &LibraryStore, sql: &str) -> i64 {
    store.raw().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn arxiv_load_end_to_end_idempotent_and_ghosts() {
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("jsonl");
    fs::create_dir_all(&jsonl).unwrap();
    write_fixture(&jsonl);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    let nl_root = tmp.path().join("nl");
    insert_ghosts(&store, &nl_root);
    let opts = || ArxivLoadOpts {
        nl_root: nl_root.clone(),
        ..Default::default()
    };

    let stats = arxiv_load(&mut store, &jsonl, &opts()).unwrap();
    assert_eq!(stats.papers_inserted, 5, "{stats:?}");
    assert_eq!(stats.papers_existing, 1, "cs.LG duplicate of 2301.00001");
    assert_eq!(stats.rows_rejected_no_id, 0);
    assert!(stats.chunks_inserted >= 10, "{stats:?}");
    assert_eq!(stats.fts_rows, stats.chunks_inserted);
    assert_eq!(stats.ghosts_marked, 2, "{stats:?}");

    // corpus = primary_category, unsuffixed/unsslugified.
    let corpora: Vec<String> = {
        let mut stmt = store
            .raw()
            .prepare("SELECT DISTINCT corpus FROM chunk ORDER BY corpus")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(corpora, vec!["cs.AI".to_string()]);

    // Paper fields: arxiv_id, year from date, subfield, access, collection,
    // path NULL, on_disk 0.
    let (arxiv_id, year, subfield, access, collection, path, on_disk): (
        String,
        i64,
        String,
        String,
        String,
        Option<String>,
        i64,
    ) = store
        .raw()
        .query_row(
            "SELECT arxiv_id, year, subfield, access, source_collection, path, on_disk
             FROM paper WHERE arxiv_id = '2301.00001'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(arxiv_id, "2301.00001");
    assert_eq!(year, 2023);
    assert_eq!(subfield, "cs.AI");
    assert_eq!(access, "open");
    assert_eq!(collection, SOURCE_COLLECTION);
    assert!(path.is_none());
    assert_eq!(on_disk, 0);

    // Giant latex produced multiple chunks.
    let giant_chunks: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM chunk WHERE filename = '2301.00006.tex'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(giant_chunks >= 6, "giant chunks: {giant_chunks}");

    // FTS finds the distinctive latex term.
    let hits: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM chunk_fts WHERE chunk_fts MATCH 'zanzibar'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(hits >= 1);

    // Ghost rows tagged, not deleted; the present-payload stub untouched.
    for (sha, expect_tagged) in [("f".repeat(64), true), ("e".repeat(64), true), ("d".repeat(64), false)] {
        let tags: Option<String> = store
            .raw()
            .query_row(
                "SELECT tags FROM paper WHERE sha256 = ?1",
                params![sha],
                |r| r.get(0),
            )
            .unwrap();
        let tagged = tags.as_deref().unwrap_or("").contains("superseded-by:arxiv");
        assert_eq!(tagged, expect_tagged, "{sha}: {tags:?}");
    }
    let on_disk: i64 = store
        .raw()
        .query_row(
            "SELECT on_disk FROM paper WHERE sha256 = ?1",
            params!["f".repeat(64)],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(on_disk, 0);

    // Rerun is a no-op: INSERT OR IGNORE on papers, per-category progress
    // consumed, ghosts already tagged.
    let papers0 = counts(&store, "SELECT COUNT(*) FROM paper");
    let chunks0 = counts(&store, "SELECT COUNT(*) FROM chunk");
    let stats = arxiv_load(&mut store, &jsonl, &opts()).unwrap();
    assert_eq!(stats.papers_inserted, 0, "{stats:?}");
    assert_eq!(stats.chunks_inserted, 0);
    assert_eq!(stats.ghosts_marked, 0);
    assert_eq!(counts(&store, "SELECT COUNT(*) FROM paper"), papers0);
    assert_eq!(counts(&store, "SELECT COUNT(*) FROM chunk"), chunks0);
}

#[test]
fn arxiv_load_limit_and_category_filter() {
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("jsonl");
    fs::create_dir_all(&jsonl).unwrap();
    write_fixture(&jsonl);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();

    let opts = ArxivLoadOpts {
        categories: Some(vec!["cs.LG".to_string()]),
        limit: None,
        ..Default::default()
    };
    let stats = arxiv_load(&mut store, &jsonl, &opts).unwrap();
    assert_eq!(stats.papers_inserted, 1);
    assert!(stats.per_category.contains_key("cs.LG"));
    assert!(!stats.per_category.contains_key("cs.AI"));
}

#[test]
fn embed_backfill_fills_and_rerun_is_noop() {
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("jsonl");
    fs::create_dir_all(&jsonl).unwrap();
    write_fixture(&jsonl);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    arxiv_load(&mut store, &jsonl, &ArxivLoadOpts::default()).unwrap();
    let n_chunks = counts(&store, "SELECT COUNT(*) FROM chunk");
    assert!(n_chunks > 0);
    assert_eq!(
        counts(&store, "SELECT COUNT(*) FROM chunk WHERE embedding IS NULL"),
        n_chunks
    );

    let mut stub = StubEmbedder;
    let stats = embed_backfill(&mut store, &mut stub, None, None).unwrap();
    assert_eq!(stats.embedded, n_chunks as u64);
    assert_eq!(stats.remaining_estimate, 0);
    let embedded: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM chunk WHERE LENGTH(embedding) = ?1",
            params![(EMBED_DIM * 4) as i64],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(embedded, n_chunks);

    let stats = embed_backfill(&mut store, &mut stub, None, None).unwrap();
    assert_eq!(stats.embedded, 0);
    assert_eq!(stats.remaining_estimate, 0);
}

/// Real MiniLM model from the fastembed cache, when present on this machine
/// (same pattern as embed.rs's cache-gated test).
#[test]
fn embed_backfill_real_model() {
    let cache = stacks_import::embed::cache_dir();
    if !cache.join("models--Qdrant--all-MiniLM-L6-v2-onnx").exists() {
        eprintln!("fastembed cache absent at {}; skipping", cache.display());
        return;
    }
    let tmp = TempDir::new().unwrap();
    let jsonl = tmp.path().join("jsonl");
    fs::create_dir_all(&jsonl).unwrap();
    write_fixture(&jsonl);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    arxiv_load(&mut store, &jsonl, &ArxivLoadOpts::default()).unwrap();

    let mut embedder = stacks_import::embed::FastBatchEmbedder::load(&cache).unwrap();
    let stats = embed_backfill(&mut store, &mut embedder, None, Some(3)).unwrap();
    assert_eq!(stats.embedded, 3);
    assert!(stats.remaining_estimate > 0);
    let norm_ok: bool = store
        .raw()
        .query_row(
            "SELECT LENGTH(embedding) = ?1 FROM chunk WHERE embedding IS NOT NULL LIMIT 1",
            params![(EMBED_DIM * 4) as i64],
            |r| r.get(0),
        )
        .unwrap();
    assert!(norm_ok);
}
