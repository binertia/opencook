//! Quota management endpoints for SOLO mode.
//!
//! Users can configure their own usage limits without authentication.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use gateway_db::QuotaRepo;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

fn default_org() -> Uuid {
    Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid")
}

#[derive(Serialize)]
pub struct QuotaListResponse {
    pub data: Vec<gateway_db::models::Quota>,
}

#[derive(Deserialize)]
pub struct CreateQuotaRequest {
    pub name: String,
    pub description: Option<String>,
    pub metric: String, // requests | tokens | cost_usd
    pub period: String, // minute | hour | day | month | total
    pub limit_value: String,
    pub warning_threshold: Option<String>,
    pub action: Option<String>, // block | warn | throttle
}

#[derive(Deserialize)]
pub struct UpdateQuotaRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub metric: Option<String>,
    pub period: Option<String>,
    pub limit_value: Option<String>,
    pub warning_threshold: Option<String>,
    pub action: Option<String>,
    pub status: Option<String>,
}

/// List all quotas for the default org.
pub async fn list_quotas(
    State(state): State<AppState>,
) -> Result<Json<QuotaListResponse>, ApiError> {
    let repo = QuotaRepo::new(state.db_pool);
    let quotas = repo.list_by_org(default_org()).await?;
    Ok(Json(QuotaListResponse { data: quotas }))
}

/// Get a single quota.
pub async fn get_quota(
    State(state): State<AppState>,
    Path(quota_id): Path<Uuid>,
) -> Result<Json<gateway_db::models::Quota>, ApiError> {
    let repo = QuotaRepo::new(state.db_pool);
    match repo.get_by_id(default_org(), quota_id).await? {
        Some(quota) => Ok(Json(quota)),
        None => Err(ApiError::new("not_found", "Quota not found")),
    }
}

/// Create a new quota.
pub async fn create_quota(
    State(state): State<AppState>,
    Json(req): Json<CreateQuotaRequest>,
) -> Result<(StatusCode, Json<gateway_db::models::Quota>), ApiError> {
    let repo = QuotaRepo::new(state.db_pool);
    let limit = Decimal::from_str_exact(&req.limit_value)
        .map_err(|_| ApiError::new("invalid_limit", "limit_value must be a valid decimal"))?;
    let warning = req
        .warning_threshold
        .as_deref()
        .and_then(|s| Decimal::from_str_exact(s).ok())
        .unwrap_or_else(|| Decimal::from_str_exact("0.8").unwrap());
    let action = req.action.unwrap_or_else(|| "block".to_string());

    let quota = repo
        .create(
            default_org(),
            None, // api_key_id
            &req.name,
            req.description.as_deref(),
            &req.metric,
            &req.period,
            limit,
            warning,
            "all",
            serde_json::Value::Object(Default::default()),
            &action,
            "active",
        )
        .await?;

    Ok((StatusCode::CREATED, Json(quota)))
}

/// Update a quota.
pub async fn update_quota(
    State(state): State<AppState>,
    Path(quota_id): Path<Uuid>,
    Json(req): Json<UpdateQuotaRequest>,
) -> Result<Json<gateway_db::models::Quota>, ApiError> {
    let repo = QuotaRepo::new(state.db_pool);
    let limit = req
        .limit_value
        .and_then(|s| Decimal::from_str_exact(&s).ok());
    let warning = req
        .warning_threshold
        .and_then(|s| Decimal::from_str_exact(&s).ok());

    match repo
        .update(
            default_org(),
            quota_id,
            req.name.as_deref(),
            Some(req.description.as_deref()),
            req.metric.as_deref(),
            req.period.as_deref(),
            limit,
            warning,
            None, // applies_to
            None, // scope_filter
            req.action.as_deref(),
            req.status.as_deref(),
        )
        .await?
    {
        Some(quota) => Ok(Json(quota)),
        None => Err(ApiError::new("not_found", "Quota not found")),
    }
}

/// Delete a quota.
pub async fn delete_quota(
    State(state): State<AppState>,
    Path(quota_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let repo = QuotaRepo::new(state.db_pool);
    if repo.delete(default_org(), quota_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new("not_found", "Quota not found"))
    }
}
