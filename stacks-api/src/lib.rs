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
use serde::Serialize;
use stacks_core::Store;

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
}

impl AppState {
    fn with<T>(
        &self,
        f: impl FnOnce(&Store) -> Result<T, stacks_core::StoreError>,
    ) -> Result<T, ApiError> {
        let store = self.store.lock().map_err(|_| ApiError::internal())?;
        f(&store).map_err(|_| ApiError::internal())
    }
}

/// Build the API router over an opened store. Listener construction lives in
/// `main` (isolated for the future tsnet swap); tests mount this directly.
pub fn build_app(store: Store) -> Router {
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1", get(api_doc))
        .route("/api/v1/schemas", get(schemas_index))
        .route("/api/v1/schemas/{name}", get(schema_by_name))
        .route("/api/v1/recipes", get(list_recipes))
        .route("/api/v1/recipes/{id}", get(get_recipe))
        .route("/api/v1/materials", get(list_materials))
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
            "GET /api/v1/materials": "search materials; params: query, kind, limit, cursor"
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
