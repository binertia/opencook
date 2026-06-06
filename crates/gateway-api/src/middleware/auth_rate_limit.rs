//! Stricter rate limiting for authentication endpoints.
//!
//! Limits login and refresh attempts to 10 requests per minute per client IP.
//! This is applied in addition to the general rate limit middleware to
//! protect against credential stuffing and brute-force attacks.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use gateway_quota::{LayerCheck, LimitResult, RateLimiter};
use redis::aio::ConnectionManager;
use tracing::warn;

use crate::error::ApiError;

/// Auth endpoint rate limit: 10 requests per minute per IP.
const AUTH_IP_LIMIT: u64 = 10;
const AUTH_WINDOW_SECS: u64 = 60;

/// Middleware enforcing a strict per-IP rate limit on authentication routes.
pub async fn auth_rate_limit_middleware(
    State(redis): State<ConnectionManager>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let limiter = RateLimiter::new(redis);

    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let layers = vec![LayerCheck::SlidingWindow {
        key: format!("ratelimit:auth:ip:{}:req", client_ip),
        limit: AUTH_IP_LIMIT,
        window_secs: AUTH_WINDOW_SECS,
    }];

    match limiter.check_layers(layers).await {
        LimitResult::Allowed { .. } => Ok(next.run(req).await),
        LimitResult::Exceeded { retry_after, .. } => {
            warn!(
                ip = %client_ip,
                retry_after = retry_after,
                "Auth endpoint rate limit exceeded"
            );
            Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "auth_rate_limit_exceeded",
                format!(
                    "Too many authentication attempts. Retry after {} seconds.",
                    retry_after
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {

    use axum::body::Body;
    use axum::http::{Request, Response, StatusCode};
    use std::convert::Infallible;
    use std::future::{ready, Future};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tower::Service;

    #[derive(Clone)]
    #[allow(dead_code)]
    struct OkService;

    impl Service<Request<Body>> for OkService {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            Box::pin(ready(Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap())))
        }
    }
}
