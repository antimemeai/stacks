//! Shadow embeddings from the laptop-hosted pplx-embed sidecar
//! (pplx-embed-v2, contextual, 2048-dim native int8). Runs parallel to the
//! MiniLM 384-dim path in `embed.rs`; rows land in `chunk_embedding_pplx`
//! keyed by (corpus, chunk_id, model) where `model` is pinned to the HF
//! revision reported by /healthz. The feature is off unless STACKS_PPLX_URL
//! is set.

use std::time::Duration;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use stacks_core::library::LibraryStore;

use crate::ImportError;

pub const ENV_URL: &str = "STACKS_PPLX_URL";
pub const DEFAULT_MODEL: &str = "pplx-embed-v2-context-9b-preview";
pub const DIMS: usize = 2048;
/// Chunks per encode_documents call; contextual quality degrades slightly at
/// window seams, which is accepted for papers longer than this.
pub const WINDOW: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PplxHealth {
    pub model: String,
    #[serde(default)]
    pub revision: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackfillStats {
    pub papers_done: u64,
    pub chunks_embedded: u64,
    pub skipped_existing: u64,
    pub errors: u64,
}

/// Live client for the pplx-embed sidecar. Construct via [`HttpPplxClient::from_env`]
/// so the feature simply does not exist when the URL is unset.
pub struct HttpPplxClient {
    agent: ureq::Agent,
    base: String,
}

impl HttpPplxClient {
    pub fn new(base: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .user_agent("stacks-import/0.1 (pplx-embed backfill)")
            .build();
        Self {
            agent,
            base: base.trim_end_matches('/').to_string(),
        }
    }

    /// None when STACKS_PPLX_URL is unset or empty — feature off.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var(ENV_URL).ok()?;
        let url = url.trim();
        if url.is_empty() {
            None
        } else {
            Some(Self::new(url))
        }
    }

    pub fn healthz(&self) -> Result<PplxHealth, ImportError> {
        let health: PplxHealth = self
            .agent
            .get(&format!("{}/healthz", self.base))
            .call()
            .map_err(http_err)?
            .into_json()?;
        Ok(health)
    }

    /// `documents` mirrors the sidecar's nested shape: one inner Vec of chunk
    /// strings per document; the response mirrors the same structure.
    pub fn encode_documents(
        &self,
        documents: &[Vec<String>],
    ) -> Result<Vec<Vec<Vec<i8>>>, ImportError> {
        let resp = self.post_json(
            "/encode_documents",
            serde_json::json!({"documents": documents}),
        )?;
        let embeddings: Vec<Vec<Vec<i8>>> = serde_json::from_value(
            resp.get("embeddings")
                .cloned()
                .ok_or_else(|| bad_shape("missing embeddings key"))?,
        )
        .map_err(|e| bad_shape(&format!("embeddings not i8 arrays: {e}")))?;
        validate_embeddings(&embeddings, documents.len(), |i| documents[i].len())?;
        Ok(embeddings)
    }

    pub fn encode_queries(&self, queries: &[String]) -> Result<Vec<Vec<i8>>, ImportError> {
        let resp = self.post_json("/encode_queries", serde_json::json!({"queries": queries}))?;
        let embeddings: Vec<Vec<i8>> = serde_json::from_value(
            resp.get("embeddings")
                .cloned()
                .ok_or_else(|| bad_shape("missing embeddings key"))?,
        )
        .map_err(|e| bad_shape(&format!("embeddings not i8 arrays: {e}")))?;
        if embeddings.len() != queries.len() {
            return Err(bad_shape(&format!(
                "expected {} query embeddings, got {}",
                queries.len(),
                embeddings.len()
            )));
        }
        for (i, v) in embeddings.iter().enumerate() {
            if v.len() != DIMS {
                return Err(bad_shape(&format!(
                    "query embedding {i}: dims {} != {DIMS}",
                    v.len()
                )));
            }
        }
        Ok(embeddings)
    }

    fn post_json(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value, ImportError> {
        let url = format!("{}{}", self.base, path);
        let mut last: Option<ImportError> = None;
        for attempt in 0..2 {
            match self.agent.post(&url).send_json(body.clone()) {
                Ok(resp) => return Ok(resp.into_json()?),
                Err(e @ ureq::Error::Transport(_)) if attempt == 0 => {
                    // One retry on transient network failure (laptop Wi-Fi
                    // blips); 9B batches are too slow to retry liberally.
                    last = Some(http_err(e));
                }
                Err(e) => return Err(http_err(e)),
            }
        }
        Err(last.unwrap())
    }
}

fn http_err(e: ureq::Error) -> ImportError {
    ImportError::Http(format!("pplx-embed sidecar: {e}"))
}

fn bad_shape(msg: &str) -> ImportError {
    ImportError::Http(format!("pplx-embed bad response: {msg}"))
}

/// Response structure must mirror the request counts exactly, and every
/// vector must be exactly DIMS long (the table CHECK would reject short
/// blobs anyway; catch it here with a clearer error).
fn validate_embeddings<T>(
    embeddings: &[Vec<Vec<T>>],
    n_docs: usize,
    doc_len: impl Fn(usize) -> usize,
) -> Result<(), ImportError> {
    if embeddings.len() != n_docs {
        return Err(bad_shape(&format!(
            "expected {n_docs} documents, got {}",
            embeddings.len()
        )));
    }
    for (d, doc) in embeddings.iter().enumerate() {
        if doc.len() != doc_len(d) {
            return Err(bad_shape(&format!(
                "document {d}: expected {} embeddings, got {}",
                doc_len(d),
                doc.len()
            )));
        }
        for (i, v) in doc.iter().enumerate() {
            if v.len() != DIMS {
                return Err(bad_shape(&format!(
                    "document {d} embedding {i}: dims {} != {DIMS}",
                    v.len()
                )));
            }
        }
    }
    Ok(())
}

/// Split a paper's chunks into ≤WINDOW-sized consecutive windows; each
/// window becomes one encode_documents call holding a single document.
pub fn chunk_windows<T>(items: &[T]) -> Vec<&[T]> {
    items.chunks(WINDOW).collect()
}

/// Papers (corpus, sha256 groups) that have chunks but no chunk_embedding_pplx
/// row for `model` on at least one chunk. Resumable: embedded chunks drop out
/// of the NOT EXISTS, so a rerun only revisits unfinished papers.
pub fn papers_needing_pplx(
    conn: &rusqlite::Connection,
    model: &str,
    limit: Option<u64>,
) -> Result<Vec<(String, String)>, ImportError> {
    let sql = "SELECT corpus, sha256 FROM chunk c
               WHERE c.sha256 IS NOT NULL
                 AND EXISTS (SELECT 1 FROM chunk c2
                             WHERE c2.corpus = c.corpus AND c2.sha256 = c.sha256
                               AND NOT EXISTS (SELECT 1 FROM chunk_embedding_pplx p
                                               WHERE p.corpus = c2.corpus
                                                 AND p.chunk_id = c2.chunk_id
                                                 AND p.model = ?1))
               GROUP BY c.corpus, c.sha256
               ORDER BY c.corpus, c.sha256";
    let rows: Vec<(String, String)> = match limit {
        Some(n) => conn
            .prepare(&format!("{sql} LIMIT ?2"))?
            .query_map(params![model, n as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?,
        None => conn
            .prepare(sql)?
            .query_map(params![model], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?,
    };
    Ok(rows)
}

/// Embed one paper's chunks (already filtered to those missing pplx rows)
/// and insert the vectors in a single transaction. Returns (embedded,
/// skipped_existing).
pub fn embed_and_insert(
    store: &mut LibraryStore,
    client: &HttpPplxClient,
    model: &str,
    corpus: &str,
    chunks: &[(i64, String)],
) -> Result<(u64, u64), ImportError> {
    // Encode every window first (network outside the write lock), then land
    // all of the paper's vectors in one per-paper transaction.
    let mut windows: Vec<Vec<Vec<i8>>> = Vec::new();
    for window in chunk_windows(chunks) {
        let texts: Vec<String> = window.iter().map(|(_, t)| t.clone()).collect();
        let docs = vec![texts];
        let vectors = client.encode_documents(&docs)?.remove(0);
        windows.push(vectors);
    }
    let mut embedded = 0u64;
    let mut skipped = 0u64;
    store.with_transaction(|conn| {
        for (window, vectors) in chunk_windows(chunks).iter().zip(windows.iter()) {
            for ((chunk_id, _), vector) in window.iter().zip(vectors.iter()) {
                let blob: Vec<u8> = vector.iter().map(|&v| v as u8).collect();
                if stacks_core::library::insert_chunk_embedding_pplx(
                    conn, corpus, *chunk_id, model, &blob,
                )? {
                    embedded += 1;
                } else {
                    skipped += 1;
                }
            }
        }
        Ok::<_, ImportError>(())
    })?;
    Ok((embedded, skipped))
}

/// Missing-pplx chunks of one paper, ordered by chunk_id.
pub fn chunks_missing_pplx(
    conn: &rusqlite::Connection,
    model: &str,
    corpus: &str,
    sha256: &str,
) -> Result<Vec<(i64, String)>, ImportError> {
    let rows = conn
        .prepare(
            "SELECT c.chunk_id, c.text FROM chunk c
             WHERE c.corpus = ?1 AND c.sha256 = ?2
               AND NOT EXISTS (SELECT 1 FROM chunk_embedding_pplx p
                               WHERE p.corpus = c.corpus
                                 AND p.chunk_id = c.chunk_id
                                 AND p.model = ?3)
             ORDER BY c.chunk_id",
        )?
        .query_map(params![corpus, sha256, model], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Stamp `model` with the sidecar's HF revision when healthz reports one, so
/// preview embeddings can never mix with a later release.
pub fn stamped_model(client: &HttpPplxClient, base: &str) -> String {
    match client.healthz() {
        Ok(PplxHealth {
            revision: Some(rev),
            ..
        }) if !rev.is_empty() => format!("{base}@{rev}"),
        _ => base.to_string(),
    }
}

/// Backfill pass: up to `limit` papers, per-paper transaction, per-paper
/// errors logged and counted without aborting the run.
pub fn backfill(
    store: &mut LibraryStore,
    client: &HttpPplxClient,
    model: &str,
    limit: Option<u64>,
) -> Result<BackfillStats, ImportError> {
    let mut stats = BackfillStats::default();
    let papers = papers_needing_pplx(store.raw(), model, limit)?;
    for (corpus, sha256) in papers {
        match (|| {
            let chunks = chunks_missing_pplx(store.raw(), model, &corpus, &sha256)?;
            embed_and_insert(store, client, model, &corpus, &chunks)
        })() {
            Ok((embedded, skipped)) => {
                stats.papers_done += 1;
                stats.chunks_embedded += embedded;
                stats.skipped_existing += skipped;
            }
            Err(e) => {
                eprintln!("pplx-backfill: paper {corpus}/{sha256}: {e}");
                stats.errors += 1;
            }
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_wrong_dims_and_counts() {
        let ok: Vec<Vec<Vec<i8>>> = vec![vec![vec![0i8; DIMS]; 3]];
        assert!(validate_embeddings(&ok, 1, |_| 3).is_ok());

        let wrong_dims: Vec<Vec<Vec<i8>>> = vec![vec![vec![0i8; 7]; 3]];
        assert!(validate_embeddings(&wrong_dims, 1, |_| 3).is_err());

        let wrong_doc_count: Vec<Vec<Vec<i8>>> = vec![vec![vec![0i8; DIMS]; 3]; 2];
        assert!(validate_embeddings(&wrong_doc_count, 1, |_| 3).is_err());

        let wrong_inner: Vec<Vec<Vec<i8>>> = vec![vec![vec![0i8; DIMS]; 2]];
        assert!(validate_embeddings(&wrong_inner, 1, |_| 3).is_err());
    }

    #[test]
    fn windows_split_consecutive() {
        let items: Vec<i64> = (0..300).collect();
        let wins = chunk_windows(&items);
        assert_eq!(wins.len(), 3);
        assert_eq!(wins[0].len(), 128);
        assert_eq!(wins[1].len(), 128);
        assert_eq!(wins[2].len(), 44);
        assert_eq!(wins[0][0], 0);
        assert_eq!(wins[1][0], 128);
        assert_eq!(wins[2][0], 256);
        // Consecutive: concatenating the windows reproduces the input.
        let flat: Vec<i64> = wins.iter().flat_map(|w| w.iter().copied()).collect();
        assert_eq!(flat, items);

        let small: Vec<i64> = (0..3).collect();
        assert_eq!(chunk_windows(&small).len(), 1);
        let empty: Vec<i64> = vec![];
        assert!(chunk_windows(&empty).is_empty());
    }

    #[test]
    fn papers_needing_pplx_is_resumable() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = LibraryStore::open(tmp.path().join("lib.db")).unwrap();
        let conn = store.raw();
        // Two papers in corpus "papers": A has 2 chunks, B has 1.
        for (sha, ids) in [("a".repeat(64), vec![1, 2]), ("b".repeat(64), vec![3])] {
            conn.execute(
                "INSERT INTO paper (sha256, filename, title) VALUES (?1, 'f.pdf', 'fixture')",
                params![sha],
            )
            .unwrap();
            for id in ids {
                conn.execute(
                    "INSERT INTO chunk (corpus, chunk_id, sha256, filename, text)
                     VALUES ('papers', ?1, ?2, 'f.pdf', 'body text here')",
                    params![id, sha],
                )
                .unwrap();
            }
        }
        // A chunk without a paper join must not create a phantom paper.
        conn.execute(
            "INSERT INTO chunk (corpus, chunk_id, sha256, filename, text)
             VALUES ('papers', 99, NULL, 'orphan.pdf', 'text')",
            [],
        )
        .unwrap();

        let model = "m@rev1";
        let papers = papers_needing_pplx(conn, model, None).unwrap();
        assert_eq!(papers, vec![("papers".into(), "a".repeat(64)), ("papers".into(), "b".repeat(64))]);

        // Fully embed paper A: it drops out of the selection.
        for id in [1, 2] {
            stacks_core::library::insert_chunk_embedding_pplx(
                conn,
                "papers",
                id,
                model,
                &[0u8; DIMS],
            )
            .unwrap();
        }
        let papers = papers_needing_pplx(conn, model, None).unwrap();
        assert_eq!(papers, vec![("papers".into(), "b".repeat(64))]);

        // A different model string still sees both papers.
        let papers = papers_needing_pplx(conn, "other-model", None).unwrap();
        assert_eq!(papers.len(), 2);

        // Limit caps the selection.
        let papers = papers_needing_pplx(conn, "other-model", Some(1)).unwrap();
        assert_eq!(papers.len(), 1);

        // chunks_missing_pplx on a partially embedded paper returns only the hole.
        stacks_core::library::insert_chunk_embedding_pplx(conn, "papers", 1, "other-model", &[0u8; DIMS])
            .unwrap();
        let missing = chunks_missing_pplx(conn, "other-model", "papers", &"a".repeat(64)).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].0, 2);
    }
}
