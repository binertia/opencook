//! Health and readiness probes.

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Serialize)]
pub struct ReadyResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Liveness probe — always returns 200.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// Readiness probe — returns 200 only when DB is reachable.
pub async fn readiness_check(State(state): State<AppState>) -> (StatusCode, Json<ReadyResponse>) {
    let db_ok = match &state.db_pool {
        gateway_db::DbBackend::Postgres(pool) => {
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        }
        gateway_db::DbBackend::Sqlite(pool) => {
            sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
        }
    };

    if db_ok {
        (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready".to_string(),
                reason: None,
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready".to_string(),
                reason: Some("database unreachable".to_string()),
            }),
        )
    }
}
