//! Retrieval eval for the pplx-embed shadow index: runs each query through
//! five legs (BM25, minilm dense, minilm hybrid RRF, pplx dense, pplx hybrid)
//! over the library DB and reports recall@k and MRR per leg. Relevance is
//! judged at paper level — a hit counts when the chunk's sha256 is in the
//! query's relevant set. pplx legs are skipped (with a note) when the sidecar
//! URL is unset or the shadow table has no rows for the resolved model.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::BufRead;
use std::path::Path;

use serde::{Deserialize, Serialize};
use stacks_api::semantic::{
    self, PplxVectorCache, VectorCache, dense_search, dense_search_pplx, rrf_fuse,
};
use stacks_core::library::LibraryStore;

use crate::embed::BatchEmbedder;
use crate::pplx_embed::HttpPplxClient;
use crate::ImportError;

/// RRF fusion constant, matching stacks-api's semantic endpoint.
pub const RRF_K: usize = 60;

#[derive(Debug, Deserialize)]
pub struct EvalQuery {
    pub query: String,
    #[serde(default)]
    pub relevant_sha256: Vec<String>,
}

/// JSONL input: one `{"query": ..., "relevant_sha256": [...]}` per line.
pub fn load_queries(path: &Path) -> Result<Vec<EvalQuery>, ImportError> {
    let f = std::fs::File::open(path)?;
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(f).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let q: EvalQuery = serde_json::from_str(&line).map_err(|e| {
            ImportError::Http(format!("queries file {} line {}: {e}", path.display(), i + 1))
        })?;
        out.push(q);
    }
    Ok(out)
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct LegMetrics {
    pub recall_at_k: f64,
    pub mrr: f64,
    /// Mean hits returned per query for this leg.
    pub mean_candidates: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_dense_candidates: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_sparse_candidates: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub k: usize,
    pub queries: usize,
    /// BTreeMap: deterministic (alphabetical) leg ordering.
    pub legs: BTreeMap<String, LegMetrics>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Fraction of queries with at least one relevant paper anywhere in the
/// ranking (rankings are pre-truncated to k). Chunks without a sha256
/// (None) can never count as relevant.
pub fn recall_at_k(rankings: &[Vec<Option<String>>], relevant: &[HashSet<String>]) -> f64 {
    if rankings.is_empty() {
        return 0.0;
    }
    let hits = rankings
        .iter()
        .zip(relevant)
        .filter(|(ranked, rel)| ranked.iter().flatten().any(|sha| rel.contains(sha)))
        .count();
    hits as f64 / rankings.len() as f64
}

/// Mean reciprocal rank of the first relevant hit, by chunk rank (1-based).
pub fn mrr(rankings: &[Vec<Option<String>>], relevant: &[HashSet<String>]) -> f64 {
    if rankings.is_empty() {
        return 0.0;
    }
    let sum: f64 = rankings
        .iter()
        .zip(relevant)
        .map(|(ranked, rel)| {
            ranked
                .iter()
                .position(|sha| sha.as_ref().is_some_and(|s| rel.contains(s)))
                .map(|pos| 1.0 / (pos as f64 + 1.0))
                .unwrap_or(0.0)
        })
        .sum();
    sum / rankings.len() as f64
}

/// Build the query-side f32 vector from the sidecar's int8 output the same
/// way `PplxHttpEmbedder` does (stacks-api semantic.rs): reinterpret as
/// bytes, dequantize, L2-normalize so dot == cosine.
pub fn pplx_query_vector(v: &[i8]) -> Vec<f32> {
    let bytes: Vec<u8> = v.iter().map(|&b| b as u8).collect();
    semantic::dequantize_i8_normalized(&bytes)
}

/// Latest model stamped into the shadow table; None when the table is empty.
pub fn latest_pplx_model(store: &LibraryStore) -> Result<Option<String>, ImportError> {
    let model = store
        .raw()
        .prepare("SELECT model FROM chunk_embedding_pplx ORDER BY embedded_at DESC LIMIT 1")?
        .query_row([], |r| r.get::<_, String>(0))
        .ok();
    Ok(model)
}

/// Backends for the optional legs; None skips the leg group with a note.
pub struct EvalBackends<'a> {
    pub minilm: Option<&'a mut dyn BatchEmbedder>,
    pub pplx: Option<(&'a HttpPplxClient, String)>,
}

fn sha256_by_rowid(
    conn: &rusqlite::Connection,
    rowids: &[i64],
) -> Result<HashMap<i64, Option<String>>, ImportError> {
    let mut stmt = conn.prepare("SELECT rowid, sha256 FROM chunk WHERE rowid = ?1")?;
    let mut out = HashMap::with_capacity(rowids.len());
    for &rid in rowids {
        let sha: Option<String> = stmt.query_row([rid], |r| r.get(1)).ok().flatten();
        out.insert(rid, sha);
    }
    Ok(out)
}

struct LegAccum {
    rankings: Vec<Vec<Option<String>>>,
    candidates: Vec<usize>,
    dense_candidates: Vec<usize>,
    sparse_candidates: Vec<usize>,
}

impl LegAccum {
    fn new() -> Self {
        LegAccum {
            rankings: Vec::new(),
            candidates: Vec::new(),
            dense_candidates: Vec::new(),
            sparse_candidates: Vec::new(),
        }
    }

    fn push(&mut self, ranked_rowids: &[i64], shas: &HashMap<i64, Option<String>>) {
        self.candidates.push(ranked_rowids.len());
        self.rankings.push(
            ranked_rowids
                .iter()
                .map(|rid| shas.get(rid).cloned().flatten())
                .collect(),
        );
    }

    fn finish(self, relevant: &[HashSet<String>]) -> LegMetrics {
        let n = self.rankings.len().max(1) as f64;
        let mean = |v: &[usize]| v.iter().sum::<usize>() as f64 / n;
        let hybrid = !self.dense_candidates.is_empty();
        LegMetrics {
            recall_at_k: recall_at_k(&self.rankings, relevant),
            mrr: mrr(&self.rankings, relevant),
            mean_candidates: mean(&self.candidates),
            mean_dense_candidates: hybrid.then(|| mean(&self.dense_candidates)),
            mean_sparse_candidates: hybrid.then(|| mean(&self.sparse_candidates)),
        }
    }
}

/// Run every available leg over all queries and aggregate the metrics.
/// Query embeddings are batched: one minilm embed call and one pplx
/// encode_queries call for the whole query set.
pub fn run_eval(
    store: &LibraryStore,
    queries: &[EvalQuery],
    k: usize,
    backends: &mut EvalBackends,
) -> Result<EvalReport, ImportError> {
    let conn = store.raw();
    let relevant: Vec<HashSet<String>> = queries
        .iter()
        .map(|q| q.relevant_sha256.iter().cloned().collect())
        .collect();
    let texts: Vec<String> = queries.iter().map(|q| q.query.clone()).collect();
    let mut notes = Vec::new();

    // Batch query embeddings, one call per backend.
    let minilm_vecs: Option<Vec<Vec<f32>>> = match backends.minilm.as_deref_mut() {
        Some(e) => Some(
            e.embed_batch(&texts)
                .map_err(|e| ImportError::Http(format!("minilm query embed: {e}")))?,
        ),
        None => {
            notes.push("minilm embedder unavailable; skipped minilm-dense and minilm-hybrid".into());
            None
        }
    };
    let pplx_vecs: Option<(String, Vec<Vec<f32>>)> = match &backends.pplx {
        Some((client, model)) => {
            let rows: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chunk_embedding_pplx WHERE model = ?1",
                rusqlite::params![model],
                |r| r.get(0),
            )?;
            if rows == 0 {
                notes.push(format!(
                    "chunk_embedding_pplx has no rows for model {model:?}; skipped pplx-dense and pplx-hybrid"
                ));
                None
            } else {
                let raw = client.encode_queries(&texts)?;
                Some((model.clone(), raw.iter().map(|v| pplx_query_vector(v)).collect()))
            }
        }
        None => {
            notes.push(format!(
                "{} is not set; skipped pplx-dense and pplx-hybrid",
                crate::pplx_embed::ENV_URL
            ));
            None
        }
    };

    let mut bm25_acc = LegAccum::new();
    let mut minilm_dense_acc = LegAccum::new();
    let mut minilm_hybrid_acc = LegAccum::new();
    let mut pplx_dense_acc = LegAccum::new();
    let mut pplx_hybrid_acc = LegAccum::new();
    let mut vcache = VectorCache::default();
    let mut pcache = PplxVectorCache::default();

    for (qi, _q) in queries.iter().enumerate() {
        let text = &texts[qi];
        let sparse = stacks_api::bm25_topk(conn, text, None, k)?;
        let sparse_ids: Vec<i64> = sparse.iter().map(|(r, _)| *r).collect();
        let sparse_shas = sha256_by_rowid(conn, &sparse_ids)?;
        bm25_acc.push(&sparse_ids, &sparse_shas);

        if let Some(vecs) = &minilm_vecs {
            let dense = dense_search(store, &mut vcache, None, &vecs[qi], k)?;
            let dense_ids: Vec<i64> = dense.iter().map(|(r, _)| *r).collect();
            let dense_shas = sha256_by_rowid(conn, &dense_ids)?;
            minilm_dense_acc.push(&dense_ids, &dense_shas);
            let fused = rrf_fuse(&dense, &sparse, RRF_K, k);
            let fused_ids: Vec<i64> = fused.iter().map(|(r, _)| *r).collect();
            let fused_shas = sha256_by_rowid(conn, &fused_ids)?;
            minilm_hybrid_acc.dense_candidates.push(dense.len());
            minilm_hybrid_acc.sparse_candidates.push(sparse.len());
            minilm_hybrid_acc.push(&fused_ids, &fused_shas);
        }

        if let Some((model, vecs)) = &pplx_vecs {
            let dense = dense_search_pplx(store, &mut pcache, model, None, &vecs[qi], k)?;
            let dense_ids: Vec<i64> = dense.iter().map(|(r, _)| *r).collect();
            let dense_shas = sha256_by_rowid(conn, &dense_ids)?;
            pplx_dense_acc.push(&dense_ids, &dense_shas);
            let fused = rrf_fuse(&dense, &sparse, RRF_K, k);
            let fused_ids: Vec<i64> = fused.iter().map(|(r, _)| *r).collect();
            let fused_shas = sha256_by_rowid(conn, &fused_ids)?;
            pplx_hybrid_acc.dense_candidates.push(dense.len());
            pplx_hybrid_acc.sparse_candidates.push(sparse.len());
            pplx_hybrid_acc.push(&fused_ids, &fused_shas);
        }
    }

    let mut legs = BTreeMap::new();
    legs.insert("bm25".to_string(), bm25_acc.finish(&relevant));
    if minilm_vecs.is_some() {
        legs.insert("minilm-dense".to_string(), minilm_dense_acc.finish(&relevant));
        legs.insert("minilm-hybrid".to_string(), minilm_hybrid_acc.finish(&relevant));
    }
    if pplx_vecs.is_some() {
        legs.insert("pplx-dense".to_string(), pplx_dense_acc.finish(&relevant));
        legs.insert("pplx-hybrid".to_string(), pplx_hybrid_acc.finish(&relevant));
    }
    Ok(EvalReport {
        k,
        queries: queries.len(),
        legs,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shas(v: &[&str]) -> Vec<Option<String>> {
        v.iter().map(|s| Some(s.to_string())).collect()
    }

    fn rel(v: &[&str]) -> HashSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn recall_counts_queries_with_any_relevant_hit() {
        // Q1 relevant at rank 2, Q2 no relevant hit, Q3 relevant at rank 1.
        let rankings = vec![
            shas(&["x", "a"]),
            shas(&["x", "y"]),
            shas(&["c"]),
        ];
        let relevant = vec![rel(&["a"]), rel(&["b"]), rel(&["c"])];
        let r = recall_at_k(&rankings, &relevant);
        assert!((r - 2.0 / 3.0).abs() < 1e-12);

        assert_eq!(recall_at_k(&[], &[]), 0.0);
    }

    #[test]
    fn mrr_uses_first_relevant_rank() {
        let rankings = vec![
            shas(&["x", "a"]),      // first relevant at rank 2 -> 1/2
            shas(&["b"]),           // rank 1 -> 1
            vec![None, None],       // no sha -> miss
            shas(&["x"]),           // relevant absent -> 0
        ];
        let relevant = vec![rel(&["a"]), rel(&["b"]), rel(&["z"]), rel(&["q"])];
        let m = mrr(&rankings, &relevant);
        assert!((m - (0.5 + 1.0) / 4.0).abs() < 1e-12);

        assert_eq!(mrr(&[], &[]), 0.0);
    }

    #[test]
    fn pplx_query_vector_matches_embedder_shape() {
        // Dequantize + L2-normalize, same as PplxHttpEmbedder.
        let v = pplx_query_vector(&[3i8, 4, 0]);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
        // Zero vector survives without NaN.
        let z = pplx_query_vector(&[0i8; 2048]);
        assert!(z.iter().all(|&x| x == 0.0));
    }
}
