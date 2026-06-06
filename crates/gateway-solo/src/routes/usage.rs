//! Usage analytics endpoints for SOLO mode.

use axum::{extract::State, Json};
use gateway_db::UsageRepo;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::AppState;

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

fn default_org() -> uuid::Uuid {
    uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid")
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct UsageQuery {
    #[serde(default = "default_period")]
    pub period: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

fn default_period() -> String {
    "hourly".to_string()
}
fn default_limit() -> i64 {
    100
}
fn default_offset() -> i64 {
    0
}

#[derive(Serialize)]
pub struct UsageResponse {
    pub data: Vec<UsageRecord>,
}

#[derive(Serialize)]
pub struct UsageRecord {
    pub period: String,
    pub period_start: String,
    pub request_count: i32,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: String,
}

#[derive(Serialize)]
pub struct CostResponse {
    pub total_cost: String,
    pub total_requests: i32,
    pub total_tokens: i64,
}

/// Get usage records.
pub async fn get_usage(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<UsageQuery>,
) -> Result<Json<UsageResponse>, ApiError> {
    let repo = UsageRepo::new(state.db_pool);
    let (start, end) = period_bounds();

    let records = repo
        .list_by_org_and_period(default_org(), &query.period, start, end)
        .await?;

    let data: Vec<UsageRecord> = records
        .into_iter()
        .map(|r| UsageRecord {
            period: r.period,
            period_start: r.period_start.to_rfc3339(),
            request_count: r.request_count,
            prompt_tokens: r.prompt_tokens,
            completion_tokens: r.completion_tokens,
            total_tokens: r.total_tokens,
            total_cost: r.total_cost.to_string(),
        })
        .collect();

    Ok(Json(UsageResponse { data }))
}

/// Get aggregated costs.
pub async fn get_costs(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<UsageQuery>,
) -> Result<Json<CostResponse>, ApiError> {
    let repo = UsageRepo::new(state.db_pool);
    let (start, end) = period_bounds();

    let records = repo
        .list_by_org_and_period(default_org(), &query.period, start, end)
        .await?;

    let total_cost: rust_decimal::Decimal = records.iter().map(|r| r.total_cost).sum();
    let total_requests: i32 = records.iter().map(|r| r.request_count).sum();
    let total_tokens: i64 = records.iter().map(|r| r.total_tokens).sum();

    Ok(Json(CostResponse {
        total_cost: total_cost.to_string(),
        total_requests,
        total_tokens,
    }))
}

fn period_bounds() -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    let now = chrono::Utc::now();
    let start = now - chrono::Duration::days(30);
    (start, now)
}
