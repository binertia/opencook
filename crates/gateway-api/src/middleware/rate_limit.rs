//! Rate limiting middleware.
//!
//! Checks multiple rate limit layers in sequence:
//! 1. Global gateway protection
//! 2. Organization-level limits
//! 3. API key-level request limits
//! 4. API key-level token limits
//! 5. Provider-level limits
//! 6. IP-level limits
//!
//! The first exceeded limit short-circuits the rest.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use gateway_auth::AuthContext;
use gateway_quota::{LayerCheck, LimitResult, RateLimiter};
use tracing::warn;

use crate::{error::ApiError, state::AppState};

/// Default rate limits.
const GLOBAL_RPS: u64 = 2000;
const GLOBAL_BURST: u64 = 4000;
const IP_RPS: u64 = 100;
const IP_BURST: u64 = 200;

/// Extract client IP respecting `trusted_proxy_count` config.
/// When `trusted_proxy_count` is 0, ignores `X-Forwarded-For` entirely.
fn extract_client_ip(req: &Request, trusted_proxy_count: usize) -> String {
    if trusted_proxy_count > 0 {
        if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
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

/// Rate limit middleware: checks all layers and rejects if any limit is exceeded.
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let limiter = RateLimiter::new(state.redis.clone());

    // Extract auth context (set by auth_middleware)
    let auth = req
        .extensions()
        .get::<AuthContext>()
        .cloned()
        .unwrap_or_else(|| {
            // Fallback for unauthenticated requests (shouldn't happen for API routes)
            AuthContext {
                auth_type: gateway_auth::AuthType::ApiKey,
                org_id: uuid::Uuid::nil(),
                user_id: None,
                key_id: None,
                role: None,
                permissions: vec![],
                rate_limit_rps: Some(100),
            }
        });

    let org_id = auth.org_id.to_string();
    let key_id = auth.key_id.map(|k| k.to_string()).unwrap_or_default();

    // Extract client IP respecting trusted proxy config
    let client_ip = extract_client_ip(&req, state.config.trusted_proxy_count);

    // Build rate limit layers
    let mut layers: Vec<LayerCheck> = vec![];

    // Layer 1: Global gateway protection (token bucket)
    layers.push(LayerCheck::TokenBucket {
        key: "ratelimit:global:req".to_string(),
        rate: GLOBAL_RPS as f64,
        burst: GLOBAL_BURST,
        cost: 1,
    });

    // Layer 2: Organization-level request limit (token bucket)
    // TODO: Load from org config. Using default for now.
    layers.push(LayerCheck::TokenBucket {
        key: format!("ratelimit:org:{}:req", org_id),
        rate: auth.rate_limit_rps.unwrap_or(100) as f64,
        burst: (auth.rate_limit_rps.unwrap_or(100) * 2) as u64,
        cost: 1,
    });

    // Layer 3: API key-level request limit (sliding window)
    if !key_id.is_empty() {
        layers.push(LayerCheck::SlidingWindow {
            key: format!("ratelimit:key:{}:req", key_id),
            limit: auth.rate_limit_rps.unwrap_or(100) as u64 * 60, // per-minute
            window_secs: 60,
        });
    }

    // Layer 4: API key-level token limit (sliding window)
    // TODO: Get estimated token count from request body. Using 1 for now.
    if !key_id.is_empty() {
        layers.push(LayerCheck::SlidingWindow {
            key: format!("ratelimit:key:{}:tok", key_id),
            limit: 1_000_000, // 1M tokens/minute default
            window_secs: 60,
        });
    }

    // Layer 5: Provider-level limit
    // TODO: Get provider from request. Using generic for now.
    layers.push(LayerCheck::TokenBucket {
        key: "ratelimit:prov:default:req".to_string(),
        rate: 1000.0,
        burst: 2000,
        cost: 1,
    });

    // Layer 6: IP-level protection (token bucket)
    layers.push(LayerCheck::TokenBucket {
        key: format!("ratelimit:ip:{}:req", client_ip),
        rate: IP_RPS as f64,
        burst: IP_BURST,
        cost: 1,
    });

    // Check all layers
    let result = limiter.check_layers(layers).await;

    match result {
        LimitResult::Allowed {
            remaining,
            reset_at,
            limit,
        } => {
            // Add rate limit headers to response
            let mut response = next.run(req).await;
            let headers = response.headers_mut();
            if let Ok(v) = limit.to_string().parse() {
                headers.insert("X-RateLimit-Limit", v);
            }
            if let Ok(v) = remaining.to_string().parse() {
                headers.insert("X-RateLimit-Remaining", v);
            }
            if let Ok(v) = reset_at.to_string().parse() {
                headers.insert("X-RateLimit-Reset", v);
            }
            Ok(response)
        }
        LimitResult::Exceeded {
            retry_after,
            limit: _,
        } => {
            warn!(
                org_id = %org_id,
                key_id = %key_id,
                ip = %client_ip,
                retry_after = retry_after,
                "Rate limit exceeded"
            );
            let err = ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                format!("Rate limit exceeded. Retry after {} seconds.", retry_after),
            );
            Err(err)
        }
    }
}
