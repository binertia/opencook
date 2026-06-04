//! Request log endpoints for SOLO mode.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use gateway_db::RequestRepo;
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, state::AppState};

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Deserialize)]
pub struct ListRequestsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Serialize)]
pub struct RequestsResponse {
    pub data: Vec<RequestItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize)]
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
}

impl From<gateway_db::models::Request> for RequestItem {
    fn from(r: gateway_db::models::Request) -> Self {
        Self {
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
        }
    }
}

pub async fn list_requests(
    State(state): State<AppState>,
    Query(query): Query<ListRequestsQuery>,
) -> Result<Json<RequestsResponse>, ApiError> {
    let org_id = uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid");
    let repo = RequestRepo::new(state.db_pool);

    // Fetch recent requests
    let requests = repo
        .list_recent(org_id, query.limit)
        .await
        .map_err(|e| ApiError::new("database_error", e.to_string()))?;

    // Simple filtering in memory for status/model
    let mut items: Vec<RequestItem> = requests.into_iter().map(RequestItem::from).collect();

    if let Some(status_filter) = query.status {
        items.retain(|r| r.status == status_filter);
    }
    if let Some(model_filter) = query.model {
        items.retain(|r| {
            r.model_requested.as_ref() == Some(&model_filter)
                || r.model_routed.as_ref() == Some(&model_filter)
        });
    }

    let total = items.len() as i64;

    // Apply offset
    let offset = query.offset as usize;
    if offset > 0 && offset < items.len() {
        items = items.split_off(offset);
    }

    Ok(Json(RequestsResponse {
        data: items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

pub async fn get_request(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> Result<Json<RequestItem>, ApiError> {
    let org_id = uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid");
    let repo = RequestRepo::new(state.db_pool);

    let req_id = uuid::Uuid::parse_str(&request_id)
        .map_err(|_| ApiError::new("bad_request", "Invalid request ID"))?;

    let request = repo
        .get_by_id(org_id, req_id)
        .await
        .map_err(|e| ApiError::new("database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new("not_found", "Request not found"))?;

    Ok(Json(RequestItem::from(request)))
}
