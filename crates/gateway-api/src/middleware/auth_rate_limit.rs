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
use tracing::warn;

use crate::{error::ApiError, state::AppState};

/// Auth endpoint rate limit: 10 requests per minute per IP.
const AUTH_IP_LIMIT: u64 = 10;
const AUTH_WINDOW_SECS: u64 = 60;

/// Extract client IP respecting `trusted_proxy_count` config.
/// When `trusted_proxy_count` is 0, ignores `X-Forwarded-For` entirely.
fn extract_client_ip(req: &Request, trusted_proxy_count: usize) -> String {
    if trusted_proxy_count > 0 {
        if let Some(xff) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
        {
            let parts: Vec<&str> = xff.split(',').map(|s| s.trim()).collect();
            if parts.len() > trusted_proxy_count {
                return parts[parts.len() - trusted_proxy_count - 1].to_string();
            } else if let Some(last) = parts.last() {
                return last.to_string();
            }
        }
    }
    req.headers()
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Middleware enforcing a strict per-IP rate limit on authentication routes.
pub async fn auth_rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let limiter = RateLimiter::new(state.redis.clone());
    let client_ip = extract_client_ip(&req, state.config.trusted_proxy_count);

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
