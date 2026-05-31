//! Quota admin routes — CRUD for quota rules.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{AuthContext, rbac::{check_permission, Permission, Role}};
use gateway_db::repos::quota_repo::QuotaRepo;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Require a specific permission; return 403 if not authorized.
fn require_permission(auth: &AuthContext, permission: Permission) -> Result<(), ApiError> {
    let role = auth
        .role
        .as_deref()
        .and_then(Role::from_str)
        .unwrap_or(Role::Viewer);

    if !check_permission(role, permission) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            format!("Role '{:?}' does not have permission '{:?}'", role, permission),
        ));
    }
    Ok(())
}

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateQuotaRequest {
    pub name: String,
    pub description: Option<String>,
    pub metric: String,
    pub period: String,
    pub limit_value: Decimal,
    #[serde(default)]
    pub warning_threshold: Decimal,
    pub applies_to: String,
    #[serde(default)]
    pub scope_filter: serde_json::Value,
    pub action: String,
    #[serde(default = "default_active")]
    pub status: String,
    pub api_key_id: Option<Uuid>,
}

fn default_active() -> String {
    "active".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuotaRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub metric: Option<String>,
    pub period: Option<String>,
    pub limit_value: Option<Decimal>,
    pub warning_threshold: Option<Decimal>,
    pub applies_to: Option<String>,
    pub scope_filter: Option<serde_json::Value>,
    pub action: Option<String>,
    pub status: Option<String>,
    pub api_key_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct QuotaResponse {
    pub id: Uuid,
    pub org_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub metric: String,
    pub period: String,
    pub limit_value: Decimal,
    pub warning_threshold: Decimal,
    pub applies_to: String,
    pub scope_filter: serde_json::Value,
    pub action: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<gateway_db::models::Quota> for QuotaResponse {
    fn from(q: gateway_db::models::Quota) -> Self {
        Self {
            id: q.id,
            org_id: q.org_id,
            api_key_id: q.api_key_id,
            name: q.name,
            description: q.description,
            metric: q.metric,
            period: q.period,
            limit_value: q.limit_value.into(),
            warning_threshold: q.warning_threshold.into(),
            applies_to: q.applies_to,
            scope_filter: q.scope_filter,
            action: q.action,
            status: q.status,
            created_at: q.created_at,
            updated_at: q.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListQuotasResponse {
    pub data: Vec<QuotaResponse>,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_quotas(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<ListQuotasResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasRead)?;

    let repo = QuotaRepo::new(state.db_pool);
    let quotas = repo.list_by_org(org_id).await.map_err(|e| ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database_error",
        e.to_string(),
    ))?;

    Ok(Json(ListQuotasResponse {
        data: quotas.into_iter().map(QuotaResponse::from).collect(),
    }))
}

pub async fn create_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
    Json(body): Json<CreateQuotaRequest>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasWrite)?;

    let repo = QuotaRepo::new(state.db_pool);
    let quota = repo
        .create(
            org_id,
            body.api_key_id,
            &body.name,
            body.description.as_deref(),
            &body.metric,
            &body.period,
            body.limit_value,
            body.warning_threshold,
            &body.applies_to,
            body.scope_filter,
            &body.action,
            &body.status,
        )
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    Ok(Json(QuotaResponse::from(quota)))
}

pub async fn get_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasRead)?;

    let repo = QuotaRepo::new(state.db_pool);
    let quota = repo
        .get_by_id(org_id, quota_id)
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    match quota {
        Some(q) => Ok(Json(QuotaResponse::from(q))),
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "quota_not_found",
            format!("Quota {} not found", quota_id),
        )),
    }
}

pub async fn update_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateQuotaRequest>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasWrite)?;

    let repo = QuotaRepo::new(state.db_pool);
    let quota = repo
        .update(
            org_id,
            quota_id,
            body.name.as_deref(),
            body.description.as_deref().map(Some).or(Some(None)),
            body.metric.as_deref(),
            body.period.as_deref(),
            body.limit_value,
            body.warning_threshold,
            body.applies_to.as_deref(),
            body.scope_filter,
            body.action.as_deref(),
            body.status.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    match quota {
        Some(q) => Ok(Json(QuotaResponse::from(q))),
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "quota_not_found",
            format!("Quota {} not found", quota_id),
        )),
    }
}

pub async fn delete_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission(&auth, Permission::QuotasDelete)?;

    let repo = QuotaRepo::new(state.db_pool);
    let deleted = repo
        .delete(org_id, quota_id)
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "quota_not_found",
            format!("Quota {} not found", quota_id),
        ))
    }
}
