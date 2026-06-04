//! Analytics endpoint — usage metrics and cost breakdowns.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Timelike, Utc};
use gateway_auth::AuthContext;
use gateway_db::{
    repos::request_repo::RequestRepo,
    Request,
};
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, state::AppState};

// ── Query / Response Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    #[serde(default = "default_range")]
    pub range: String,
}

fn default_range() -> String {
    "30d".to_string()
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesPoint {
    pub timestamp: String,
    pub requests: i64,
    pub tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
    pub latency_ms: f64,
    pub cache_hits: i64,
    pub cache_misses: i64,
}

#[derive(Debug, Serialize)]
pub struct BreakdownItem {
    pub dimension: String,
    pub value: String,
    pub requests: i64,
    pub tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct CacheBreakdownItem {
    pub model: String,
    pub requests: i64,
    pub cache_hits: i64,
    pub cache_hit_rate: f64,
    pub cost_saved_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_cost_usd: f64,
    pub cost_saved_from_cache_usd: f64,
    pub avg_latency_ms: f64,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
    pub time_series: Vec<TimeSeriesPoint>,
    pub by_model: Vec<BreakdownItem>,
    pub by_status: Vec<BreakdownItem>,
    pub top_cached_models: Vec<CacheBreakdownItem>,
}

// ── Handler ──────────────────────────────────────────────────────────

pub async fn get_analytics(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<AnalyticsResponse>, ApiError> {
    let (start, end) = compute_range(&query.range);
    let repo = RequestRepo::new(state.db_pool.clone());

    let stats = repo
        .aggregate_stats(auth.org_id, start, end)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let requests = repo
        .list_recent(auth.org_id, 1000)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let time_series = build_time_series(&requests, start, end, &query.range);
    let by_model = build_model_breakdown(&requests);
    let by_status = build_status_breakdown(&requests);
    let top_cached_models = build_cache_breakdown(&requests);

    let total_tokens: i64 = requests.iter().map(|r| r.total_tokens as i64).sum();
    let prompt_tokens: i64 = requests.iter().map(|r| r.prompt_tokens as i64).sum();
    let completion_tokens: i64 = requests.iter().map(|r| r.completion_tokens as i64).sum();
    let total_errors = requests.iter().filter(|r| r.status == "error").count() as i64;
    let error_rate = if stats.total_requests > 0 {
        (total_errors as f64 / stats.total_requests as f64) * 100.0
    } else {
        0.0
    };

    let cache_hit_rate = if stats.cache_hits + stats.cache_misses > 0 {
        (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64) * 100.0
    } else {
        0.0
    };

    let cost_saved_from_cache_usd: f64 = requests
        .iter()
        .filter(|r| r.cache_hit)
        .map(|r| f64::try_from(r.total_cost).unwrap_or(0.0))
        .sum();

    Ok(Json(AnalyticsResponse {
        total_requests: stats.total_requests,
        total_tokens,
        prompt_tokens,
        completion_tokens,
        total_cost_usd: stats.total_cost,
        cost_saved_from_cache_usd,
        avg_latency_ms: stats.avg_latency_ms,
        cache_hit_rate,
        error_rate,
        time_series,
        by_model,
        by_status,
        top_cached_models,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn compute_range(range: &str) -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    let start = match range {
        "today" => now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
        "7d" => now - chrono::Duration::days(7),
        "30d" => now - chrono::Duration::days(30),
        _ => now - chrono::Duration::days(30),
    };
    (start, now)
}

fn build_time_series(
    requests: &[Request],
    start: DateTime<Utc>,
    _end: DateTime<Utc>,
    range: &str,
) -> Vec<TimeSeriesPoint> {
    let bucket_duration = match range {
        "today" => chrono::Duration::hours(1),
        "7d" => chrono::Duration::days(1),
        _ => chrono::Duration::days(1),
    };

    let mut buckets: std::collections::BTreeMap<DateTime<Utc>, TimeSeriesPoint> =
        std::collections::BTreeMap::new();

    // Initialize buckets
    let mut t = start;
    while t < Utc::now() {
        buckets.insert(
            t,
            TimeSeriesPoint {
                timestamp: t.to_rfc3339(),
                requests: 0,
                tokens: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                cost_usd: 0.0,
                latency_ms: 0.0,
                cache_hits: 0,
                cache_misses: 0,
            },
        );
        t += bucket_duration;
    }

    for r in requests {
        let bucket = r.gateway_received_at.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let bucket = if range == "today" {
            let hour = r.gateway_received_at.hour();
            r.gateway_received_at.date_naive().and_hms_opt(hour, 0, 0).unwrap().and_utc()
        } else {
            bucket
        };

        if let Some(point) = buckets.get_mut(&bucket) {
            point.requests += 1;
            point.tokens += r.total_tokens as i64;
            point.prompt_tokens += r.prompt_tokens as i64;
            point.completion_tokens += r.completion_tokens as i64;
            point.cost_usd += f64::try_from(r.total_cost).unwrap_or(0.0);
            if let Some(lat) = r.latency_total_ms {
                point.latency_ms += lat as f64;
            }
            if r.cache_hit {
                point.cache_hits += 1;
            } else {
                point.cache_misses += 1;
            }
        }
    }

    // Average latency per bucket
    for point in buckets.values_mut() {
        if point.requests > 0 {
            point.latency_ms /= point.requests as f64;
        }
    }

    buckets.into_values().collect()
}

fn build_model_breakdown(requests: &[Request]) -> Vec<BreakdownItem> {
    let mut map: std::collections::HashMap<String, BreakdownItem> =
        std::collections::HashMap::new();

    for r in requests {
        let model = r
            .model_routed
            .clone()
            .unwrap_or_else(|| r.model_requested.clone().unwrap_or_else(|| "unknown".to_string()));

        let entry = map.entry(model.clone()).or_insert_with(|| BreakdownItem {
            dimension: "model".to_string(),
            value: model,
            requests: 0,
            tokens: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            cost_usd: 0.0,
        });

        entry.requests += 1;
        entry.tokens += r.total_tokens as i64;
        entry.prompt_tokens += r.prompt_tokens as i64;
        entry.completion_tokens += r.completion_tokens as i64;
        entry.cost_usd += f64::try_from(r.total_cost).unwrap_or(0.0);
    }

    let mut items: Vec<_> = map.into_values().collect();
    items.sort_by(|a, b| b.requests.cmp(&a.requests));
    items.into_iter().take(10).collect()
}

fn build_status_breakdown(requests: &[Request]) -> Vec<BreakdownItem> {
    let mut map: std::collections::HashMap<String, BreakdownItem> =
        std::collections::HashMap::new();

    for r in requests {
        let status = if r.cache_hit {
            "cached"
        } else {
            &r.status
        }
        .to_string();

        let entry = map.entry(status.clone()).or_insert_with(|| BreakdownItem {
            dimension: "status".to_string(),
            value: status,
            requests: 0,
            tokens: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            cost_usd: 0.0,
        });

        entry.requests += 1;
        entry.tokens += r.total_tokens as i64;
        entry.prompt_tokens += r.prompt_tokens as i64;
        entry.completion_tokens += r.completion_tokens as i64;
        entry.cost_usd += f64::try_from(r.total_cost).unwrap_or(0.0);
    }

    map.into_values().collect()
}

fn build_cache_breakdown(requests: &[Request]) -> Vec<CacheBreakdownItem> {
    let mut map: std::collections::HashMap<String, (i64, i64, f64)> =
        std::collections::HashMap::new();

    for r in requests {
        let model = r
            .model_routed
            .clone()
            .unwrap_or_else(|| r.model_requested.clone().unwrap_or_else(|| "unknown".to_string()));

        let (total, hits, saved) = map.entry(model.clone()).or_insert((0, 0, 0.0));
        *total += 1;
        if r.cache_hit {
            *hits += 1;
            *saved += f64::try_from(r.total_cost).unwrap_or(0.0);
        }
    }

    let mut items: Vec<CacheBreakdownItem> = map
        .into_iter()
        .map(|(model, (requests, cache_hits, cost_saved_usd))| {
            let rate = if requests > 0 {
                (cache_hits as f64 / requests as f64) * 100.0
            } else {
                0.0
            };
            CacheBreakdownItem {
                model,
                requests,
                cache_hits,
                cache_hit_rate: rate,
                cost_saved_usd,
            }
        })
        .collect();

    items.sort_by(|a, b| b.cache_hits.cmp(&a.cache_hits));
    items.into_iter().take(10).collect()
}
