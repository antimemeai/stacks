//! stacks-mcp: read-only MCP (Model Context Protocol) server over the stacks
//! research library. Mirrors the stacks-api query logic against the same
//! library DB, always opened read-only.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use stacks_api::semantic;
use stacks_core::library::LibraryStore;

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 100;
const MAX_K: u32 = 50;

// ---------- tool arguments ----------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchChunksBm25Args {
    /// FTS5 MATCH query over chunk text.
    pub query: String,
    /// Restrict to one corpus (e.g. an NL wave corpus name).
    pub corpus: Option<String>,
    /// Max hits (1..=100, default 50).
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchChunksSemanticArgs {
    /// Natural-language query.
    pub query: String,
    /// Restrict to one corpus.
    pub corpus: Option<String>,
    /// "dense" (exact cosine KNN) or "hybrid" (RRF k=60 fusion with BM25).
    /// Default "hybrid".
    pub mode: Option<String>,
    /// Embedding backend: "minilm" (default) or "pplx" (pplx-embed-v2 shadow
    /// index; requires the laptop sidecar, STACKS_PPLX_URL).
    pub backend: Option<String>,
    /// Result count (1..=50, default 20).
    pub k: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPaperArgs {
    /// Paper sha256 (hex, 64 chars).
    pub sha256: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetChunkArgs {
    /// Corpus name (as returned in search hits).
    pub corpus: String,
    /// Chunk id within the corpus (as returned in search hits).
    pub chunk_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPapersArgs {
    /// Substring match on title/authors/filename.
    pub query: Option<String>,
    /// Exact subfield filter.
    pub subfield: Option<String>,
    /// Inclusive minimum publication year.
    pub year_from: Option<i64>,
    /// Inclusive maximum publication year.
    pub year_to: Option<i64>,
    /// Max rows (1..=100, default 50).
    pub limit: Option<u32>,
    /// Keyset cursor from a previous response's meta.next_cursor.
    pub cursor: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDocumentArgs {
    /// Document sha256 (hex, 64 chars).
    pub id: String,
}

// ---------- query logic (mirrors stacks-api handlers) ----------

fn check_limit(limit: Option<u32>) -> Result<u32, McpError> {
    let n = limit.unwrap_or(DEFAULT_LIMIT);
    if n == 0 || n > MAX_LIMIT {
        return Err(McpError::invalid_params(
            format!("limit must be within 1..={MAX_LIMIT}, got {n}"),
            None,
        ));
    }
    Ok(n)
}

fn json_text(v: &serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string(v).map_err(|e| McpError::internal_error(e.to_string(), None))?,
    )]))
}

fn internal(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn bm25_search(
    lib: &LibraryStore,
    query: &str,
    corpus: Option<&str>,
    limit: u32,
) -> Result<serde_json::Value, McpError> {
    let conn = lib.raw();
    let mut sql = String::from(
        "SELECT c.rowid, c.corpus, c.chunk_id, c.sha256, c.filename, c.section,
                snippet(chunk_fts, 0, '<b>', '</b>', '…', 32) AS snip,
                bm25(chunk_fts) AS rank
         FROM chunk_fts JOIN chunk c ON c.rowid = chunk_fts.rowid
         WHERE chunk_fts MATCH :q",
    );
    if corpus.is_some() {
        sql.push_str(" AND c.corpus = :corpus");
    }
    sql.push_str(" ORDER BY rank, c.rowid LIMIT :limit");
    let mut stmt = conn.prepare(&sql).map_err(internal)?;
    let limit64 = limit as i64;
    let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
        vec![(":q", &query), (":limit", &limit64)];
    if let Some(c) = &corpus {
        bindings.push((":corpus", c));
    }
    let rows: Vec<serde_json::Value> = stmt
        .query_map(
            bindings
                .iter()
                .map(|(n, v)| (*n, *v))
                .collect::<Vec<_>>()
                .as_slice(),
            |row| {
                Ok(serde_json::json!({
                    "rowid": row.get::<_, i64>(0)?,
                    "corpus": row.get::<_, String>(1)?,
                    "chunk_id": row.get::<_, i64>(2)?,
                    "sha256": row.get::<_, Option<String>>(3)?,
                    "filename": row.get::<_, String>(4)?,
                    "section": row.get::<_, Option<String>>(5)?,
                    "snippet": row.get::<_, String>(6)?,
                    "rank": row.get::<_, f64>(7)?,
                }))
            },
        )
        .map_err(internal)?
        .collect::<Result<_, _>>()
        .map_err(internal)?;
    Ok(serde_json::json!({
        "data": rows,
        "meta": {"limit": limit, "ordering": "bm25 rank ASC, rowid ASC"},
    }))
}

/// BM25 top-k by rowid (sparse leg of hybrid fusion).
fn bm25_topk(
    conn: &rusqlite::Connection,
    query: &str,
    corpus: Option<&str>,
    k: usize,
) -> Result<Vec<(i64, f64)>, McpError> {
    let mut sql = String::from(
        "SELECT c.rowid, bm25(chunk_fts) FROM chunk_fts JOIN chunk c ON c.rowid = chunk_fts.rowid
         WHERE chunk_fts MATCH :q",
    );
    if corpus.is_some() {
        sql.push_str(" AND c.corpus = :corpus");
    }
    sql.push_str(" ORDER BY 2, 1 LIMIT :k");
    let mut stmt = conn.prepare(&sql).map_err(internal)?;
    let k64 = k as i64;
    let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> = vec![(":q", &query), (":k", &k64)];
    if let Some(c) = &corpus {
        bindings.push((":corpus", c));
    }
    let rows = stmt
        .query_map(
            bindings
                .iter()
                .map(|(n, v)| (*n, *v))
                .collect::<Vec<_>>()
                .as_slice(),
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
        )
        .map_err(internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(internal)?;
    Ok(rows)
}

fn hydrate_hits(
    conn: &rusqlite::Connection,
    hits: &[(i64, f64)],
) -> Result<Vec<serde_json::Value>, McpError> {
    let mut stmt = conn
        .prepare(
            "SELECT rowid, corpus, chunk_id, sha256, filename, section, substr(text, 1, 400)
             FROM chunk WHERE rowid = ?1",
        )
        .map_err(internal)?;
    let mut out = Vec::with_capacity(hits.len());
    for (rowid, score) in hits {
        let v = stmt
            .query_row([*rowid], |row| {
                Ok(serde_json::json!({
                    "rowid": rowid,
                    "corpus": row.get::<_, String>(1)?,
                    "chunk_id": row.get::<_, i64>(2)?,
                    "sha256": row.get::<_, Option<String>>(3)?,
                    "filename": row.get::<_, String>(4)?,
                    "section": row.get::<_, Option<String>>(5)?,
                    "text_preview": row.get::<_, String>(6)?,
                    "score": score,
                }))
            })
            .map_err(internal)?;
        out.push(v);
    }
    Ok(out)
}

fn chunk_detail(
    lib: &LibraryStore,
    corpus: &str,
    chunk_id: i64,
) -> Result<Option<serde_json::Value>, McpError> {
    let conn = lib.raw();
    let v = conn
        .query_row(
            "SELECT rowid, corpus, chunk_id, sha256, filename, title, section, text, word_count
             FROM chunk WHERE corpus = ?1 AND chunk_id = ?2",
            rusqlite::params![corpus, chunk_id],
            |row| {
                Ok(serde_json::json!({
                    "rowid": row.get::<_, i64>(0)?,
                    "corpus": row.get::<_, String>(1)?,
                    "chunk_id": row.get::<_, i64>(2)?,
                    "sha256": row.get::<_, Option<String>>(3)?,
                    "filename": row.get::<_, String>(4)?,
                    "title": row.get::<_, Option<String>>(5)?,
                    "section": row.get::<_, Option<String>>(6)?,
                    "text": row.get::<_, String>(7)?,
                    "word_count": row.get::<_, Option<i64>>(8)?,
                }))
            },
        )
        .ok();
    Ok(v)
}

fn paper_detail(lib: &LibraryStore, sha256: &str) -> Result<Option<serde_json::Value>, McpError> {
    let conn = lib.raw();
    let paper = conn
        .query_row(
            &format!(
                "SELECT {} FROM paper WHERE sha256 = ?1",
                stacks_api::PAPER_COLS
            ),
            [sha256],
            stacks_api::paper_from_row,
        )
        .optional()
        .map_err(internal)?;
    let Some(paper) = paper else {
        return Ok(None);
    };
    let enrichment: Option<serde_json::Value> = conn
        .query_row(
            "SELECT openalex_id, openalex_topics, openalex_concepts, openalex_cited_by,
                    s2_paper_id, s2_tldr, s2_fields_of_study, s2_influential_citation_count,
                    unpaywall_oa_status, unpaywall_oa_url, enriched_at
             FROM paper_enrichment WHERE sha256 = ?1",
            [sha256],
            |row| {
                Ok(serde_json::json!({
                    "openalex_id": row.get::<_, Option<String>>(0)?,
                    "openalex_topics": row.get::<_, Option<String>>(1)?,
                    "openalex_concepts": row.get::<_, Option<String>>(2)?,
                    "openalex_cited_by": row.get::<_, Option<i64>>(3)?,
                    "s2_paper_id": row.get::<_, Option<String>>(4)?,
                    "s2_tldr": row.get::<_, Option<String>>(5)?,
                    "s2_fields_of_study": row.get::<_, Option<String>>(6)?,
                    "s2_influential_citation_count": row.get::<_, Option<i64>>(7)?,
                    "unpaywall_oa_status": row.get::<_, Option<String>>(8)?,
                    "unpaywall_oa_url": row.get::<_, Option<String>>(9)?,
                    "enriched_at": row.get::<_, Option<String>>(10)?,
                }))
            },
        )
        .optional()
        .map_err(internal)?;
    let chunk_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunk WHERE sha256 = ?1",
            [sha256],
            |r| r.get(0),
        )
        .map_err(internal)?;
    Ok(Some(serde_json::json!({
        "paper": paper,
        "enrichment": enrichment,
        "chunk_count": chunk_count,
    })))
}

fn papers_list(
    lib: &LibraryStore,
    args: &ListPapersArgs,
    limit: u32,
) -> Result<serde_json::Value, McpError> {
    let conn = lib.raw();
    let cursor = args.cursor.unwrap_or(0);
    let query = args.query.as_ref().filter(|q| !q.trim().is_empty());
    let subfield = args.subfield.as_ref().filter(|s| !s.trim().is_empty());
    let mut sql = format!(
        "SELECT rowid, {} FROM paper WHERE rowid > :cursor",
        stacks_api::PAPER_COLS
    );
    if query.is_some() {
        sql.push_str(" AND (title LIKE :q OR authors LIKE :q OR filename LIKE :q)");
    }
    if subfield.is_some() {
        sql.push_str(" AND subfield = :subfield");
    }
    if args.year_from.is_some() {
        sql.push_str(" AND year >= :year_from");
    }
    if args.year_to.is_some() {
        sql.push_str(" AND year <= :year_to");
    }
    sql.push_str(" ORDER BY rowid LIMIT :limit");
    let mut stmt = conn.prepare(&sql).map_err(internal)?;
    let limit_plus_one = limit as i64 + 1;
    let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
        vec![(":cursor", &cursor), (":limit", &limit_plus_one)];
    let like;
    if let Some(q) = query {
        like = format!("%{q}%");
        bindings.push((":q", &like));
    }
    if let Some(s) = subfield {
        bindings.push((":subfield", s));
    }
    if let Some(y) = &args.year_from {
        bindings.push((":year_from", y));
    }
    if let Some(y) = &args.year_to {
        bindings.push((":year_to", y));
    }
    let rows: Vec<serde_json::Value> = stmt
        .query_map(
            bindings
                .iter()
                .map(|(n, v)| (*n, *v))
                .collect::<Vec<_>>()
                .as_slice(),
            |row| {
                let mut v = serde_json::to_value(stacks_api::paper_from_row_at(row)?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                v["rowid"] = serde_json::json!(row.get::<_, i64>(0)?);
                Ok(v)
            },
        )
        .map_err(internal)?
        .collect::<Result<_, _>>()
        .map_err(internal)?;
    let mut rows = rows;
    let next_cursor = if rows.len() as u32 > limit {
        rows.truncate(limit as usize);
        rows.last().and_then(|r| r["rowid"].as_i64())
    } else {
        None
    };
    let data: Vec<serde_json::Value> = rows;
    Ok(serde_json::json!({
        "data": data,
        "meta": {
            "limit": limit,
            "max_limit": MAX_LIMIT,
            "next_cursor": next_cursor,
            "ordering": "rowid ASC (keyset; pass meta.next_cursor as cursor)",
        },
    }))
}

fn status(lib: &LibraryStore) -> Result<serde_json::Value, McpError> {
    let conn = lib.raw();
    let count = |table: &str| -> Result<i64, McpError> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .map_err(internal)
    };
    let mut stmt = conn
        .prepare("SELECT status, COUNT(*) FROM inproc_queue GROUP BY status ORDER BY 2 DESC")
        .map_err(internal)?;
    let queue: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "status": r.get::<_, String>(0)?,
                "count": r.get::<_, i64>(1)?,
            }))
        })
        .map_err(internal)?
        .collect::<Result<_, _>>()
        .map_err(internal)?;
    let mut cstmt = conn
        .prepare("SELECT corpus, COUNT(*) FROM chunk GROUP BY corpus ORDER BY corpus")
        .map_err(internal)?;
    let corpora: Vec<serde_json::Value> = cstmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "corpus": r.get::<_, String>(0)?,
                "chunks": r.get::<_, i64>(1)?,
            }))
        })
        .map_err(internal)?
        .collect::<Result<_, _>>()
        .map_err(internal)?;
    Ok(serde_json::json!({
        "counts": {
            "papers": count("paper")?,
            "enrichments": count("paper_enrichment")?,
            "chunks": count("chunk")?,
            "quarantined_chunks": count("chunk_quarantine")?,
            "documents": count("document")?,
            "datasets": count("dataset")?,
            "inproc_queue": count("inproc_queue")?,
        },
        "inproc_queue_by_status": queue,
        "corpora": corpora,
    }))
}

fn document_detail(lib: &LibraryStore, id: &str) -> Result<Option<serde_json::Value>, McpError> {
    let conn = lib.raw();
    conn.query_row(
        "SELECT sha256, family, kind, title, authors, year, language, pages, source_url,
                download_url, path, location_root, bytes, retrieved_at, text_layer_path,
                collection, original_language, transliterated_title, soviet_stratum
         FROM document WHERE sha256 = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "sha256": row.get::<_, String>(0)?,
                "family": row.get::<_, String>(1)?,
                "kind": row.get::<_, String>(2)?,
                "title": row.get::<_, Option<String>>(3)?,
                "authors": row.get::<_, Option<String>>(4)?,
                "year": row.get::<_, Option<i64>>(5)?,
                "language": row.get::<_, Option<String>>(6)?,
                "pages": row.get::<_, Option<i64>>(7)?,
                "source_url": row.get::<_, Option<String>>(8)?,
                "download_url": row.get::<_, Option<String>>(9)?,
                "path": row.get::<_, String>(10)?,
                "location_root": row.get::<_, String>(11)?,
                "bytes": row.get::<_, Option<i64>>(12)?,
                "retrieved_at": row.get::<_, Option<String>>(13)?,
                "text_layer_path": row.get::<_, Option<String>>(14)?,
                "collection": row.get::<_, Option<String>>(15)?,
                "original_language": row.get::<_, Option<String>>(16)?,
                "transliterated_title": row.get::<_, Option<String>>(17)?,
                "soviet_stratum": row.get::<_, Option<String>>(18)?,
            }))
        },
    )
    .optional()
    .map_err(internal)
}

// ---------- server ----------

/// Lazily loaded semantic state: the ONNX embedder (first use) plus the
/// byte-capped per-corpus vector cache.
pub struct SemanticSlot {
    cache_dir: PathBuf,
    embedder: Option<Box<dyn semantic::Embedder>>,
    cache: semantic::VectorCache,
    pplx_url: Option<String>,
    pplx_embedder: Option<semantic::PplxHttpEmbedder>,
    pplx_cache: semantic::PplxVectorCache,
}

impl SemanticSlot {
    pub fn new(cache_dir: PathBuf) -> Self {
        SemanticSlot {
            cache_dir,
            embedder: None,
            cache: semantic::VectorCache::default(),
            pplx_url: std::env::var("STACKS_PPLX_URL").ok(),
            pplx_embedder: None,
            pplx_cache: semantic::PplxVectorCache::default(),
        }
    }

    fn embedder(&mut self) -> Result<&mut dyn semantic::Embedder, McpError> {
        if self.embedder.is_none() {
            let e = semantic::FastEmbedder::new(&self.cache_dir).map_err(internal)?;
            self.embedder = Some(Box::new(e));
        }
        Ok(self.embedder.as_deref_mut().expect("just loaded"))
    }

    fn pplx(&mut self) -> Result<&mut semantic::PplxHttpEmbedder, McpError> {
        let Some(url) = &self.pplx_url else {
            return Err(McpError::invalid_params(
                "backend=pplx not configured (STACKS_PPLX_URL unset)",
                None,
            ));
        };
        if self.pplx_embedder.is_none() {
            self.pplx_embedder = Some(semantic::PplxHttpEmbedder::new(url));
        }
        Ok(self.pplx_embedder.as_mut().expect("just created"))
    }
}

#[derive(Clone)]
pub struct StacksMcp {
    library: Arc<Mutex<LibraryStore>>,
    semantic: Arc<Mutex<SemanticSlot>>,
    // Read by the #[tool_handler]-generated dispatch; the dead-code lint
    // doesn't see the macro expansion.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl StacksMcp {
    pub fn new(library: LibraryStore, cache_dir: PathBuf) -> Self {
        StacksMcp {
            library: Arc::new(Mutex::new(library)),
            semantic: Arc::new(Mutex::new(SemanticSlot::new(cache_dir))),
            tool_router: Self::tool_router(),
        }
    }

    fn with_library<T>(
        &self,
        f: impl FnOnce(&LibraryStore) -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        let lib = self.library.lock().map_err(internal)?;
        f(&lib)
    }
}

#[tool_router]
impl StacksMcp {
    #[tool(
        name = "search_chunks_bm25",
        description = "FTS5 BM25 full-text search over library chunks; returns snippets. Read-only."
    )]
    async fn search_chunks_bm25(
        &self,
        Parameters(args): Parameters<SearchChunksBm25Args>,
    ) -> Result<CallToolResult, McpError> {
        let limit = check_limit(args.limit)?;
        if args.query.trim().is_empty() {
            return Err(McpError::invalid_params("query must be non-empty", None));
        }
        let v = self.with_library(|lib| {
            bm25_search(lib, &args.query, args.corpus.as_deref(), limit)
        })?;
        json_text(&v)
    }

    #[tool(
        name = "search_chunks_semantic",
        description = "Dense cosine KNN (mode=dense) or hybrid RRF(k=60) fusion with BM25 (mode=hybrid, default) over chunk embeddings. backend=minilm (default) or pplx (shadow index via laptop sidecar). Read-only."
    )]
    async fn search_chunks_semantic(
        &self,
        Parameters(args): Parameters<SearchChunksSemanticArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args.query.trim().is_empty() {
            return Err(McpError::invalid_params("query must be non-empty", None));
        }
        let k = args.k.unwrap_or(20);
        if k == 0 || k > MAX_K {
            return Err(McpError::invalid_params(
                format!("k must be within 1..={MAX_K}, got {k}"),
                None,
            ));
        }
        let mode: &'static str = match args.mode.as_deref() {
            None | Some("hybrid") => "hybrid",
            Some("dense") => "dense",
            Some(v) => {
                return Err(McpError::invalid_params(
                    format!("unknown mode {v:?}; allowed: dense, hybrid"),
                    None,
                ))
            }
        };
        let library = self.library.clone();
        let sem = self.semantic.clone();
        let query = args.query;
        let corpus = args.corpus;
        let backend = match args.backend.as_deref() {
            None | Some("minilm") => "minilm",
            Some("pplx") => "pplx",
            Some(v) => {
                return Err(McpError::invalid_params(
                    format!("unknown backend {v:?}; allowed: minilm, pplx"),
                    None,
                ))
            }
        };
        tokio::task::spawn_blocking(move || -> Result<CallToolResult, McpError> {
            let mut sem = sem.lock().map_err(internal)?;
            let lib = library.lock().map_err(internal)?;
            let conn = lib.raw();
            let (dense, cache_bytes) = if backend == "pplx" {
                let query_vec = semantic::Embedder::embed(sem.pplx()?, &query).map_err(internal)?;
                let model: String = conn
                    .query_row(
                        "SELECT model FROM chunk_embedding_pplx
                         ORDER BY embedded_at DESC LIMIT 1",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(|_| {
                        McpError::internal_error(
                            "no pplx embeddings indexed yet (run pplx-backfill)",
                            None,
                        )
                    })?;
                let d = semantic::dense_search_pplx(
                    &lib,
                    &mut sem.pplx_cache,
                    &model,
                    corpus.as_deref(),
                    &query_vec,
                    k as usize,
                )
                .map_err(internal)?;
                (d, sem.pplx_cache.cached_bytes())
            } else {
                let query_vec = sem.embedder()?.embed(&query).map_err(internal)?;
                let d = semantic::dense_search(
                    &lib,
                    &mut sem.cache,
                    corpus.as_deref(),
                    &query_vec,
                    k as usize,
                )
                .map_err(internal)?;
                (d, sem.cache.cached_bytes())
            };
            let (hits, sparse_count) = if mode == "hybrid" {
                let sparse = bm25_topk(conn, &query, corpus.as_deref(), k as usize)?;
                let n = sparse.len();
                (semantic::rrf_fuse(&dense, &sparse, 60, k as usize), n)
            } else {
                (dense.iter().map(|(r, s)| (*r, *s as f64)).collect(), 0usize)
            };
            let data = hydrate_hits(conn, &hits)?;
            json_text(&serde_json::json!({
                "data": data,
                "meta": {
                    "k": k,
                    "max_k": MAX_K,
                    "mode": mode,
                    "corpus": corpus,
                    "dense_candidates": dense.len(),
                    "sparse_candidates": sparse_count,
                    "fusion": if mode == "hybrid" { "rrf(k=60)" } else { "none" },
                    "backend": backend,
                    "vector_cache_bytes": cache_bytes,
                },
            }))
        })
        .await
        .map_err(internal)?
    }

    #[tool(
        name = "get_paper",
        description = "One paper by sha256: catalog row + enrichment + chunk count. Read-only."
    )]
    async fn get_paper(
        &self,
        Parameters(args): Parameters<GetPaperArgs>,
    ) -> Result<CallToolResult, McpError> {
        let Some(v) = self.with_library(|lib| paper_detail(lib, &args.sha256))? else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no paper with sha256 {}",
                args.sha256
            ))]));
        };
        json_text(&v)
    }

    #[tool(
        name = "list_papers",
        description = "Search library papers; filters query/subfield/year_from/year_to, keyset cursor pagination. Read-only."
    )]
    async fn list_papers(
        &self,
        Parameters(args): Parameters<ListPapersArgs>,
    ) -> Result<CallToolResult, McpError> {
        let limit = check_limit(args.limit)?;
        let v = self.with_library(|lib| papers_list(lib, &args, limit))?;
        json_text(&v)
    }

    #[tool(
        name = "library_status",
        description = "Library table counts, per-corpus chunk counts, and inproc_queue status breakdown. Read-only."
    )]
    async fn library_status(&self) -> Result<CallToolResult, McpError> {
        let v = self.with_library(status)?;
        json_text(&v)
    }

    #[tool(
        name = "get_document",
        description = "One acquisition-bay document by sha256. Read-only."
    )]
    async fn get_document(
        &self,
        Parameters(args): Parameters<GetDocumentArgs>,
    ) -> Result<CallToolResult, McpError> {
        let Some(v) = self.with_library(|lib| document_detail(lib, &args.id))? else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no document with sha256 {}",
                args.id
            ))]));
        };
        json_text(&v)
    }

    #[tool(
        name = "get_chunk",
        description = "Full text of one chunk by corpus + chunk_id (from a search hit). Read-only."
    )]
    async fn get_chunk(
        &self,
        Parameters(args): Parameters<GetChunkArgs>,
    ) -> Result<CallToolResult, McpError> {
        let Some(v) = self.with_library(|lib| chunk_detail(lib, &args.corpus, args.chunk_id))? else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no chunk {}#{}",
                args.corpus, args.chunk_id
            ))]));
        };
        json_text(&v)
    }
}

#[tool_handler]
impl ServerHandler for StacksMcp {
    fn get_info(&self) -> ServerConfig {
        let mut server_info = Implementation::from_build_env();
        server_info.name = "stacks-mcp".into();
        server_info.version = env!("CARGO_PKG_VERSION").into();
        let mut info = ServerConfig::default();
        info.protocol_version = ProtocolVersion::LATEST_WITH_INITIALIZE;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = server_info;
        info.instructions = Some(
            "Read-only search over the stacks research library: BM25/semantic chunk \
             search, full chunk fetch, paper and document lookup, catalog listing, \
             library status."
                .into(),
        );
        info
    }
}

/// Streamable-HTTP MCP service over an opened (read-only) library store.
/// Mount with `Router::new().nest_service("/mcp", build_service(..))`.
pub fn build_service(
    library: LibraryStore,
    cache_dir: PathBuf,
) -> StreamableHttpService<StacksMcp, LocalSessionManager> {
    build_service_with_hosts(library, cache_dir, Vec::new())
}

/// Like [`build_service`], with extra entries for the Host-header allowlist
/// (rmcp defaults to loopback only; a reverse proxy such as `tailscale serve`
/// forwards the public hostname).
pub fn build_service_with_hosts(
    library: LibraryStore,
    cache_dir: PathBuf,
    extra_allowed_hosts: Vec<String>,
) -> StreamableHttpService<StacksMcp, LocalSessionManager> {
    let library = Arc::new(Mutex::new(library));
    let sem = Arc::new(Mutex::new(SemanticSlot::new(cache_dir)));
    let mut config = StreamableHttpServerConfig::default();
    if !extra_allowed_hosts.is_empty() {
        let mut hosts = config.allowed_hosts.clone();
        hosts.extend(extra_allowed_hosts);
        config = config.with_allowed_hosts(hosts);
    }
    StreamableHttpService::new(
        move || {
            Ok(StacksMcp {
                library: library.clone(),
                semantic: sem.clone(),
                tool_router: StacksMcp::tool_router(),
            })
        },
        Arc::new(LocalSessionManager::default()),
        config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_args_deserialize() {
        let a: SearchChunksBm25Args =
            serde_json::from_value(serde_json::json!({"query": "sintering"})).unwrap();
        assert_eq!(a.query, "sintering");
        assert!(a.corpus.is_none() && a.limit.is_none());
        let a: SearchChunksBm25Args = serde_json::from_value(
            serde_json::json!({"query": "x", "corpus": "wave1", "limit": 10}),
        )
        .unwrap();
        assert_eq!(a.corpus.as_deref(), Some("wave1"));
        assert_eq!(a.limit, Some(10));
        assert!(serde_json::from_value::<SearchChunksBm25Args>(serde_json::json!({})).is_err());
    }

    #[test]
    fn semantic_args_defaults() {
        let a: SearchChunksSemanticArgs =
            serde_json::from_value(serde_json::json!({"query": "ferroelectric"})).unwrap();
        assert!(a.mode.is_none() && a.k.is_none());
        let a: SearchChunksSemanticArgs = serde_json::from_value(
            serde_json::json!({"query": "x", "mode": "dense", "k": 50}),
        )
        .unwrap();
        assert_eq!(a.mode.as_deref(), Some("dense"));
        assert_eq!(a.k, Some(50));
    }

    #[test]
    fn list_papers_args_deserialize() {
        let a: ListPapersArgs = serde_json::from_value(serde_json::json!({
            "query": "zirconia", "subfield": "ceramics", "year_from": 1990,
            "year_to": 2005, "cursor": 42, "limit": 25,
        }))
        .unwrap();
        assert_eq!(a.year_from, Some(1990));
        assert_eq!(a.cursor, Some(42));
        assert!(serde_json::from_value::<ListPapersArgs>(serde_json::json!({"cursor": "x"}))
            .is_err());
    }

    #[test]
    fn scalar_args_deserialize() {
        let g: GetPaperArgs =
            serde_json::from_value(serde_json::json!({"sha256": "ab".repeat(32)})).unwrap();
        assert_eq!(g.sha256.len(), 64);
        let d: GetDocumentArgs =
            serde_json::from_value(serde_json::json!({"id": "cd".repeat(32)})).unwrap();
        assert_eq!(d.id.len(), 64);
        assert!(serde_json::from_value::<GetDocumentArgs>(serde_json::json!({})).is_err());
    }

    #[test]
    fn limit_validation() {
        assert_eq!(check_limit(None).unwrap(), DEFAULT_LIMIT);
        assert_eq!(check_limit(Some(1)).unwrap(), 1);
        assert_eq!(check_limit(Some(MAX_LIMIT)).unwrap(), MAX_LIMIT);
        assert!(check_limit(Some(0)).is_err());
        assert!(check_limit(Some(MAX_LIMIT + 1)).is_err());
    }

    #[test]
    fn arg_schemas_generate() {
        let _ = schemars::schema_for!(SearchChunksBm25Args);
        let _ = schemars::schema_for!(SearchChunksSemanticArgs);
        let _ = schemars::schema_for!(GetPaperArgs);
        let _ = schemars::schema_for!(ListPapersArgs);
        let _ = schemars::schema_for!(GetDocumentArgs);
    }
}
