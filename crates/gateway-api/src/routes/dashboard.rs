//! Dashboard KPI endpoint.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
use gateway_auth::AuthContext;
use gateway_db::{
    repos::{
        provider_config_repo::ProviderConfigRepo,
        request_repo::{RequestRepo, RequestStats},
    },
    ProviderConfig,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, state::AppState};

// ── Query / Response Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    #[serde(default = "default_range")]
    pub range: String,
}

fn default_range() -> String {
    "today".to_string()
}

#[derive(Debug, Serialize)]
pub struct RecentRequestItem {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub provider: String,
    pub status: String,
    pub tokens: i32,
    pub cost_usd: f64,
    pub latency_ms: i32,
}

#[derive(Debug, Serialize)]
pub struct ActiveProviderItem {
    pub id: String,
    pub name: String,
    pub status: String,
    pub last_check: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub total_requests: i64,
    pub total_cost_usd: f64,
    pub cache_hit_rate: f64,
    pub avg_latency_ms: f64,
    pub requests_change: f64,
    pub cost_change: f64,
    pub cache_change: f64,
    pub latency_change: f64,
    pub recent_requests: Vec<RecentRequestItem>,
    pub active_providers: Vec<ActiveProviderItem>,
}

// ── Handler ──────────────────────────────────────────────────────────

pub async fn get_dashboard(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<DashboardResponse>, ApiError> {
    let (current_start, current_end, previous_start, previous_end) =
        compute_time_ranges(&query.range);

    let req_repo = RequestRepo::new(state.db_pool.clone());

    // Current period stats
    let current_stats = req_repo
        .aggregate_stats(auth.org_id, current_start, current_end)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    // Previous period stats (for change calculation)
    let previous_stats = req_repo
        .aggregate_stats(auth.org_id, previous_start, previous_end)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    // Recent requests
    let recent = req_repo.list_recent(auth.org_id, 10).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    // Active providers with health
    let provider_repo = ProviderConfigRepo::new(state.db_pool.clone());
    let providers = provider_repo.list_by_org(auth.org_id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    let active_providers = build_active_providers(&state, &providers).await;

    let recent_requests: Vec<RecentRequestItem> = recent
        .into_iter()
        .map(|r| RecentRequestItem {
            id: r.id.to_string(),
            timestamp: r.gateway_received_at.to_rfc3339(),
            model: r
                .model_routed
                .unwrap_or_else(|| r.model_requested.unwrap_or_default()),
            provider: "-".to_string(), // TODO: resolve provider name from provider_config_id
            status: if r.cache_hit {
                "cached".to_string()
            } else if r.status == "success" {
                "success".to_string()
            } else {
                "error".to_string()
            },
            tokens: r.total_tokens,
            cost_usd: r.total_cost.try_into().unwrap_or(0.0),
            latency_ms: r.latency_total_ms.unwrap_or(0),
        })
        .collect();

    let cache_hit_rate = calc_cache_rate(&current_stats);
    let prev_cache_hit_rate = calc_cache_rate(&previous_stats);

    let avg_latency_ms = if current_stats.total_requests > 0 {
        current_stats.avg_latency_ms
    } else {
        0.0
    };
    let prev_avg_latency_ms = if previous_stats.total_requests > 0 {
        previous_stats.avg_latency_ms
    } else {
        0.0
    };

    let total_cost_usd = current_stats.total_cost;
    let prev_total_cost = previous_stats.total_cost;

    Ok(Json(DashboardResponse {
        total_requests: current_stats.total_requests,
        total_cost_usd,
        cache_hit_rate,
        avg_latency_ms,
        requests_change: pct_change(
            previous_stats.total_requests as f64,
            current_stats.total_requests as f64,
        ),
        cost_change: pct_change(prev_total_cost, total_cost_usd),
        cache_change: pct_change(prev_cache_hit_rate, cache_hit_rate),
        latency_change: pct_change(prev_avg_latency_ms, avg_latency_ms),
        recent_requests,
        active_providers,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn compute_time_ranges(
    range: &str,
) -> (DateTime<Utc>, DateTime<Utc>, DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    match range {
        "today" => {
            let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let prev_start = start - Duration::days(1);
            let prev_end = start;
            (start, now, prev_start, prev_end)
        }
        "7d" => {
            let start = now - Duration::days(7);
            let prev_start = start - Duration::days(7);
            (start, now, prev_start, start)
        }
        "30d" => {
            let start = now - Duration::days(30);
            let prev_start = start - Duration::days(30);
            (start, now, prev_start, start)
        }
        _ => {
            // Default to today
            let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let prev_start = start - Duration::days(1);
            let prev_end = start;
            (start, now, prev_start, prev_end)
        }
    }
}

fn calc_cache_rate(stats: &RequestStats) -> f64 {
    let total = stats.cache_hits + stats.cache_misses;
    if total > 0 {
        (stats.cache_hits as f64 / total as f64) * 100.0
    } else {
        0.0
    }
}

fn pct_change(prev: f64, curr: f64) -> f64 {
    if prev == 0.0 {
        if curr == 0.0 {
            0.0
        } else {
            100.0
        }
    } else {
        ((curr - prev) / prev) * 100.0
    }
}

async fn build_active_providers(
    state: &AppState,
    providers: &[ProviderConfig],
) -> Vec<ActiveProviderItem> {
    let mut result = Vec::new();

    let mut conn = state.redis.clone();

    for p in providers {
        let key = format!("health:{}", p.id);
        let health_json: Option<String> = conn.get(&key).await.ok().flatten();

        let (status, last_check) = if let Some(json_str) = health_json {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let status = val
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let last_check = val
                    .get("checked_at")
                    .and_then(|s| s.as_str())
                    .unwrap_or(&p.updated_at.to_rfc3339())
                    .to_string();
                (status, last_check)
            } else {
                ("unknown".to_string(), p.updated_at.to_rfc3339())
            }
        } else {
            ("unknown".to_string(), p.updated_at.to_rfc3339())
        };

        result.push(ActiveProviderItem {
            id: p.id.to_string(),
            name: p.name.clone(),
            status,
            last_check,
        });
    }

    result
}
