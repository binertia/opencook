//! Prometheus metrics exposition endpoint.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

/// GET /metrics — Prometheus-compatible metrics text.
/// No auth required (Prometheus scrapes this).
pub async fn metrics_handler(State(_state): State<AppState>) -> Response {
    match gateway_observability::metrics::handle() {
        Some(handle) => {
            let body = handle.render();
            ([("content-type", "text/plain; charset=utf-8")], body).into_response()
        }
        None => {
            (StatusCode::SERVICE_UNAVAILABLE, "Metrics not initialized").into_response()
        }
    }
}
