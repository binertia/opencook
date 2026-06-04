//! Solo-mode dashboard API stubs.
//!
//! Returns sensible defaults for the React dashboard so pages don't crash
//! when running in SOLO mode without a full PostgreSQL backend.

use axum::{extract::State, Json};
use gateway_db::RequestRepo;
use serde::Serialize;

use crate::{error::ApiError, state::AppState};

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

// ── Dashboard KPIs ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DashboardResponse {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub estimated_cost: f64,
    pub active_providers: i64,
    pub cache_hit_rate: f64,
    pub avg_latency_ms: f64,
}

pub async fn get_dashboard(State(state): State<AppState>) -> Result<Json<DashboardResponse>, ApiError> {
    let org_id = uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid");
    let repo = RequestRepo::new(state.db_pool);

    let now = chrono::Utc::now();
    let start = now - chrono::Duration::hours(24);

    let stats = repo
        .aggregate_stats(org_id, start, now)
        .await
        .map_err(|e| ApiError::new("database_error", e.to_string()))?;

    let cache_hit_rate = if stats.total_requests > 0 {
        (stats.cache_hits as f64 / stats.total_requests as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(DashboardResponse {
        total_requests: stats.total_requests,
        total_tokens: 0,
        estimated_cost: stats.total_cost,
        active_providers: 1,
        cache_hit_rate,
        avg_latency_ms: stats.avg_latency_ms,
    }))
}

// ── Providers ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProviderListResponse {
    pub data: Vec<ProviderItem>,
}

#[derive(Serialize)]
pub struct ProviderItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub base_url: String,
    pub models: Vec<ProviderModel>,
    pub health_status: String,
    pub last_error: Option<String>,
}

#[derive(Serialize)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    pub status: String,
}

pub async fn list_providers() -> Json<ProviderListResponse> {
    let providers = vec![
        ProviderItem {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            kind: "openai".to_string(),
            status: "active".to_string(),
            base_url: "https://api.openai.com".to_string(),
            models: vec![
                ProviderModel { id: "gpt-4o".to_string(), name: "GPT-4o".to_string(), status: "active".to_string() },
                ProviderModel { id: "gpt-4o-mini".to_string(), name: "GPT-4o Mini".to_string(), status: "active".to_string() },
            ],
            health_status: if std::env::var("OPENAI_API_KEY").is_ok() { "healthy" } else { "no_key" }.to_string(),
            last_error: None,
        },
        ProviderItem {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            kind: "anthropic".to_string(),
            status: "active".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            models: vec![
                ProviderModel { id: "claude-3-5-sonnet".to_string(), name: "Claude 3.5 Sonnet".to_string(), status: "active".to_string() },
            ],
            health_status: if std::env::var("ANTHROPIC_API_KEY").is_ok() { "healthy" } else { "no_key" }.to_string(),
            last_error: None,
        },
    ];

    Json(ProviderListResponse { data: providers })
}

// ── Analytics ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AnalyticsResponse {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub requests_by_model: Vec<ModelStat>,
    pub requests_by_status: Vec<StatusStat>,
}

#[derive(Serialize)]
pub struct ModelStat {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct StatusStat {
    pub status: String,
    pub count: i64,
}

pub async fn get_analytics(State(state): State<AppState>) -> Result<Json<AnalyticsResponse>, ApiError> {
    let org_id = uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid");
    let repo = RequestRepo::new(state.db_pool);

    let now = chrono::Utc::now();
    let start = now - chrono::Duration::hours(24);

    let stats = repo
        .aggregate_stats(org_id, start, now)
        .await
        .map_err(|e| ApiError::new("database_error", e.to_string()))?;

    Ok(Json(AnalyticsResponse {
        total_requests: stats.total_requests,
        total_tokens: 0,
        total_cost: stats.total_cost,
        requests_by_model: vec![],
        requests_by_status: vec![
            StatusStat { status: "success".to_string(), count: stats.total_requests - stats.cache_misses },
            StatusStat { status: "cache_hit".to_string(), count: stats.cache_hits },
        ],
    }))
}

// ── API Keys ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ApiKeyListResponse {
    pub data: Vec<ApiKeyItem>,
}

#[derive(Serialize)]
pub struct ApiKeyItem {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub status: String,
    pub scopes: Vec<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

pub async fn list_api_keys() -> Json<ApiKeyListResponse> {
    Json(ApiKeyListResponse {
        data: vec![ApiKeyItem {
            id: "solo-key".to_string(),
            name: "Default Solo Key".to_string(),
            key_prefix: "solo".to_string(),
            status: "active".to_string(),
            scopes: vec!["all".to_string()],
            last_used_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
    })
}

// ── Users ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UserListResponse {
    pub data: Vec<UserItem>,
}

#[derive(Serialize)]
pub struct UserItem {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub last_login_at: Option<String>,
    pub created_at: String,
}

pub async fn list_users() -> Json<UserListResponse> {
    Json(UserListResponse {
        data: vec![UserItem {
            id: "solo-user".to_string(),
            email: "solo@opencook.local".to_string(),
            name: "Solo Developer".to_string(),
            role: "owner".to_string(),
            status: "active".to_string(),
            last_login_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
    })
}
