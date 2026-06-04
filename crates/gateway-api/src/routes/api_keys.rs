//! API key management routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{generate_api_key, AuthContext};
use gateway_db::{repos::api_key_repo::ApiKeyRepo, ApiKey as DbApiKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiKeyItem {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub rate_limit_rps: i32,
    pub status: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeysListResponse {
    pub object: String,
    pub data: Vec<ApiKeyItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Option<Vec<String>>,
    pub rate_limit_rps: Option<i32>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub rate_limit_rps: i32,
    pub status: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub status: Option<String>,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_api_keys(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<ApiKeysListResponse>, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let keys = repo
        .list_by_org(auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(ApiKeysListResponse {
        object: "list".to_string(),
        data: keys.iter().map(|k| db_to_item(k)).collect(),
    }))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let (key_plain, key_hash, key_prefix) = generate_api_key();
    let scopes = body.scopes.unwrap_or_else(|| vec!["all".to_string()]);
    let rate_limit_rps = body.rate_limit_rps.unwrap_or(10);
    let expires_at = body
        .expires_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let key = repo
        .create(
            auth.org_id,
            None,
            &body.name,
            &key_hash,
            &key_prefix,
            scopes.clone(),
            rate_limit_rps,
            expires_at,
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(CreateApiKeyResponse {
        id: key.id.to_string(),
        name: key.name,
        key: key_plain,
        prefix: key.key_prefix,
        scopes,
        rate_limit_rps,
        status: key.status,
        expires_at: key.expires_at.map(|t| t.to_rfc3339()),
        created_at: key.created_at.to_rfc3339(),
    }))
}

pub async fn update_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(key_id): Path<String>,
    Json(body): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyItem>, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let key_uuid = Uuid::parse_str(&key_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_key_id", "Invalid API key ID"))?;

    repo
        .update(auth.org_id, key_uuid, body.name.as_deref(), body.status.as_deref())
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let key = repo
        .get_by_id(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "key_not_found", "API key not found"))?;

    Ok(Json(db_to_item(&key)))
}

pub async fn delete_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let key_uuid = Uuid::parse_str(&key_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_key_id", "Invalid API key ID"))?;

    repo
        .delete(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_to_item(key: &DbApiKey) -> ApiKeyItem {
    ApiKeyItem {
        id: key.id.to_string(),
        name: key.name.clone(),
        prefix: key.key_prefix.clone(),
        scopes: key.scopes.0.clone(),
        rate_limit_rps: key.rate_limit_rps,
        status: key.status.clone(),
        expires_at: key.expires_at.map(|t| t.to_rfc3339()),
        last_used_at: key.last_used_at.map(|t| t.to_rfc3339()),
        created_at: key.created_at.to_rfc3339(),
    }
}
