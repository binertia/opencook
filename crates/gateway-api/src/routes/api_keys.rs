//! API key management routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{generate_api_key, AuthContext};
use gateway_db::{
    models::AuditAction,
    repos::api_key_repo::ApiKeyRepo,
    ApiKey as DbApiKey,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    state::AppState,
    validation::sanitize_display_text,
};
use validator::Validate;

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

#[derive(Debug, Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    pub scopes: Option<Vec<String>>,
    #[validate(range(min = 1, max = 10000, message = "Rate limit must be 1-10000 RPS"))]
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

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateApiKeyRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
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
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let (key_plain, key_hash, key_prefix) = generate_api_key();
    let scopes = body.scopes.unwrap_or_else(|| vec!["all".to_string()]);
    let rate_limit_rps = body.rate_limit_rps.unwrap_or(10);
    let expires_at = body
        .expires_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let name = sanitize_display_text(&body.name);
    let key = repo
        .create(
            auth.org_id,
            None,
            &name,
            &key_hash,
            &key_prefix,
            scopes.clone(),
            rate_limit_rps,
            expires_at,
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::ApiKeyCreated,
        "api_key",
        Some(&key.id.to_string()),
        None,
        Some(json!({
            "name": key.name,
            "prefix": key.key_prefix,
            "scopes": scopes,
            "rate_limit_rps": rate_limit_rps,
        })),
        "API key created",
    )
    .await;

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
    Extension(ctx): Extension<AuditRequestContext>,
    Path(key_id): Path<String>,
    ValidatedJson(body): ValidatedJson<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyItem>, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let key_uuid = Uuid::parse_str(&key_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_key_id", "Invalid API key ID"))?;

    let existing = repo
        .get_by_id(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "key_not_found", "API key not found"))?;

    let name = body.name.as_deref().map(sanitize_display_text);
    let name_ref = name.as_deref();
    repo
        .update(auth.org_id, key_uuid, name_ref, body.status.as_deref())
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let key = repo
        .get_by_id(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "key_not_found", "API key not found"))?;

    let old_values = json!({
        "name": existing.name,
        "status": existing.status,
    });
    let new_values = json!({
        "name": key.name,
        "status": key.status,
    });
    let action = if body.status.as_deref() == Some("revoked") || body.status.as_deref() == Some("inactive") {
        AuditAction::ApiKeyRevoked
    } else {
        AuditAction::Update
    };
    audit::record(
        &state,
        &auth,
        &ctx,
        action,
        "api_key",
        Some(&key.id.to_string()),
        Some(old_values),
        Some(new_values),
        if action == AuditAction::ApiKeyRevoked { "API key revoked" } else { "API key updated" },
    )
    .await;

    Ok(Json(db_to_item(&key)))
}

pub async fn delete_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());

    let key_uuid = Uuid::parse_str(&key_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_key_id", "Invalid API key ID"))?;

    let existing = repo
        .get_by_id(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "key_not_found", "API key not found"))?;

    repo
        .delete(auth.org_id, key_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::Delete,
        "api_key",
        Some(&existing.id.to_string()),
        Some(json!({
            "name": existing.name,
            "prefix": existing.key_prefix,
            "status": existing.status,
        })),
        None,
        "API key deleted",
    )
    .await;

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
