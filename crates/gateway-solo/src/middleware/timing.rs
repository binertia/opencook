//! Request timing middleware — adds request ID and duration headers.

use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

/// Layer that injects `X-Gateway-Request-ID` and `X-Request-Time-Ms` headers.
pub async fn timing_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let request_id = uuid::Uuid::now_v7().to_string();

    let mut req = req;
    req.headers_mut().insert(
        "x-gateway-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    let span = tracing::info_span!("request", %request_id, method = %req.method(), path = %req.uri().path());
    let _enter = span.enter();

    let mut response = next.run(req).await;

    let duration_ms = start.elapsed().as_millis() as u64;
    response.headers_mut().insert(
        "x-request-time-ms",
        HeaderValue::from_str(&duration_ms.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response.headers_mut().insert(
        "x-gateway-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    tracing::info!(
        status = %response.status().as_u16(),
        duration_ms = %duration_ms,
        "Request completed"
    );

    response
}
