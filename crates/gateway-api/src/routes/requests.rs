//! Request log listing endpoint.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::AuthContext;
use gateway_db::repos::request_repo::RequestRepo;
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, state::AppState};

// ── Query / Response Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListRequestsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

fn default_limit() -> i64 { 50 }
fn default_offset() -> i64 { 0 }

#[derive(Debug, Serialize)]
pub struct RequestItem {
    pub id: String,
    pub trace_id: String,
    pub model_requested: Option<String>,
    pub model_routed: Option<String>,
    pub status: String,
    pub status_code: Option<i32>,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub total_cost: String,
    pub latency_total_ms: Option<i32>,
    pub cache_hit: bool,
    pub gateway_received_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RequestsListResponse {
    pub data: Vec<RequestItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_requests(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<ListRequestsQuery>,
) -> Result<Json<RequestsListResponse>, ApiError> {
    let repo = RequestRepo::new(state.db_pool.clone());

    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.max(0);

    // Fetch one extra to determine if there's a next page
    let rows = repo
        .list_recent(auth.org_id, limit + 1)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let has_more = rows.len() > limit as usize;
    let total_count = rows.len() as i64;
    let items: Vec<RequestItem> = rows.into_iter().take(limit as usize).map(|r| RequestItem {
        id: r.id.to_string(),
        trace_id: r.trace_id,
        model_requested: r.model_requested,
        model_routed: r.model_routed,
        status: r.status,
        status_code: r.status_code,
        prompt_tokens: r.prompt_tokens,
        completion_tokens: r.completion_tokens,
        total_tokens: r.total_tokens,
        total_cost: r.total_cost.to_string(),
        latency_total_ms: r.latency_total_ms,
        cache_hit: r.cache_hit,
        gateway_received_at: Some(r.gateway_received_at.to_rfc3339()),
        completed_at: r.completed_at.map(|d| d.to_rfc3339()),
        error_message: r.error_message,
        provider: None, // Could be populated from provider_config lookup
    }).collect();

    Ok(Json(RequestsListResponse {
        data: items,
        total: total_count,
        limit,
        offset,
    }))
}
