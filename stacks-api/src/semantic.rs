//! Dense semantic search over library chunks: query embedding (fastembed,
//! all-MiniLM-L6-v2 ONNX — parity-gated against NL's sentence-transformers
//! pipeline, see docs/embedding-parity-report.md) plus exact brute-force
//! cosine over lazily cached per-corpus vector slabs, and RRF(k=60) hybrid
//! fusion with FTS5 BM25 (algorithm per NL scripts/wave_corpus/retriever.py).

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;

use stacks_core::library::LibraryStore;
use stacks_core::StoreError;

pub const EMBED_DIM: usize = 384;
/// Lazy per-corpus cache cap (~2 GB of f32 vectors).
pub const CACHE_CAP_BYTES: usize = 2 << 30;

/// Query embedder abstraction so fixtures don't need the ONNX model.
pub trait Embedder: Send {
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, String>;
}

/// fastembed-backed embedder over the cached all-MiniLM-L6-v2 ONNX model.
pub struct FastEmbedder {
    inner: Mutex<fastembed::TextEmbedding>,
}

impl FastEmbedder {
    /// `cache_dir` is the fastembed/HF-style cache root containing
    /// `models--Qdrant--all-MiniLM-L6-v2-onnx`.
    pub fn new(cache_dir: &Path) -> Result<Self, String> {
        let opts = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(cache_dir.to_path_buf());
        let model = fastembed::TextEmbedding::try_new(opts).map_err(|e| e.to_string())?;
        Ok(Self {
            inner: Mutex::new(model),
        })
    }
}

impl Embedder for FastEmbedder {
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.inner.lock().map_err(|e| e.to_string())?;
        let mut out = model
            .embed(vec![text.to_string()], None)
            .map_err(|e| e.to_string())?;
        out.pop().ok_or_else(|| "empty embedding".to_string())
    }
}

/// A loaded corpus: parallel rowid + vector arrays.
struct CorpusVectors {
    rowids: Vec<i64>,
    /// row-major matrix, rowids.len() * EMBED_DIM floats
    matrix: Vec<f32>,
}

/// Lazy per-corpus vector cache with a byte-capped LRU eviction order.
pub struct VectorCache {
    corpora: HashMap<String, CorpusVectors>,
    lru: VecDeque<String>,
    bytes: usize,
    cap: usize,
}

impl Default for VectorCache {
    fn default() -> Self {
        Self::new(CACHE_CAP_BYTES)
    }
}

impl VectorCache {
    pub fn new(cap_bytes: usize) -> Self {
        VectorCache {
            corpora: HashMap::new(),
            lru: VecDeque::new(),
            bytes: 0,
            cap: cap_bytes,
        }
    }

    fn get_or_load(
        &mut self,
        store: &LibraryStore,
        corpus: &str,
    ) -> Result<&CorpusVectors, StoreError> {
        if !self.corpora.contains_key(corpus) {
            let mut stmt = store.raw().prepare(
                "SELECT rowid, embedding FROM chunk
                 WHERE corpus = ?1 AND embedding IS NOT NULL ORDER BY rowid",
            )?;
            let mut rowids = Vec::new();
            let mut matrix = Vec::new();
            let mut rows = stmt.query([corpus])?;
            while let Some(row) = rows.next()? {
                rowids.push(row.get::<_, i64>(0)?);
                let blob = row.get::<_, Vec<u8>>(1)?;
                matrix.extend_from_slice(bytearray_to_f32(&blob).as_slice());
            }
            let v = CorpusVectors { rowids, matrix };
            self.bytes += v.matrix.len() * 4;
            self.corpora.insert(corpus.to_string(), v);
            self.lru.push_back(corpus.to_string());
            while self.bytes > self.cap {
                let Some(evict) = self.lru.pop_front() else {
                    break;
                };
                if evict == corpus {
                    self.lru.push_back(evict);
                    break;
                }
                if let Some(v) = self.corpora.remove(&evict) {
                    self.bytes -= v.matrix.len() * 4;
                }
            }
        }
        Ok(self.corpora.get(corpus).expect("just loaded"))
    }

    pub fn cached_bytes(&self) -> usize {
        self.bytes
    }
}

/// Decode a little-endian f32 blob (no unsafe: chunk-wise conversion).
pub fn bytearray_to_f32(blob: &[u8]) -> Vec<f32> {
    blob.as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

/// Exact top-k by dot product (vectors are unit-normed, so dot == cosine).
pub fn brute_force_topk(
    rowids: &[i64],
    matrix: &[f32],
    query: &[f32],
    k: usize,
) -> Vec<(i64, f32)> {
    let dim = query.len();
    let mut best: Vec<(i64, f32)> = Vec::with_capacity(k + 1);
    for (i, &rowid) in rowids.iter().enumerate() {
        let row = &matrix[i * dim..(i + 1) * dim];
        let score: f32 = row.iter().zip(query).map(|(a, b)| a * b).sum();
        let pos = best.partition_point(|&(_, s)| s > score);
        if pos < k {
            best.insert(pos, (rowid, score));
            best.truncate(k);
        }
    }
    best
}

/// One fused/dense hit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticHit {
    pub rowid: i64,
    pub score: f64,
}

/// Reciprocal Rank Fusion, k=60 (NL `scripts/wave_corpus/retriever.py`
/// `_reciprocal_rank_fusion`): score = Σ over lists of 1/(60 + rank),
/// ranks 1-based, output sorted descending.
pub fn rrf_fuse(
    dense: &[(i64, f32)],
    sparse: &[(i64, f64)],
    k: usize,
    limit: usize,
) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for (rank, (rid, _)) in dense.iter().enumerate() {
        *scores.entry(*rid).or_default() += 1.0 / (k as f64 + rank as f64 + 1.0);
    }
    for (rank, (rid, _)) in sparse.iter().enumerate() {
        *scores.entry(*rid).or_default() += 1.0 / (k as f64 + rank as f64 + 1.0);
    }
    let mut fused: Vec<(i64, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    fused.truncate(limit);
    fused
}

/// Dense top-k over one or all corpora (merged by score when corpus=None).
pub fn dense_search(
    store: &LibraryStore,
    cache: &mut VectorCache,
    corpus: Option<&str>,
    query: &[f32],
    k: usize,
) -> Result<Vec<(i64, f32)>, StoreError> {
    let corpora: Vec<String> = match corpus {
        Some(c) => vec![c.to_string()],
        None => {
            let mut stmt = store
                .raw()
                .prepare("SELECT DISTINCT corpus FROM chunk ORDER BY corpus")?;
            let v: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<_, _>>()?;
            drop(stmt);
            v
        }
    };
    let mut all: Vec<(i64, f32)> = Vec::new();
    for c in &corpora {
        let v = cache.get_or_load(store, c)?;
        all.extend(brute_force_topk(&v.rowids, &v.matrix, query, k));
    }
    all.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    all.truncate(k);
    Ok(all)
}
