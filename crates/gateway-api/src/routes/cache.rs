//! Cache analytics and stats routes.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{
    rbac::{check_permission, Permission, Role},
    AuthContext,
};
use gateway_cache::analytics::CacheAnalytics;
use serde::{Deserialize, Serialize};

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
pub struct CacheStatsQuery {
    /// Time period: "1h", "24h", "7d", "30d"
    #[serde(default = "default_period")]
    pub period: String,
}

fn default_period() -> String {
    "24h".to_string()
}

#[derive(Debug, Serialize)]
pub struct CacheStatsResponse {
    pub org_id: String,
    pub period: String,
    pub hit_rate: f64,
    pub cost_saved_usd: f64,
    pub entry_count: i64,
    pub top_models: Vec<TopModelStats>,
}

#[derive(Debug, Serialize)]
pub struct SemanticCacheStatsResponse {
    pub org_id: String,
    pub total_entries: i64,
    pub newest_entry: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TopModelStats {
    pub model_id: String,
    pub entry_count: i64,
    pub total_hits: i64,
    pub avg_hits: f64,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn get_cache_stats(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<CacheStatsQuery>,
) -> Result<Json<CacheStatsResponse>, ApiError> {
    require_permission(&auth, Permission::UsageRead)?;

    let analytics = CacheAnalytics::new(state.db_pool.clone());

    let hit_rate = analytics
        .get_hit_rate(auth.org_id, &query.period)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "analytics_error", e.to_string()))?;

    let cost_saved_usd = analytics
        .get_cost_saved(auth.org_id, &query.period)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "analytics_error", e.to_string()))?;

    let top_models = analytics
        .get_top_cached_models(auth.org_id, 10)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    let entry_count = analytics.get_entry_count(auth.org_id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    Ok(Json(CacheStatsResponse {
        org_id: auth.org_id.to_string(),
        period: query.period,
        hit_rate,
        cost_saved_usd,
        entry_count,
        top_models: top_models
            .into_iter()
            .map(|m| TopModelStats {
                model_id: m.model_id,
                entry_count: m.entry_count,
                total_hits: m.total_hits,
                avg_hits: m.avg_hits,
            })
            .collect(),
    }))
}

pub async fn get_semantic_cache_stats(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<SemanticCacheStatsResponse>, ApiError> {
    require_permission(&auth, Permission::UsageRead)?;

    let stats = match &state.pgvector_semantic_cache {
        Some(cache) => cache.stats(auth.org_id).await.map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?,
        None => {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "semantic_cache_unavailable",
                "Semantic cache is not enabled or not available in SQLite mode",
            ));
        }
    };

    Ok(Json(SemanticCacheStatsResponse {
        org_id: auth.org_id.to_string(),
        total_entries: stats.total_entries,
        newest_entry: stats.newest_entry.map(|d| d.to_rfc3339()),
    }))
}
