//! Parity gate utility: embed strings with the Rust/fastembed pipeline, and
//! brute-force KNN over a source-format embeddings sqlite (for retrieval
//! parity against sqlite-vec MATCH).
//!
//! usage:
//!   parity_embed embed <strings.txt> <out.json> [cache_dir]
//!   parity_embed brute <source.sqlite> <query.json> <k>

use std::path::Path;

use stacks_api::semantic::{brute_force_topk, bytearray_to_f32, Embedder, FastEmbedder};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cache = args
        .get(4)
        .cloned()
        .unwrap_or_else(|| "/home/patrick/neurotic_library/.fastembed_cache".to_string());
    match args.get(1).map(String::as_str) {
        Some("embed") => {
            let strings: Vec<String> = std::fs::read_to_string(&args[2])
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            let mut embedder = FastEmbedder::new(Path::new(&cache)).unwrap();
            let mut out = Vec::new();
            for s in &strings {
                out.push(embedder.embed(s).unwrap());
            }
            std::fs::write(&args[3], serde_json::to_string(&out).unwrap()).unwrap();
            println!("embedded {}", strings.len());
        }
        Some("brute") => {
            let conn = rusqlite::Connection::open_with_flags(
                &args[2],
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let mut rowids = Vec::new();
            let mut matrix = Vec::new();
            let mut stmt = conn.prepare("SELECT id FROM chunks ORDER BY id").unwrap();
            let ids: Vec<i64> = stmt
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let slabs: Vec<Vec<u8>> = conn
                .prepare("SELECT vectors FROM vec_chunks_vector_chunks00 ORDER BY rowid")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            drop(stmt);
            for id in ids {
                let ordinal = (id - 1) as usize;
                let slab = &slabs[ordinal / 1024];
                let off = (ordinal % 1024) * 1536;
                rowids.push(id);
                matrix.extend_from_slice(&bytearray_to_f32(&slab[off..off + 1536]));
            }
            let queries: Vec<Vec<f32>> =
                serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
            let k: usize = args[4].parse().unwrap();
            let results: Vec<Vec<(i64, f32)>> = queries
                .iter()
                .map(|q| brute_force_topk(&rowids, &matrix, q, k))
                .collect();
            println!("{}", serde_json::to_string(&results).unwrap());
        }
        _ => {
            eprintln!("usage: parity_embed embed|brute ...");
            std::process::exit(1);
        }
    }
}
