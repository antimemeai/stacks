//! stacks-api: localhost read API over the stacks system of record.
//!
//! This is the LLM-ergonomics surface: agents discover the model from
//! `GET /api/v1/schemas`, page deterministically with keyset cursors, and
//! receive stable machine-readable errors.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::OptionalExtension;
use serde::Serialize;
use stacks_core::Store;

pub mod semantic;

/// Hard cap on page size, advertised in every list response's meta block.
pub const MAX_LIMIT: u32 = 100;
const DEFAULT_LIMIT: u32 = 50;

/// Stable machine-readable error codes (closed set).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidQuery,
    NotFound,
    Internal,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    code: ErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl ApiError {
    pub fn invalid_query(message: impl Into<String>, details: Option<serde_json::Value>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            body: ErrorBody {
                code: ErrorCode::InvalidQuery,
                message: message.into(),
                details,
            },
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::NOT_FOUND,
            body: ErrorBody {
                code: ErrorCode::NotFound,
                message: message.into(),
                details: None,
            },
        }
    }

    /// Internal failures are deliberately generic: no internals in the body.
    pub fn internal() -> Self {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ErrorBody {
                code: ErrorCode::Internal,
                message: "internal error".to_string(),
                details: None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.body }))).into_response()
    }
}

#[derive(Clone)]
struct AppState {
    store: Arc<Mutex<Store>>,
    library: Option<Arc<Mutex<stacks_core::library::LibraryStore>>>,
    materials: Option<Arc<Mutex<stacks_core::materials::MaterialsStore>>>,
    semantic: Option<Arc<SemanticState>>,
}

/// Dense retrieval state: the query embedder plus the lazy vector cache.
pub struct SemanticState {
    embedder: Mutex<Box<dyn semantic::Embedder>>,
    cache: Mutex<semantic::VectorCache>,
}

impl AppState {
    fn with<T>(
        &self,
        f: impl FnOnce(&Store) -> Result<T, stacks_core::StoreError>,
    ) -> Result<T, ApiError> {
        let store = self.store.lock().map_err(|_| ApiError::internal())?;
        f(&store).map_err(|_| ApiError::internal())
    }

    fn with_materials<T>(
        &self,
        f: impl FnOnce(&stacks_core::materials::MaterialsStore) -> Result<T, stacks_core::StoreError>,
    ) -> Result<T, ApiError> {
        let Some(m) = &self.materials else {
            return Err(ApiError::not_found(
                "materials database not mounted on this instance",
            ));
        };
        let m = m.lock().map_err(|_| ApiError::internal())?;
        f(&m).map_err(|_| ApiError::internal())
    }

    fn with_library<T>(
        &self,
        f: impl FnOnce(&stacks_core::library::LibraryStore) -> Result<T, stacks_core::StoreError>,
    ) -> Result<T, ApiError> {
        let Some(lib) = &self.library else {
            return Err(ApiError::not_found(
                "library database not mounted on this instance",
            ));
        };
        let lib = lib.lock().map_err(|_| ApiError::internal())?;
        f(&lib).map_err(|_| ApiError::internal())
    }
}

/// Build the API router over an opened store. Listener construction lives in
/// `main` (isolated for the future tsnet swap); tests mount this directly.
pub fn build_app(store: Store) -> Router {
    build_app_full(store, None)
}

/// Full app: recipe store plus the optional library database.
pub fn build_app_full(store: Store, library: Option<stacks_core::library::LibraryStore>) -> Router {
    build_app_semantic(store, library, None)
}

/// Full app with optional semantic search state (embedder + vector cache).
pub fn build_app_semantic(
    store: Store,
    library: Option<stacks_core::library::LibraryStore>,
    embedder: Option<Box<dyn semantic::Embedder>>,
) -> Router {
    build_app_materials(store, library, embedder, None)
}

/// Everything plus the materials pillar.
pub fn build_app_materials(
    store: Store,
    library: Option<stacks_core::library::LibraryStore>,
    embedder: Option<Box<dyn semantic::Embedder>>,
    materials: Option<stacks_core::materials::MaterialsStore>,
) -> Router {
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
        library: library.map(|l| Arc::new(Mutex::new(l))),
        semantic: embedder.map(|e| {
            Arc::new(SemanticState {
                embedder: Mutex::new(e),
                cache: Mutex::new(semantic::VectorCache::default()),
            })
        }),
        materials: materials.map(|m| Arc::new(Mutex::new(m))),
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1", get(api_doc))
        .route("/api/v1/schemas", get(schemas_index))
        .route("/api/v1/schemas/{name}", get(schema_by_name))
        .route("/api/v1/recipes", get(list_recipes))
        .route("/api/v1/recipes/{id}", get(get_recipe))
        .route("/api/v1/recipes/materials", get(list_materials))
        .route("/api/v1/papers", get(list_papers))
        .route("/api/v1/papers/{sha256}", get(get_paper))
        .route("/api/v1/papers/{sha256}/chunks", get(get_paper_chunks))
        .route("/api/v1/chunks/search", get(search_chunks))
        .route("/api/v1/chunks/semantic", get(semantic_chunks))
        .route("/api/v1/library/status", get(library_status))
        .route("/api/v1/documents", get(list_documents))
        .route("/api/v1/documents/status", get(documents_status))
        .route("/api/v1/datasets", get(list_datasets))
        .route("/api/v1/datasets/status", get(datasets_status))
        .route("/api/v1/datasets/{id}", get(get_dataset))
        .route("/api/v1/documents/{sha256}", get(get_document))
        .route("/api/v1/materials", get(list_material_entries))
        .route("/api/v1/materials/status", get(materials_status))
        .route("/api/v1/materials/{external_key}", get(get_material_entry))
        .with_state(state)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn api_doc() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "stacks-api",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "GET /healthz": "liveness probe",
            "GET /api/v1/schemas": "index of JSON Schemas for the core model",
            "GET /api/v1/schemas/{name}": "JSON Schema for one model type",
            "GET /api/v1/recipes": "search recipes; params: query, source, status, limit, cursor",
            "GET /api/v1/recipes/{id}": "full recipe document (steps, materials, provenance)",
            "GET /api/v1/recipes/materials": "search recipe materials (moved from /api/v1/materials in wave F); params: query, kind, limit, cursor",
            "GET /api/v1/papers": "search library papers; params: query, subfield, year_min, year_max, has_doi, limit, cursor",
            "GET /api/v1/papers/{sha256}": "paper detail: catalog row + enrichment + chunk count",
            "GET /api/v1/papers/{sha256}/chunks": "keyset-paged chunks of one paper",
            "GET /api/v1/chunks/search": "FTS5 BM25 full-text search over chunks; params: query, corpus, limit, cursor",
            "GET /api/v1/chunks/semantic": "dense KNN (exact cosine) or hybrid RRF; params: query, corpus, k (cap 50), mode=dense|hybrid",
            "GET /api/v1/library/status": "library import provenance: source mtimes, imported_at, per-corpus counts",
            "GET /api/v1/documents": "acquisition-bay documents; params: query, family, kind, language, collection, limit, cursor",
            "GET /api/v1/documents/status": "document corpus counts by family/kind/collection + intake triage summary",
            "GET /api/v1/datasets": "dataset registry; params: query, domain, status, limit, cursor",
            "GET /api/v1/datasets/{id}": "one dataset (numeric id or exact name)",
            "GET /api/v1/datasets/status": "registry counts by status + sha256_status",
            "GET /api/v1/documents/{sha256}": "one document row (payload referenced in place via location_root + path)",
            "GET /api/v1/materials": "computational materials (MP/COD/OQMD/TOP4040); params: query, elements, source, band_gap_min, band_gap_max, limit, cursor",
            "GET /api/v1/materials/{external_key}": "one entry: structure summary + property bundle + robocrys text",
            "GET /api/v1/materials/status": "materials import provenance and per-source counts"
        },
        "conventions": {
            "pagination": "keyset cursors; pass meta.next_cursor as ?cursor=; limit cap is advertised per response",
            "errors": "{\"error\": {\"code\": \"invalid_query|not_found|internal\", \"message\": ..., \"details\"?}}",
            "schemas": "the model contract lives at /api/v1/schemas — introspect rather than guess"
        }
    }))
}

type SchemaEntry = (&'static str, fn() -> schemars::schema::RootSchema);

fn schema_registry() -> Vec<SchemaEntry> {
    vec![
        ("Recipe", || schemars::schema_for!(stacks_core::Recipe)),
        ("Material", || schemars::schema_for!(stacks_core::Material)),
        ("RecipeStep", || {
            schemars::schema_for!(stacks_core::RecipeStep)
        }),
        ("StepMaterial", || {
            schemars::schema_for!(stacks_core::StepMaterial)
        }),
        ("Quantity", || schemars::schema_for!(stacks_core::Quantity)),
        ("Conditions", || {
            schemars::schema_for!(stacks_core::Conditions)
        }),
        ("Run", || schemars::schema_for!(stacks_core::Run)),
        ("RunStep", || schemars::schema_for!(stacks_core::RunStep)),
        ("Provenance", || {
            schemars::schema_for!(stacks_core::Provenance)
        }),
        ("Unit", || schemars::schema_for!(stacks_core::Unit)),
        ("Operator", || schemars::schema_for!(stacks_core::Operator)),
        ("ChangeLogEntry", || {
            schemars::schema_for!(stacks_core::ChangeLogEntry)
        }),
        ("LibraryPaper", || {
            schemars::schema_for!(stacks_core::library::LibraryPaper)
        }),
        ("PaperEnrichment", || {
            schemars::schema_for!(stacks_core::library::PaperEnrichment)
        }),
        ("LibraryChunk", || {
            schemars::schema_for!(stacks_core::library::LibraryChunk)
        }),
        ("LibraryDocument", || {
            schemars::schema_for!(stacks_core::library::LibraryDocument)
        }),
        ("MaterialEntry", || {
            schemars::schema_for!(stacks_core::materials::MaterialEntry)
        }),
        ("Dataset", || {
            schemars::schema_for!(stacks_core::library::Dataset)
        }),
        ("Property", || {
            schemars::schema_for!(stacks_core::materials::Property)
        }),
    ]
}

async fn schemas_index() -> Json<serde_json::Value> {
    let names: Vec<&str> = schema_registry().iter().map(|(n, _)| *n).collect();
    Json(serde_json::json!({
        "schemas": names,
        "url_template": "/api/v1/schemas/{name}"
    }))
}

async fn schema_by_name(Path(name): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let registry = schema_registry();
    let Some((_, make)) = registry.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)) else {
        return Err(ApiError::not_found(format!(
            "no schema named {name:?}; see /api/v1/schemas"
        )));
    };
    let schema = make();
    Ok(Json(
        serde_json::to_value(schema).map_err(|_| ApiError::internal())?,
    ))
}

/// Parsed, validated list-recipes parameters.
struct RecipeParams {
    query: Option<String>,
    source: Option<String>,
    status: Option<String>,
    limit: u32,
    cursor: i64,
}

fn parse_recipe_params(raw: &HashMap<String, String>) -> Result<RecipeParams, ApiError> {
    for key in raw.keys() {
        if !["query", "source", "status", "limit", "cursor"].contains(&key.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown query parameter {key:?}"),
                Some(serde_json::json!({
                    "allowed": ["query", "source", "status", "limit", "cursor"]
                })),
            ));
        }
    }

    let limit = match raw.get("limit") {
        None => DEFAULT_LIMIT,
        Some(v) => {
            let n: u32 = v.parse().map_err(|_| {
                ApiError::invalid_query(format!("limit must be an integer, got {v:?}"), None)
            })?;
            if n == 0 || n > MAX_LIMIT {
                return Err(ApiError::invalid_query(
                    format!("limit must be within 1..={MAX_LIMIT}, got {n}"),
                    Some(serde_json::json!({"max": MAX_LIMIT})),
                ));
            }
            n
        }
    };
    let cursor = match raw.get("cursor") {
        None => 0,
        Some(v) => v.parse().map_err(|_| {
            ApiError::invalid_query(format!("cursor must be an integer id, got {v:?}"), None)
        })?,
    };
    if let Some(status) = raw.get("status") {
        if !["draft", "deployed", "retired"].contains(&status.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown status {status:?}"),
                Some(serde_json::json!({"allowed": ["draft", "deployed", "retired"]})),
            ));
        }
    }
    Ok(RecipeParams {
        query: raw.get("query").filter(|q| !q.trim().is_empty()).cloned(),
        source: raw.get("source").filter(|s| !s.trim().is_empty()).cloned(),
        status: raw.get("status").cloned(),
        limit,
        cursor,
    })
}

async fn list_recipes(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let params = parse_recipe_params(&raw)?;
    let like = params.query.as_ref().map(|q| format!("%{q}%"));
    state
        .with(|store| {
            let conn = store.raw();
            let mut sql = String::from(
            "SELECT r.id, r.name, r.version, r.status, r.external_key, m.formula, p.source_dataset
             FROM recipe r
             LEFT JOIN material m ON m.id = r.target_material_id
             LEFT JOIN provenance p ON p.id = r.provenance_id
             WHERE r.id > :cursor",
        );
            if like.is_some() {
                sql.push_str(" AND (r.name LIKE :like OR m.formula LIKE :like)");
            }
            if params.source.is_some() {
                sql.push_str(" AND p.source_dataset = :source");
            }
            if params.status.is_some() {
                sql.push_str(" AND r.status = :status");
            }
            sql.push_str(" ORDER BY r.id LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = params.limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &params.cursor), (":limit", &limit_plus_one)];
            if let Some(l) = &like {
                bindings.push((":like", l));
            }
            if let Some(s) = &params.source {
                bindings.push((":source", s));
            }
            if let Some(s) = &params.status {
                bindings.push((":status", s));
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
                            "id": row.get::<_, i64>(0)?,
                            "name": row.get::<_, String>(1)?,
                            "version": row.get::<_, i64>(2)?,
                            "status": row.get::<_, String>(3)?,
                            "external_key": row.get::<_, Option<String>>(4)?,
                            "target_formula": row.get::<_, Option<String>>(5)?,
                            "source_dataset": row.get::<_, Option<String>>(6)?,
                        }))
                    },
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > params.limit {
                rows.truncate(params.limit as usize);
                rows.last().and_then(|r| r["id"].as_i64())
            } else {
                None
            };
            Ok(serde_json::json!({
                "data": rows,
                "meta": {
                    "limit": params.limit,
                    "max_limit": MAX_LIMIT,
                    "next_cursor": next_cursor,
                    "ordering": "id ASC (keyset; pass meta.next_cursor as ?cursor=)"
                }
            }))
        })
        .map(Json)
}

async fn get_recipe(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bundle = state.with(|store| store.load_recipe(id))?;
    let Some(bundle) = bundle else {
        return Err(ApiError::not_found(format!("no recipe with id {id}")));
    };
    let doc = serde_json::json!({
        "recipe": bundle.recipe,
        "provenance": bundle.provenance,
        "steps": bundle
            .steps
            .iter()
            .map(|s| serde_json::json!({
                "step": s.step,
                "materials": s.materials,
            }))
            .collect::<Vec<_>>(),
    });
    Ok(Json(doc))
}

struct MaterialParams {
    query: Option<String>,
    kind: Option<String>,
    limit: u32,
    cursor: i64,
}

fn parse_material_params(raw: &HashMap<String, String>) -> Result<MaterialParams, ApiError> {
    for key in raw.keys() {
        if !["query", "kind", "limit", "cursor"].contains(&key.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown query parameter {key:?}"),
                Some(serde_json::json!({"allowed": ["query", "kind", "limit", "cursor"]})),
            ));
        }
    }
    for key in raw.keys() {
        if !["query", "kind", "limit", "cursor"].contains(&key.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown query parameter {key:?}"),
                Some(serde_json::json!({"allowed": ["query", "kind", "limit", "cursor"]})),
            ));
        }
    }
    let limit = match raw.get("limit") {
        None => DEFAULT_LIMIT,
        Some(v) => {
            let n: u32 = v.parse().map_err(|_| {
                ApiError::invalid_query(format!("limit must be an integer, got {v:?}"), None)
            })?;
            if n == 0 || n > MAX_LIMIT {
                return Err(ApiError::invalid_query(
                    format!("limit must be within 1..={MAX_LIMIT}, got {n}"),
                    Some(serde_json::json!({"max": MAX_LIMIT})),
                ));
            }
            n
        }
    };
    let cursor = match raw.get("cursor") {
        None => 0,
        Some(v) => v.parse().map_err(|_| {
            ApiError::invalid_query(format!("cursor must be an integer id, got {v:?}"), None)
        })?,
    };
    if let Some(kind) = raw.get("kind") {
        let closed = ["molecule", "formula", "mixture"];
        if !closed.contains(&kind.as_str()) && !kind.starts_with("other:") {
            return Err(ApiError::invalid_query(
                format!("unknown material kind {kind:?}"),
                Some(serde_json::json!({
                    "allowed": ["molecule", "formula", "mixture", "other:<note>"]
                })),
            ));
        }
    }
    Ok(MaterialParams {
        query: raw.get("query").filter(|q| !q.trim().is_empty()).cloned(),
        kind: raw.get("kind").cloned(),
        limit,
        cursor,
    })
}

async fn list_materials(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let params = parse_material_params(&raw)?;
    let like = params.query.as_ref().map(|q| format!("%{q}%"));
    state
        .with(|store| {
            let conn = store.raw();
            let mut sql = String::from(
                "SELECT id, kind, formula, canonical_smiles, names_json, cas, identity
             FROM material WHERE id > :cursor",
            );
            if like.is_some() {
                sql.push_str(
                " AND (names_json LIKE :like OR formula LIKE :like OR canonical_smiles LIKE :like)",
            );
            }
            if params.kind.is_some() {
                sql.push_str(" AND kind = :kind");
            }
            sql.push_str(" ORDER BY id LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = params.limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &params.cursor), (":limit", &limit_plus_one)];
            if let Some(l) = &like {
                bindings.push((":like", l));
            }
            if let Some(k) = &params.kind {
                bindings.push((":kind", k));
            }
            let rows: Vec<serde_json::Value> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |row| {
                        let names: Vec<String> =
                            serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default();
                        Ok(serde_json::json!({
                            "id": row.get::<_, i64>(0)?,
                            "kind": row.get::<_, String>(1)?,
                            "formula": row.get::<_, Option<String>>(2)?,
                            "canonical_smiles": row.get::<_, Option<String>>(3)?,
                            "names": names,
                            "cas": row.get::<_, Option<String>>(5)?,
                            "identity": row.get::<_, Option<String>>(6)?,
                        }))
                    },
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > params.limit {
                rows.truncate(params.limit as usize);
                rows.last().and_then(|r| r["id"].as_i64())
            } else {
                None
            };
            Ok(serde_json::json!({
                "data": rows,
                "meta": {
                    "limit": params.limit,
                    "max_limit": MAX_LIMIT,
                    "next_cursor": next_cursor,
                    "ordering": "id ASC (keyset; pass meta.next_cursor as ?cursor=)"
                }
            }))
        })
        .map(Json)
}

// ---------- library endpoints ----------

/// Validate the query-param set; reject anything unknown with the stable
/// error shape. Shared by the library handlers.
fn reject_unknown_params(raw: &HashMap<String, String>, allowed: &[&str]) -> Result<(), ApiError> {
    for key in raw.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown query parameter {key:?}"),
                Some(serde_json::json!({ "allowed": allowed })),
            ));
        }
    }
    Ok(())
}

fn parse_limit(raw: &HashMap<String, String>) -> Result<u32, ApiError> {
    match raw.get("limit") {
        None => Ok(DEFAULT_LIMIT),
        Some(v) => {
            let n: u32 = v.parse().map_err(|_| {
                ApiError::invalid_query(format!("limit must be an integer, got {v:?}"), None)
            })?;
            if n == 0 || n > MAX_LIMIT {
                return Err(ApiError::invalid_query(
                    format!("limit must be within 1..={MAX_LIMIT}, got {n}"),
                    Some(serde_json::json!({"max": MAX_LIMIT})),
                ));
            }
            Ok(n)
        }
    }
}

fn parse_cursor(raw: &HashMap<String, String>) -> Result<i64, ApiError> {
    match raw.get("cursor") {
        None => Ok(0),
        Some(v) => v.parse().map_err(|_| {
            ApiError::invalid_query(format!("cursor must be an integer id, got {v:?}"), None)
        }),
    }
}

fn meta(limit: u32, next_cursor: Option<i64>) -> serde_json::Value {
    serde_json::json!({
        "limit": limit,
        "max_limit": MAX_LIMIT,
        "next_cursor": next_cursor,
        "ordering": "rowid ASC (keyset; pass meta.next_cursor as ?cursor=)"
    })
}

const PAPER_COLS: &str = "sha256, filename, path, size_bytes, registered_at, on_disk, doi,
    arxiv_id, title, authors, year, abstract, journal, source_url, access, blob_key,
    blob_synced_at, subfield, tags, original_language, original_script_title,
    transliterated_title, translation_of, translated_in, soviet_stratum, source_collection";

fn paper_from_row(row: &rusqlite::Row) -> rusqlite::Result<stacks_core::library::LibraryPaper> {
    Ok(stacks_core::library::LibraryPaper {
        sha256: row.get(0)?,
        filename: row.get(1)?,
        path: row.get(2)?,
        size_bytes: row.get(3)?,
        registered_at: row.get(4)?,
        on_disk: row.get(5)?,
        doi: row.get(6)?,
        arxiv_id: row.get(7)?,
        title: row.get(8)?,
        authors: row.get(9)?,
        year: row.get(10)?,
        abstract_: row.get(11)?,
        journal: row.get(12)?,
        source_url: row.get(13)?,
        access: row.get(14)?,
        blob_key: row.get(15)?,
        blob_synced_at: row.get(16)?,
        subfield: row.get(17)?,
        tags: row.get(18)?,
        original_language: row.get(19)?,
        original_script_title: row.get(20)?,
        transliterated_title: row.get(21)?,
        translation_of: row.get(22)?,
        translated_in: row.get(23)?,
        soviet_stratum: row.get(24)?,
        source_collection: row.get(25)?,
    })
}

async fn list_papers(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(
        &raw,
        &[
            "query", "subfield", "year_min", "year_max", "has_doi", "limit", "cursor",
        ],
    )?;
    let limit = parse_limit(&raw)?;
    let cursor = parse_cursor(&raw)?;
    let query = raw.get("query").filter(|q| !q.trim().is_empty()).cloned();
    let subfield = raw
        .get("subfield")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    let year_min = match raw.get("year_min") {
        None => None,
        Some(v) => Some(v.parse::<i64>().map_err(|_| {
            ApiError::invalid_query(format!("year_min must be an integer, got {v:?}"), None)
        })?),
    };
    let year_max = match raw.get("year_max") {
        None => None,
        Some(v) => Some(v.parse::<i64>().map_err(|_| {
            ApiError::invalid_query(format!("year_max must be an integer, got {v:?}"), None)
        })?),
    };
    let has_doi = match raw.get("has_doi").map(String::as_str) {
        None => None,
        Some("true") => Some(true),
        Some("false") => Some(false),
        Some(v) => {
            return Err(ApiError::invalid_query(
                format!("has_doi must be true or false, got {v:?}"),
                None,
            ))
        }
    };
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let mut sql = format!("SELECT rowid, {PAPER_COLS} FROM paper WHERE rowid > :cursor");
            if query.is_some() {
                sql.push_str(" AND (title LIKE :q OR authors LIKE :q OR filename LIKE :q)");
            }
            if subfield.is_some() {
                sql.push_str(" AND subfield = :subfield");
            }
            if year_min.is_some() {
                sql.push_str(" AND year >= :year_min");
            }
            if year_max.is_some() {
                sql.push_str(" AND year <= :year_max");
            }
            match has_doi {
                Some(true) => sql.push_str(" AND doi IS NOT NULL AND doi != ''"),
                Some(false) => sql.push_str(" AND (doi IS NULL OR doi = '')"),
                None => {}
            }
            sql.push_str(" ORDER BY rowid LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &cursor), (":limit", &limit_plus_one)];
            let like;
            if let Some(q) = &query {
                like = format!("%{q}%");
                bindings.push((":q", &like));
            }
            if let Some(s) = &subfield {
                bindings.push((":subfield", s));
            }
            if let Some(y) = &year_min {
                bindings.push((":year_min", y));
            }
            if let Some(y) = &year_max {
                bindings.push((":year_max", y));
            }
            let rows: Vec<(i64, stacks_core::library::LibraryPaper)> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |row| Ok((row.get(0)?, paper_from_row_at(row)?)),
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last().map(|(id, _)| *id)
            } else {
                None
            };
            let data: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(rowid, p)| {
                    let mut v = serde_json::to_value(&p)?;
                    v["rowid"] = serde_json::json!(rowid);
                    Ok(v)
                })
                .collect::<Result<_, serde_json::Error>>()?;
            Ok(serde_json::json!({"data": data, "meta": meta(limit, next_cursor)}))
        })
        .map(Json)
}

/// Same column order as PAPER_COLS, offset by one leading rowid column.
fn paper_from_row_at(row: &rusqlite::Row) -> rusqlite::Result<stacks_core::library::LibraryPaper> {
    Ok(stacks_core::library::LibraryPaper {
        sha256: row.get(1)?,
        filename: row.get(2)?,
        path: row.get(3)?,
        size_bytes: row.get(4)?,
        registered_at: row.get(5)?,
        on_disk: row.get(6)?,
        doi: row.get(7)?,
        arxiv_id: row.get(8)?,
        title: row.get(9)?,
        authors: row.get(10)?,
        year: row.get(11)?,
        abstract_: row.get(12)?,
        journal: row.get(13)?,
        source_url: row.get(14)?,
        access: row.get(15)?,
        blob_key: row.get(16)?,
        blob_synced_at: row.get(17)?,
        subfield: row.get(18)?,
        tags: row.get(19)?,
        original_language: row.get(20)?,
        original_script_title: row.get(21)?,
        transliterated_title: row.get(22)?,
        translation_of: row.get(23)?,
        translated_in: row.get(24)?,
        soviet_stratum: row.get(25)?,
        source_collection: row.get(26)?,
    })
}

async fn get_paper(
    State(state): State<AppState>,
    Path(sha256): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let paper = conn
                .query_row(
                    &format!("SELECT {PAPER_COLS} FROM paper WHERE sha256 = ?1"),
                    [&sha256],
                    paper_from_row,
                )
                .optional()?;
            let Some(paper) = paper else {
                return Ok(None);
            };
            let enrichment: Option<serde_json::Value> = conn
                .query_row(
                    "SELECT openalex_id, openalex_topics, openalex_concepts, openalex_cited_by,
                        s2_paper_id, s2_tldr, s2_fields_of_study, s2_influential_citation_count,
                        unpaywall_oa_status, unpaywall_oa_url, enriched_at
                 FROM paper_enrichment WHERE sha256 = ?1",
                    [&sha256],
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
                .optional()?;
            let chunk_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM chunk WHERE sha256 = ?1",
                [&sha256],
                |r| r.get(0),
            )?;
            Ok(Some(serde_json::json!({
                "paper": paper,
                "enrichment": enrichment,
                "chunk_count": chunk_count,
            })))
        })?
        .ok_or_else(|| ApiError::not_found(format!("no paper with sha256 {sha256}")))
        .map(Json)
}

async fn get_paper_chunks(
    State(state): State<AppState>,
    Path(sha256): Path<String>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(&raw, &["limit", "cursor"])?;
    let limit = parse_limit(&raw)?;
    let cursor = parse_cursor(&raw)?;
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM paper WHERE sha256 = ?1)",
                [&sha256],
                |r| r.get(0),
            )?;
            if !exists {
                return Ok(None);
            }
            let mut stmt = conn.prepare(
                "SELECT rowid, corpus, chunk_id, filename, title, section, text, word_count,
                    embedding IS NOT NULL
             FROM chunk WHERE sha256 = ?1 AND rowid > ?2 ORDER BY rowid LIMIT ?3",
            )?;
            let rows: Vec<serde_json::Value> = stmt
                .query_map(
                    rusqlite::params![&sha256, cursor, limit as i64 + 1],
                    |row| {
                        Ok(serde_json::json!({
                            "rowid": row.get::<_, i64>(0)?,
                            "corpus": row.get::<_, String>(1)?,
                            "chunk_id": row.get::<_, i64>(2)?,
                            "filename": row.get::<_, String>(3)?,
                            "title": row.get::<_, Option<String>>(4)?,
                            "section": row.get::<_, Option<String>>(5)?,
                            "text": row.get::<_, String>(6)?,
                            "word_count": row.get::<_, Option<i64>>(7)?,
                            "has_embedding": row.get::<_, bool>(8)?,
                        }))
                    },
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last().and_then(|r| r["rowid"].as_i64())
            } else {
                None
            };
            Ok(Some(
                serde_json::json!({"data": rows, "meta": meta(limit, next_cursor)}),
            ))
        })?
        .ok_or_else(|| ApiError::not_found(format!("no paper with sha256 {sha256}")))
        .map(Json)
}

async fn search_chunks(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(&raw, &["query", "corpus", "limit", "cursor"])?;
    let limit = parse_limit(&raw)?;
    let query = raw
        .get("query")
        .filter(|q| !q.trim().is_empty())
        .cloned()
        .ok_or_else(|| ApiError::invalid_query("query is required for /chunks/search", None))?;
    let corpus = raw.get("corpus").filter(|s| !s.trim().is_empty()).cloned();
    // Keyset over (bm25 rank, rowid): cursor is "<rank_bits>:<rowid>".
    // bm25() scores are negative-ish floats; ascending = best first.
    let (cursor_rank, cursor_rowid) = match raw.get("cursor") {
        None => (None, 0i64),
        Some(v) => {
            let (bits, rowid) = v.split_once(':').ok_or_else(|| {
                ApiError::invalid_query(
                    "cursor must be '<rank>:<rowid>' from meta.next_cursor",
                    None,
                )
            })?;
            let rank = f64::from_bits(
                u64::from_str_radix(bits, 16)
                    .map_err(|_| ApiError::invalid_query("malformed cursor rank", None))?,
            );
            let rowid: i64 = rowid
                .parse()
                .map_err(|_| ApiError::invalid_query("malformed cursor rowid", None))?;
            (Some(rank), rowid)
        }
    };
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let mut sql = String::from(
                "WITH hits AS (
                SELECT c.rowid AS rowid, bm25(chunk_fts) AS rank,
                       snippet(chunk_fts, 0, '<b>', '</b>', '…', 32) AS snip
                FROM chunk_fts JOIN chunk c ON c.rowid = chunk_fts.rowid
                WHERE chunk_fts MATCH :q",
            );
            if corpus.is_some() {
                sql.push_str(" AND c.corpus = :corpus");
            }
            sql.push_str(
                ")
            SELECT c.rowid, c.corpus, c.chunk_id, c.sha256, c.filename, c.section,
                   h.snip, h.rank
            FROM hits h JOIN chunk c ON c.rowid = h.rowid",
            );
            if cursor_rank.is_some() {
                sql.push_str(" WHERE (h.rank > :crank OR (h.rank = :crank AND h.rowid > :crowid))");
            }
            sql.push_str(" ORDER BY h.rank, h.rowid LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":q", &query), (":limit", &limit_plus_one)];
            if let Some(c) = &corpus {
                bindings.push((":corpus", c));
            }
            let crank = cursor_rank.unwrap_or(0.0);
            if cursor_rank.is_some() {
                bindings.push((":crank", &crank));
                bindings.push((":crowid", &cursor_rowid));
            }
            let rows: Vec<(i64, f64, serde_json::Value)> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |row| {
                        let rank: f64 = row.get(7)?;
                        Ok((
                            row.get(0)?,
                            rank,
                            serde_json::json!({
                                "rowid": row.get::<_, i64>(0)?,
                                "corpus": row.get::<_, String>(1)?,
                                "chunk_id": row.get::<_, i64>(2)?,
                                "sha256": row.get::<_, Option<String>>(3)?,
                                "filename": row.get::<_, String>(4)?,
                                "section": row.get::<_, Option<String>>(5)?,
                                "snippet": row.get::<_, String>(6)?,
                                "rank": rank,
                            }),
                        ))
                    },
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last()
                    .map(|(rowid, rank, _)| format!("{:x}:{}", rank.to_bits(), rowid))
            } else {
                None
            };
            let data: Vec<serde_json::Value> = rows.into_iter().map(|(_, _, v)| v).collect();
            Ok(
                serde_json::json!({"data": data, "meta": meta(limit, None).tap_mut(|m| {
                    m["next_cursor"] = serde_json::json!(next_cursor);
                    m["ordering"] = serde_json::json!("bm25 rank ASC, rowid ASC (keyset)");
                })}),
            )
        })
        .map(Json)
}

async fn library_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let meta_rows: Vec<serde_json::Value> = conn
                .prepare(
                    "SELECT source, source_path, source_mtime, imported_at, row_counts
                 FROM import_meta ORDER BY id",
                )?
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "source": row.get::<_, String>(0)?,
                        "source_path": row.get::<_, String>(1)?,
                        "source_mtime": row.get::<_, Option<String>>(2)?,
                        "imported_at": row.get::<_, String>(3)?,
                        "row_counts": serde_json::from_str::<serde_json::Value>(
                            &row.get::<_, String>(4)?).unwrap_or(serde_json::Value::Null),
                    }))
                })?
                .collect::<Result<_, _>>()?;
            let papers: i64 = conn.query_row("SELECT COUNT(*) FROM paper", [], |r| r.get(0))?;
            let enrichments: i64 =
                conn.query_row("SELECT COUNT(*) FROM paper_enrichment", [], |r| r.get(0))?;
            let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunk", [], |r| r.get(0))?;
            let quarantined: i64 =
                conn.query_row("SELECT COUNT(*) FROM chunk_quarantine", [], |r| r.get(0))?;
            let mut corpus_stmt =
                conn.prepare("SELECT corpus, COUNT(*) FROM chunk GROUP BY corpus ORDER BY corpus")?;
            let corpora: Vec<serde_json::Value> = corpus_stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "corpus": row.get::<_, String>(0)?,
                        "chunks": row.get::<_, i64>(1)?,
                    }))
                })?
                .collect::<Result<_, _>>()?;
            Ok(serde_json::json!({
                "imports": meta_rows,
                "counts": {
                    "papers": papers,
                    "enrichments": enrichments,
                    "chunks": chunks,
                    "quarantined_chunks": quarantined,
                },
                "corpora": corpora,
            }))
        })
        .map(Json)
}

/// Small helper to mutate a json! value inline (kept local to search meta).
trait TapMut {
    fn tap_mut(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl TapMut for serde_json::Value {
    fn tap_mut(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

// ---------- semantic endpoint ----------

/// BM25 top-k by rowid (no cursor; used as the sparse leg of hybrid fusion).
fn bm25_topk(
    conn: &rusqlite::Connection,
    query: &str,
    corpus: Option<&str>,
    k: usize,
) -> Result<Vec<(i64, f64)>, stacks_core::StoreError> {
    let mut sql = String::from(
        "SELECT c.rowid, bm25(chunk_fts) FROM chunk_fts JOIN chunk c ON c.rowid = chunk_fts.rowid
         WHERE chunk_fts MATCH :q",
    );
    if corpus.is_some() {
        sql.push_str(" AND c.corpus = :corpus");
    }
    sql.push_str(" ORDER BY 2, 1 LIMIT :k");
    let mut stmt = conn.prepare(&sql)?;
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
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn hydrate_hits(
    conn: &rusqlite::Connection,
    hits: &[(i64, f64)],
) -> Result<Vec<serde_json::Value>, stacks_core::StoreError> {
    let mut stmt = conn.prepare(
        "SELECT rowid, corpus, chunk_id, sha256, filename, section, substr(text, 1, 400)
         FROM chunk WHERE rowid = ?1",
    )?;
    let mut out = Vec::with_capacity(hits.len());
    for (rowid, score) in hits {
        let v = stmt.query_row([*rowid], |row| {
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
        })?;
        out.push(v);
    }
    Ok(out)
}

async fn semantic_chunks(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(&raw, &["query", "corpus", "k", "mode"])?;
    let query = raw
        .get("query")
        .filter(|q| !q.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            ApiError::invalid_query(
                "query is required for /chunks/semantic and must be non-empty",
                None,
            )
        })?;
    let k = match raw.get("k") {
        None => 20u32,
        Some(v) => {
            let n: u32 = v.parse().map_err(|_| {
                ApiError::invalid_query(format!("k must be an integer, got {v:?}"), None)
            })?;
            if n == 0 || n > 50 {
                return Err(ApiError::invalid_query(
                    format!("k must be within 1..=50, got {n}"),
                    Some(serde_json::json!({"max": 50})),
                ));
            }
            n
        }
    };
    let mode = match raw.get("mode").map(String::as_str) {
        None | Some("dense") => "dense",
        Some("hybrid") => "hybrid",
        Some(v) => {
            return Err(ApiError::invalid_query(
                format!("unknown mode {v:?}"),
                Some(serde_json::json!({"allowed": ["dense", "hybrid"]})),
            ))
        }
    };
    let corpus = raw.get("corpus").filter(|s| !s.trim().is_empty()).cloned();

    let Some(sem) = &state.semantic else {
        return Err(ApiError::not_found(
            "semantic search not mounted on this instance",
        ));
    };
    let query_vec = {
        let mut embedder = sem.embedder.lock().map_err(|_| ApiError::internal())?;
        embedder.embed(&query).map_err(|_| ApiError::internal())?
    };
    let library = state.library.clone();
    let sem = sem.clone();
    tokio::task::spawn_blocking(move || {
        let Some(lib) = &library else {
            return Err(ApiError::not_found(
                "library database not mounted on this instance",
            ));
        };
        let lib = lib.lock().map_err(|_| ApiError::internal())?;
        let mut cache = sem.cache.lock().map_err(|_| ApiError::internal())?;
        let conn = lib.raw();
        let dense =
            semantic::dense_search(&lib, &mut cache, corpus.as_deref(), &query_vec, k as usize)
                .map_err(|_| ApiError::internal())?;
        let (hits, sparse_count) = if mode == "hybrid" {
            let sparse = bm25_topk(conn, &query, corpus.as_deref(), k as usize)
                .map_err(|_| ApiError::internal())?;
            let n = sparse.len();
            (semantic::rrf_fuse(&dense, &sparse, 60, k as usize), n)
        } else {
            (dense.iter().map(|(r, s)| (*r, *s as f64)).collect(), 0usize)
        };
        let data = hydrate_hits(conn, &hits).map_err(|_| ApiError::internal())?;
        Ok(Json(serde_json::json!({
            "data": data,
            "meta": {
                "k": k,
                "max_k": 50,
                "mode": mode,
                "corpus": corpus,
                "dense_candidates": dense.len(),
                "sparse_candidates": sparse_count,
                "fusion": if mode == "hybrid" { "rrf(k=60)" } else { "none" },
                "vector_cache_bytes": cache.cached_bytes(),
            }
        })))
    })
    .await
    .map_err(|_| ApiError::internal())?
}

// ---------- document endpoints ----------

const DOC_COLS: &str = "sha256, family, kind, title, authors, year, language, pages,
    source_url, download_url, path, location_root, bytes, retrieved_at, text_layer_path,
    collection, original_language, transliterated_title, soviet_stratum";

fn doc_from_row(row: &rusqlite::Row, offset: usize) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "sha256": row.get::<_, String>(offset)?,
        "family": row.get::<_, String>(offset + 1)?,
        "kind": row.get::<_, String>(offset + 2)?,
        "title": row.get::<_, Option<String>>(offset + 3)?,
        "authors": row.get::<_, Option<String>>(offset + 4)?,
        "year": row.get::<_, Option<i64>>(offset + 5)?,
        "language": row.get::<_, Option<String>>(offset + 6)?,
        "pages": row.get::<_, Option<i64>>(offset + 7)?,
        "source_url": row.get::<_, Option<String>>(offset + 8)?,
        "download_url": row.get::<_, Option<String>>(offset + 9)?,
        "path": row.get::<_, String>(offset + 10)?,
        "location_root": row.get::<_, String>(offset + 11)?,
        "bytes": row.get::<_, Option<i64>>(offset + 12)?,
        "retrieved_at": row.get::<_, Option<String>>(offset + 13)?,
        "text_layer_path": row.get::<_, Option<String>>(offset + 14)?,
        "collection": row.get::<_, Option<String>>(offset + 15)?,
        "original_language": row.get::<_, Option<String>>(offset + 16)?,
        "transliterated_title": row.get::<_, Option<String>>(offset + 17)?,
        "soviet_stratum": row.get::<_, Option<String>>(offset + 18)?,
    }))
}

async fn list_documents(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(
        &raw,
        &[
            "query",
            "family",
            "kind",
            "language",
            "collection",
            "limit",
            "cursor",
        ],
    )?;
    let collection = raw
        .get("collection")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    let limit = parse_limit(&raw)?;
    let cursor = parse_cursor(&raw)?;
    if let Some(kind) = raw.get("kind") {
        let closed = [
            "book",
            "report",
            "patent",
            "thesis",
            "paper",
            "dataset-paper",
            "archive",
            "article",
        ];
        if !closed.contains(&kind.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown document kind {kind:?}"),
                Some(serde_json::json!({"allowed": closed})),
            ));
        }
    }
    let query = raw.get("query").filter(|q| !q.trim().is_empty()).cloned();
    let family = raw.get("family").filter(|s| !s.trim().is_empty()).cloned();
    let kind = raw.get("kind").cloned();
    let language = raw
        .get("language")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let mut sql = format!("SELECT rowid, {DOC_COLS} FROM document WHERE rowid > :cursor");
            if query.is_some() {
                sql.push_str(" AND (title LIKE :q OR authors LIKE :q OR path LIKE :q)");
            }
            if family.is_some() {
                sql.push_str(" AND family = :family");
            }
            if kind.is_some() {
                sql.push_str(" AND kind = :kind");
            }
            if language.is_some() {
                sql.push_str(" AND language = :language");
            }
            if collection.is_some() {
                sql.push_str(" AND collection = :collection");
            }
            sql.push_str(" ORDER BY rowid LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &cursor), (":limit", &limit_plus_one)];
            let like;
            if let Some(q) = &query {
                like = format!("%{q}%");
                bindings.push((":q", &like));
            }
            if let Some(f) = &family {
                bindings.push((":family", f));
            }
            if let Some(c) = &collection {
                bindings.push((":collection", c));
            }
            if let Some(k) = &kind {
                bindings.push((":kind", k));
            }
            if let Some(l) = &language {
                bindings.push((":language", l));
            }
            let rows: Vec<(i64, serde_json::Value)> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |row| Ok((row.get(0)?, doc_from_row(row, 1)?)),
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last().map(|(id, _)| *id)
            } else {
                None
            };
            let data: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(rowid, mut v)| {
                    v["rowid"] = serde_json::json!(rowid);
                    v
                })
                .collect();
            Ok(serde_json::json!({"data": data, "meta": meta(limit, next_cursor)}))
        })
        .map(Json)
}

async fn get_document(
    State(state): State<AppState>,
    Path(sha256): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let doc = state.with_library(|lib| {
        lib.raw()
            .query_row(
                &format!("SELECT {DOC_COLS} FROM document WHERE sha256 = ?1"),
                [&sha256],
                |row| doc_from_row(row, 0),
            )
            .optional()
            .map_err(stacks_core::StoreError::from)
    })?;
    doc.map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no document with sha256 {sha256}")))
}

// ---------- materials-pillar endpoints ----------

async fn list_material_entries(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(
        &raw,
        &[
            "query",
            "elements",
            "source",
            "band_gap_min",
            "band_gap_max",
            "limit",
            "cursor",
        ],
    )?;
    let limit = parse_limit(&raw)?;
    let cursor = parse_cursor(&raw)?;
    if let Some(src) = raw.get("source") {
        if !["mp", "cod", "oqmd", "top"].contains(&src.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown source {src:?}"),
                Some(serde_json::json!({"allowed": ["mp", "cod", "oqmd", "top"]})),
            ));
        }
    }
    let query = raw.get("query").filter(|q| !q.trim().is_empty()).cloned();
    let elements = raw
        .get("elements")
        .filter(|s| !s.trim().is_empty())
        .cloned();
    let source = raw.get("source").cloned();
    let bg_min = match raw.get("band_gap_min") {
        None => None,
        Some(v) => Some(v.parse::<f64>().map_err(|_| {
            ApiError::invalid_query(format!("band_gap_min must be numeric, got {v:?}"), None)
        })?),
    };
    let bg_max = match raw.get("band_gap_max") {
        None => None,
        Some(v) => Some(v.parse::<f64>().map_err(|_| {
            ApiError::invalid_query(format!("band_gap_max must be numeric, got {v:?}"), None)
        })?),
    };
    state
        .with_materials(|m| {
            let conn = m.raw();
            let has_bg_filter = bg_min.is_some() || bg_max.is_some();
            let mut sql = String::from(
                "SELECT DISTINCT e.rowid, e.external_key, e.source, e.source_id, e.formula, e.elements,
                    e.nsites, e.spacegroup, e.spacegroup_number, e.crystal_system, e.density
             FROM material_entry e",
            );
            if has_bg_filter {
                sql.push_str(
                    " JOIN property p ON p.external_key = e.external_key AND p.kind = 'band_gap'",
                );
            }
            sql.push_str(" WHERE e.rowid > :cursor");
            if query.is_some() {
                sql.push_str(" AND e.formula LIKE :q");
            }
            if elements.is_some() {
                sql.push_str(" AND e.elements LIKE :elements");
            }
            if source.is_some() {
                sql.push_str(" AND e.source = :source");
            }
            if let Some(x) = bg_min {
                let _ = x;
                sql.push_str(" AND p.value >= :bg_min");
            }
            if bg_max.is_some() {
                sql.push_str(" AND p.value <= :bg_max");
            }
            sql.push_str(" ORDER BY e.rowid LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &cursor), (":limit", &limit_plus_one)];
            let like;
            if let Some(q) = &query {
                like = format!("%{q}%");
                bindings.push((":q", &like));
            }
            if let Some(e) = &elements {
                bindings.push((":elements", e));
            }
            if let Some(s) = &source {
                bindings.push((":source", s));
            }
            if let Some(x) = &bg_min {
                bindings.push((":bg_min", x));
            }
            if let Some(x) = &bg_max {
                bindings.push((":bg_max", x));
            }
            let rows: Vec<(i64, serde_json::Value)> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            serde_json::json!({
                                "external_key": row.get::<_, String>(1)?,
                                "source": row.get::<_, String>(2)?,
                                "source_id": row.get::<_, String>(3)?,
                                "formula": row.get::<_, Option<String>>(4)?,
                                "elements": row.get::<_, Option<String>>(5)?,
                                "nsites": row.get::<_, Option<i64>>(6)?,
                                "spacegroup": row.get::<_, Option<String>>(7)?,
                                "spacegroup_number": row.get::<_, Option<i64>>(8)?,
                                "crystal_system": row.get::<_, Option<String>>(9)?,
                                "density": row.get::<_, Option<f64>>(10)?,
                            }),
                        ))
                    },
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last().map(|(id, _)| *id)
            } else {
                None
            };
            let data: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(rowid, mut v)| {
                    v["rowid"] = serde_json::json!(rowid);
                    v
                })
                .collect();
            Ok(serde_json::json!({"data": data, "meta": meta(limit, next_cursor)}))
        })
        .map(Json)
}

async fn get_material_entry(
    State(state): State<AppState>,
    Path(external_key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let doc = state.with_materials(|m| {
        let conn = m.raw();
        let entry = conn
            .query_row(
                "SELECT external_key, source, source_id, formula, elements, nsites, spacegroup,
                        spacegroup_number, crystal_system, cell_json, structure_json, density,
                        description, reference_path, imported_at
                 FROM material_entry WHERE external_key = ?1",
                [&external_key],
                |row| {
                    Ok(serde_json::json!({
                        "external_key": row.get::<_, String>(0)?,
                        "source": row.get::<_, String>(1)?,
                        "source_id": row.get::<_, String>(2)?,
                        "formula": row.get::<_, Option<String>>(3)?,
                        "elements": row.get::<_, Option<String>>(4)?,
                        "nsites": row.get::<_, Option<i64>>(5)?,
                        "spacegroup": row.get::<_, Option<String>>(6)?,
                        "spacegroup_number": row.get::<_, Option<i64>>(7)?,
                        "crystal_system": row.get::<_, Option<String>>(8)?,
                        "cell": row.get::<_, Option<String>>(9)?
                            .map(|s| serde_json::from_str::<serde_json::Value>(&s).unwrap_or_default()),
                        "structure": row.get::<_, Option<String>>(10)?
                            .map(|s| serde_json::from_str::<serde_json::Value>(&s).unwrap_or_default()),
                        "density": row.get::<_, Option<f64>>(11)?,
                        "description": row.get::<_, Option<String>>(12)?,
                        "reference_path": row.get::<_, Option<String>>(13)?,
                        "imported_at": row.get::<_, String>(14)?,
                    }))
                },
            )
            .optional()
            .map_err(stacks_core::StoreError::from)?;
        let Some(entry) = entry else { return Ok(None) };
        let mut stmt = conn.prepare(
            "SELECT collection, kind, value, unit, text_value FROM property
             WHERE external_key = ?1 ORDER BY collection, kind",
        )?;
        let props: Vec<serde_json::Value> = stmt
            .query_map([&external_key], |row| {
                Ok(serde_json::json!({
                    "collection": row.get::<_, String>(0)?,
                    "kind": row.get::<_, String>(1)?,
                    "value": row.get::<_, Option<f64>>(2)?,
                    "unit": row.get::<_, Option<String>>(3)?,
                    "text_value": row.get::<_, Option<String>>(4)?,
                }))
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some(serde_json::json!({"entry": entry, "properties": props})))
    })?;
    doc.map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no material entry {external_key}")))
}

async fn materials_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.with_materials(|m| {
        let conn = m.raw();
        let entries: i64 = conn.query_row("SELECT COUNT(*) FROM material_entry", [], |r| r.get(0))?;
        let props: i64 = conn.query_row("SELECT COUNT(*) FROM property", [], |r| r.get(0))?;
        let quarantined: i64 =
            conn.query_row("SELECT COUNT(*) FROM material_quarantine", [], |r| r.get(0))?;
        let mut stmt = conn.prepare(
            "SELECT source, COUNT(*), COUNT(formula) FROM material_entry GROUP BY source ORDER BY source",
        )?;
        let by_source: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "source": row.get::<_, String>(0)?,
                    "entries": row.get::<_, i64>(1)?,
                    "with_formula": row.get::<_, i64>(2)?,
                }))
            })?
            .collect::<Result<_, _>>()?;
        let mut kstmt = conn.prepare(
            "SELECT kind, COUNT(*), unit FROM property GROUP BY kind, unit ORDER BY 2 DESC",
        )?;
        let kinds: Vec<serde_json::Value> = kstmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "kind": row.get::<_, String>(0)?,
                    "count": row.get::<_, i64>(1)?,
                    "unit": row.get::<_, Option<String>>(2)?,
                }))
            })?
            .collect::<Result<_, _>>()?;
        let imports: Vec<serde_json::Value> = conn
            .prepare("SELECT source, source_path, source_mtime, imported_at FROM import_meta ORDER BY id")?
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "source": row.get::<_, String>(0)?,
                    "source_path": row.get::<_, String>(1)?,
                    "source_mtime": row.get::<_, Option<String>>(2)?,
                    "imported_at": row.get::<_, String>(3)?,
                }))
            })?
            .collect::<Result<_, _>>()?;
        Ok(serde_json::json!({
            "counts": {"entries": entries, "properties": props, "quarantined": quarantined},
            "by_source": by_source,
            "property_kinds": kinds,
            "imports": imports,
        }))
    })
    .map(Json)
}

async fn documents_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.with_library(|lib| {
        let conn = lib.raw();
        let count = |q: &str| -> Result<Vec<serde_json::Value>, stacks_core::StoreError> {
            let mut stmt = conn.prepare(q)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(serde_json::json!({
                        "key": r.get::<_, Option<String>>(0)?,
                        "count": r.get::<_, i64>(1)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        };
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM document", [], |r| r.get(0))?;
        let triage: i64 = conn.query_row("SELECT COUNT(*) FROM intake_triage", [], |r| r.get(0))?;
        Ok(serde_json::json!({
            "documents": total,
            "by_family": count("SELECT family, COUNT(*) FROM document GROUP BY family ORDER BY 2 DESC")?,
            "by_kind": count("SELECT kind, COUNT(*) FROM document GROUP BY kind ORDER BY 2 DESC")?,
            "by_collection": count("SELECT collection, COUNT(*) FROM document WHERE collection IS NOT NULL GROUP BY collection ORDER BY 2 DESC")?,
            "triage_decisions": count("SELECT decision, COUNT(*) FROM intake_triage GROUP BY decision ORDER BY 2 DESC")?,
            "triage_rows": triage,
        }))
    })
    .map(Json)
}

// ---------- dataset registry endpoints ----------

const DATASET_COLS: &str = "id, name, path, location_root, size_bytes, file_count,
    dominant_formats, sha256_status, description, domains, status, status_note, registered_at";

fn dataset_from_row(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "name": row.get::<_, String>(1)?,
        "path": row.get::<_, String>(2)?,
        "location_root": row.get::<_, String>(3)?,
        "size_bytes": row.get::<_, Option<i64>>(4)?,
        "file_count": row.get::<_, Option<i64>>(5)?,
        "dominant_formats": row.get::<_, Option<String>>(6)?
            .map(|s| serde_json::from_str::<serde_json::Value>(&s).unwrap_or_default()),
        "sha256_status": row.get::<_, String>(7)?,
        "description": row.get::<_, String>(8)?,
        "domains": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(9)?)
            .unwrap_or_default(),
        "status": row.get::<_, String>(10)?,
        "status_note": row.get::<_, Option<String>>(11)?,
        "registered_at": row.get::<_, String>(12)?,
    }))
}

async fn list_datasets(
    State(state): State<AppState>,
    Query(raw): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    reject_unknown_params(&raw, &["query", "domain", "status", "limit", "cursor"])?;
    let limit = parse_limit(&raw)?;
    let cursor = parse_cursor(&raw)?;
    if let Some(status) = raw.get("status") {
        let closed = [
            "registered",
            "migrated",
            "preserved_original",
            "queryable",
            "extraction_queue",
            "missing",
        ];
        if !closed.contains(&status.as_str()) {
            return Err(ApiError::invalid_query(
                format!("unknown dataset status {status:?}"),
                Some(serde_json::json!({"allowed": closed})),
            ));
        }
    }
    let query = raw.get("query").filter(|q| !q.trim().is_empty()).cloned();
    let domain = raw.get("domain").filter(|s| !s.trim().is_empty()).cloned();
    let status = raw.get("status").cloned();
    state
        .with_library(|lib| {
            let conn = lib.raw();
            let mut sql = format!("SELECT {DATASET_COLS} FROM dataset WHERE id > :cursor");
            if query.is_some() {
                sql.push_str(" AND (name LIKE :q OR description LIKE :q OR path LIKE :q)");
            }
            if domain.is_some() {
                sql.push_str(" AND domains LIKE :domain");
            }
            if status.is_some() {
                sql.push_str(" AND status = :status");
            }
            sql.push_str(" ORDER BY id LIMIT :limit");
            let mut stmt = conn.prepare(&sql)?;
            let limit_plus_one = limit as i64 + 1;
            let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
                vec![(":cursor", &cursor), (":limit", &limit_plus_one)];
            let like;
            if let Some(q) = &query {
                like = format!("%{q}%");
                bindings.push((":q", &like));
            }
            let dom;
            if let Some(d) = &domain {
                dom = format!("%\"{d}\"%");
                bindings.push((":domain", &dom));
            }
            if let Some(s) = &status {
                bindings.push((":status", s));
            }
            let rows: Vec<serde_json::Value> = stmt
                .query_map(
                    bindings
                        .iter()
                        .map(|(n, v)| (*n, *v))
                        .collect::<Vec<_>>()
                        .as_slice(),
                    dataset_from_row,
                )?
                .collect::<Result<_, _>>()?;
            let mut rows = rows;
            let next_cursor = if rows.len() as u32 > limit {
                rows.truncate(limit as usize);
                rows.last().and_then(|r| r["id"].as_i64())
            } else {
                None
            };
            Ok(serde_json::json!({"data": rows, "meta": meta(limit, next_cursor)}))
        })
        .map(Json)
}

async fn get_dataset(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let doc = state.with_library(|lib| {
        let conn = lib.raw();
        let by_id: Option<serde_json::Value> = if let Ok(n) = id.parse::<i64>() {
            conn.query_row(
                &format!("SELECT {DATASET_COLS} FROM dataset WHERE id = ?1"),
                [n],
                dataset_from_row,
            )
            .optional()
            .map_err(stacks_core::StoreError::from)?
        } else {
            None
        };
        let by_name = match by_id {
            Some(v) => Some(v),
            None => conn
                .query_row(
                    &format!("SELECT {DATASET_COLS} FROM dataset WHERE name = ?1"),
                    [&id],
                    dataset_from_row,
                )
                .optional()
                .map_err(stacks_core::StoreError::from)?,
        };
        Ok(by_name)
    })?;
    doc.map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no dataset {id}")))
}

async fn datasets_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.with_library(|lib| {
        let conn = lib.raw();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM dataset", [], |r| r.get(0))?;
        let bytes: Option<i64> =
            conn.query_row("SELECT SUM(size_bytes) FROM dataset", [], |r| r.get(0))?;
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM dataset GROUP BY status ORDER BY 2 DESC",
        )?;
        let by_status: Vec<serde_json::Value> = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({"status": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)?}))
            })?
            .collect::<Result<_, _>>()?;
        Ok(serde_json::json!({
            "datasets": total,
            "total_bytes": bytes,
            "by_status": by_status,
        }))
    })
    .map(Json)
}
