//! Usage analytics routes — aggregated usage and cost data.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use gateway_auth::{
    rbac::{check_permission, Permission, Role},
    AuthContext,
};
use gateway_db::repos::usage_repo::UsageRepo;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

fn require_permission(auth: &AuthContext, permission: Permission) -> Result<(), Box<ApiError>> {
    let role = auth
        .role
        .as_deref()
        .and_then(Role::from_str)
        .unwrap_or(Role::Viewer);

    if !check_permission(role, permission) {
        return Err(Box::new(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            format!(
                "Role '{:?}' does not have permission '{:?}'",
                role, permission
            ),
        )));
    }
    Ok(())
}

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    /// ISO 8601 start time
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,
    /// ISO 8601 end time
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
    /// Granularity: hourly, daily, monthly
    #[serde(default = "default_granularity")]
    pub granularity: String,
}

fn default_granularity() -> String {
    "hourly".to_string()
}

#[derive(Debug, Serialize)]
pub struct UsageRecordResponse {
    pub id: Uuid,
    pub org_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub provider_config_id: Option<Uuid>,
    pub provider_model_id: Option<Uuid>,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub request_count: i32,
    pub request_success: i32,
    pub request_error: i32,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub total_cost: Decimal,
    pub latency_ms_p50: Option<i32>,
    pub latency_ms_p90: Option<i32>,
    pub latency_ms_p99: Option<i32>,
    pub latency_ms_avg: Option<i32>,
    pub cache_hits: i32,
    pub cache_misses: i32,
}

impl From<gateway_db::models::UsageRecord> for UsageRecordResponse {
    fn from(u: gateway_db::models::UsageRecord) -> Self {
        Self {
            id: u.id,
            org_id: u.org_id,
            api_key_id: u.api_key_id,
            provider_config_id: u.provider_config_id,
            provider_model_id: u.provider_model_id,
            period: u.period,
            period_start: u.period_start,
            request_count: u.request_count,
            request_success: u.request_success,
            request_error: u.request_error,
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            input_cost: u.input_cost.into(),
            output_cost: u.output_cost.into(),
            total_cost: u.total_cost.into(),
            latency_ms_p50: u.latency_ms_p50,
            latency_ms_p90: u.latency_ms_p90,
            latency_ms_p99: u.latency_ms_p99,
            latency_ms_avg: u.latency_ms_avg,
            cache_hits: u.cache_hits,
            cache_misses: u.cache_misses,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UsageListResponse {
    pub data: Vec<UsageRecordResponse>,
}

#[derive(Debug, Serialize)]
pub struct CostBreakdownItem {
    pub dimension: String, // provider, model, api_key, etc.
    pub dimension_id: Option<Uuid>,
    pub total_cost: Decimal,
    pub request_count: i32,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct CostBreakdownResponse {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub total_cost: Decimal,
    pub by_provider: Vec<CostBreakdownItem>,
    pub by_model: Vec<CostBreakdownItem>,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn get_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsageListResponse>, ApiError> {
    require_permission(&auth, Permission::UsageRead)?;
    if auth.org_id != org_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot access usage for another organization",
        ));
    }

    let end = query.end_time.unwrap_or_else(Utc::now);
    let start = query
        .start_time
        .unwrap_or_else(|| end - chrono::Duration::days(7));

    let repo = UsageRepo::new(state.db_pool);
    let records = repo
        .list_by_org_and_period(org_id, &query.granularity, start, end)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    Ok(Json(UsageListResponse {
        data: records.into_iter().map(UsageRecordResponse::from).collect(),
    }))
}

pub async fn get_costs(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<CostBreakdownResponse>, ApiError> {
    require_permission(&auth, Permission::UsageRead)?;
    if auth.org_id != org_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot access costs for another organization",
        ));
    }

    let end = query.end_time.unwrap_or_else(Utc::now);
    let start = query
        .start_time
        .unwrap_or_else(|| end - chrono::Duration::days(7));

    let repo = UsageRepo::new(state.db_pool);
    let records = repo
        .list_by_org_and_period(org_id, &query.granularity, start, end)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    let mut total_cost = Decimal::ZERO;
    let mut by_provider: std::collections::HashMap<Option<Uuid>, CostBreakdownItem> =
        std::collections::HashMap::new();
    let mut by_model: std::collections::HashMap<Option<Uuid>, CostBreakdownItem> =
        std::collections::HashMap::new();

    for r in &records {
        total_cost += r.total_cost;

        let provider_entry =
            by_provider
                .entry(r.provider_config_id)
                .or_insert_with(|| CostBreakdownItem {
                    dimension: "provider".to_string(),
                    dimension_id: r.provider_config_id,
                    total_cost: Decimal::ZERO,
                    request_count: 0,
                    total_tokens: 0,
                });
        provider_entry.total_cost += r.total_cost;
        provider_entry.request_count += r.request_count;
        provider_entry.total_tokens += r.total_tokens;

        let model_entry =
            by_model
                .entry(r.provider_model_id)
                .or_insert_with(|| CostBreakdownItem {
                    dimension: "model".to_string(),
                    dimension_id: r.provider_model_id,
                    total_cost: Decimal::ZERO,
                    request_count: 0,
                    total_tokens: 0,
                });
        model_entry.total_cost += r.total_cost;
        model_entry.request_count += r.request_count;
        model_entry.total_tokens += r.total_tokens;
    }

    Ok(Json(CostBreakdownResponse {
        start_time: start,
        end_time: end,
        total_cost,
        by_provider: by_provider.into_values().collect(),
        by_model: by_model.into_values().collect(),
    }))
}
