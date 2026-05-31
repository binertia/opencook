//! Prometheus metrics endpoint.

use axum::{body::Body, http::StatusCode, response::Response};

pub async fn metrics_handler() -> Response<Body> {
    match gateway_observability::metrics::handle() {
        Some(handle) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; charset=utf-8")
            .body(Body::from(handle.render()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Metrics not initialized"))
            .unwrap(),
    }
}
