//! Quota admin routes — CRUD for quota rules.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{AuthContext, rbac::{check_permission, Permission, Role}};
use gateway_db::{
    models::AuditAction,
    repos::quota_repo::QuotaRepo,
};
use rust_decimal::Decimal;
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

#[derive(Debug, Deserialize, Validate)]
pub struct CreateQuotaRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    #[validate(length(max = 512, message = "Description must be at most 512 characters"))]
    pub description: Option<String>,
    #[validate(length(min = 1, max = 64, message = "Metric must be 1-64 characters"))]
    pub metric: String,
    #[validate(length(min = 1, max = 32, message = "Period must be 1-32 characters"))]
    pub period: String,
    pub limit_value: Decimal,
    #[serde(default)]
    pub warning_threshold: Decimal,
    #[validate(length(min = 1, max = 64, message = "Applies-to must be 1-64 characters"))]
    pub applies_to: String,
    #[serde(default)]
    pub scope_filter: serde_json::Value,
    #[validate(length(min = 1, max = 64, message = "Action must be 1-64 characters"))]
    pub action: String,
    #[serde(default = "default_active")]
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
    pub status: String,
    pub api_key_id: Option<Uuid>,
}

fn default_active() -> String {
    "active".to_string()
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateQuotaRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "Description must be at most 512 characters"))]
    pub description: Option<String>,
    #[validate(length(min = 1, max = 64, message = "Metric must be 1-64 characters"))]
    pub metric: Option<String>,
    #[validate(length(min = 1, max = 32, message = "Period must be 1-32 characters"))]
    pub period: Option<String>,
    pub limit_value: Option<Decimal>,
    pub warning_threshold: Option<Decimal>,
    #[validate(length(min = 1, max = 64, message = "Applies-to must be 1-64 characters"))]
    pub applies_to: Option<String>,
    pub scope_filter: Option<serde_json::Value>,
    #[validate(length(min = 1, max = 64, message = "Action must be 1-64 characters"))]
    pub action: Option<String>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
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

    let repo = QuotaRepo::new(state.db_pool.clone());
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
    Extension(ctx): Extension<AuditRequestContext>,
    Path(org_id): Path<Uuid>,
    ValidatedJson(body): ValidatedJson<CreateQuotaRequest>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasWrite)?;

    let repo = QuotaRepo::new(state.db_pool.clone());
    let name = sanitize_display_text(&body.name);
    let description = body.description.as_deref().map(sanitize_display_text);
    let metric = sanitize_display_text(&body.metric);
    let period = sanitize_display_text(&body.period);
    let applies_to = sanitize_display_text(&body.applies_to);
    let action = sanitize_display_text(&body.action);
    let status = sanitize_display_text(&body.status);
    let quota = repo
        .create(
            org_id,
            body.api_key_id,
            &name,
            description.as_deref(),
            &metric,
            &period,
            body.limit_value,
            body.warning_threshold,
            &applies_to,
            body.scope_filter,
            &action,
            &status,
        )
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::Create,
        "quota",
        Some(&quota.id.to_string()),
        None,
        Some(json!({
            "name": quota.name,
            "metric": quota.metric,
            "period": quota.period,
            "limit_value": quota.limit_value,
            "applies_to": quota.applies_to,
            "status": quota.status,
        })),
        "Quota created",
    )
    .await;

    Ok(Json(QuotaResponse::from(quota)))
}

pub async fn get_quota(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasRead)?;

    let repo = QuotaRepo::new(state.db_pool.clone());
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
    Extension(ctx): Extension<AuditRequestContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
    ValidatedJson(body): ValidatedJson<UpdateQuotaRequest>,
) -> Result<Json<QuotaResponse>, ApiError> {
    require_permission(&auth, Permission::QuotasWrite)?;

    let repo = QuotaRepo::new(state.db_pool.clone());
    let existing = repo
        .get_by_id(org_id, quota_id)
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?
        .ok_or_else(|| ApiError::new(
            StatusCode::NOT_FOUND,
            "quota_not_found",
            format!("Quota {} not found", quota_id),
        ))?;

    let name = body.name.as_deref().map(sanitize_display_text);
    let description = body.description.as_deref().map(sanitize_display_text);
    let metric = body.metric.as_deref().map(sanitize_display_text);
    let period = body.period.as_deref().map(sanitize_display_text);
    let applies_to = body.applies_to.as_deref().map(sanitize_display_text);
    let action = body.action.as_deref().map(sanitize_display_text);
    let status = body.status.as_deref().map(sanitize_display_text);
    let quota = repo
        .update(
            org_id,
            quota_id,
            name.as_deref(),
            description.as_deref().map(Some).or(Some(None)),
            metric.as_deref(),
            period.as_deref(),
            body.limit_value,
            body.warning_threshold,
            applies_to.as_deref(),
            body.scope_filter,
            action.as_deref(),
            status.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    match quota {
        Some(ref q) => {
            audit::record(
                &state,
                &auth,
                &ctx,
                AuditAction::Update,
                "quota",
                Some(&q.id.to_string()),
                Some(json!({
                    "name": existing.name,
                    "metric": existing.metric,
                    "period": existing.period,
                    "limit_value": existing.limit_value,
                    "applies_to": existing.applies_to,
                    "status": existing.status,
                })),
                Some(json!({
                    "name": q.name,
                    "metric": q.metric,
                    "period": q.period,
                    "limit_value": q.limit_value,
                    "applies_to": q.applies_to,
                    "status": q.status,
                })),
                "Quota updated",
            )
            .await;
            Ok(Json(QuotaResponse::from(q.clone())))
        }
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
    Extension(ctx): Extension<AuditRequestContext>,
    Path((org_id, quota_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission(&auth, Permission::QuotasDelete)?;

    let repo = QuotaRepo::new(state.db_pool.clone());
    let existing = repo
        .get_by_id(org_id, quota_id)
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    let deleted = repo
        .delete(org_id, quota_id)
        .await
        .map_err(|e| ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    if deleted {
        if let Some(q) = existing {
            audit::record(
                &state,
                &auth,
                &ctx,
                AuditAction::Delete,
                "quota",
                Some(&q.id.to_string()),
                Some(json!({
                    "name": q.name,
                    "metric": q.metric,
                    "period": q.period,
                    "limit_value": q.limit_value,
                    "applies_to": q.applies_to,
                    "status": q.status,
                })),
                None,
                "Quota deleted",
            )
            .await;
        }
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "quota_not_found",
            format!("Quota {} not found", quota_id),
        ))
    }
}
