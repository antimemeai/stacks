//! Batch embedding for newly stamped chunks. fastembed (all-MiniLM-L6-v2,
//! 384-dim) exactly as stacks-api/src/semantic.rs runs it, so query-side
//! cosine over these blobs is bit-compatible with the NL-migrated corpus
//! (little-endian f32, 1536 bytes per vector).

use std::path::{Path, PathBuf};

pub const EMBED_DIM: usize = 384;
/// Default model cache — the NL fastembed cache. TODO: move to
/// /srv/stacks/models once the data root owns it.
pub const DEFAULT_CACHE: &str = "/home/patrick/neurotic_library/.fastembed_cache";

pub fn cache_dir() -> PathBuf {
    std::env::var("STACKS_FASTEMBED_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CACHE))
}

/// Serialize an embedding the way the migrated rows store it (semantic.rs's
/// bytearray_to_f32 reads little-endian f32 quads).
pub fn f32_vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Batch embedder seam so tests don't need the ONNX model.
pub trait BatchEmbedder {
    fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

pub struct FastBatchEmbedder {
    inner: fastembed::TextEmbedding,
}

impl FastBatchEmbedder {
    /// Loads the model from `cache`; Err (model missing etc.) is expected
    /// and handled by the caller as the embed-failure path.
    pub fn load(cache: &Path) -> Result<Self, String> {
        let opts = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(cache.to_path_buf());
        let inner = fastembed::TextEmbedding::try_new(opts).map_err(|e| e.to_string())?;
        Ok(Self { inner })
    }
}

impl BatchEmbedder for FastBatchEmbedder {
    fn embed_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.inner
            .embed(texts.to_vec(), None)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_format_is_le_f32() {
        let v = vec![1.0f32, -2.5, 0.0];
        let blob = f32_vec_to_blob(&v);
        assert_eq!(blob.len(), 12);
        let back: Vec<f32> = blob
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        assert_eq!(back, v);
    }

    /// Real model, only when the cache is present (it is on this machine).
    #[test]
    fn fastembed_produces_384_dim() {
        let cache = cache_dir();
        if !cache.join("models--Qdrant--all-MiniLM-L6-v2-onnx").exists() {
            eprintln!("fastembed cache absent at {}; skipping", cache.display());
            return;
        }
        let mut e = FastBatchEmbedder::load(&cache).unwrap();
        let out = e
            .embed_batch(&["perovskite synthesis at high temperature".to_string()])
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), EMBED_DIM);
        let norm: f32 = out[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "expected normalized vector, norm={norm}");
    }
}
