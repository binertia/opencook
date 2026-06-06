//! Unified authentication middleware.
//!
//! Handles both API key auth (`Authorization: Bearer sk_gw_...`) and
//! session JWT auth (`Authorization: Bearer <jwt>`).
//!
//! Attaches an `AuthContext` to request extensions for downstream handlers.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use gateway_auth::{sha256_hex, validate_key_format, verify_key_hash, AuthContext, AuthType};
use gateway_db::{models::ApiKey, ApiKeyRepo};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Redis cache TTL for API key entries (5 minutes).
const APIKEY_CACHE_TTL: i64 = 300;

/// Public routes that skip authentication.
const PUBLIC_ROUTES: &[&str] = &[
    "/health",
    "/ready",
    "/login",
    "/v1/auth/login",
    "/api/v1/auth/login",
    "/v1/auth/refresh",
    "/api/v1/auth/refresh",
];

/// Unified auth middleware: validates API keys or JWT session tokens.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let path = req.uri().path();

    // Skip auth for public routes
    if is_public_route(path) {
        return Ok(next.run(req).await);
    }

    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "missing_auth_header",
                "Missing Authorization header. Expected: Bearer <token>",
            )
        })?;

    // Parse Bearer token
    let token = parse_bearer_token(auth_header).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_auth_format",
            "Invalid Authorization header format. Expected: Bearer <token>",
        )
    })?;

    // Determine auth type and validate
    let auth_context = if token.starts_with("sk_gw_") {
        // API key path
        api_key_auth(&state, token).await?
    } else {
        // Session JWT path
        session_auth(&state, token)?
    };

    // Attach context to request extensions for downstream handlers
    req.extensions_mut().insert(auth_context);

    Ok(next.run(req).await)
}

/// Validate an API key against the cache or database and build auth context.
async fn api_key_auth(state: &AppState, token: &str) -> Result<AuthContext, ApiError> {
    if !validate_key_format(token) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key_format",
            "Invalid API key format.",
        ));
    }

    let key_hash = sha256_hex(token);
    let cache_key = format!("auth:apikey:{key_hash}");

    // 1. Try Redis cache first.
    let cached: Option<String> = {
        let mut conn = state.redis.clone();
        redis::cmd("GET")
            .arg(&cache_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(None)
    };

    let api_key = if let Some(json_str) = cached {
        match serde_json::from_str::<ApiKey>(&json_str) {
            Ok(key) => {
                tracing::debug!(key_id = %key.id, "API key auth cache hit");
                key
            }
            Err(e) => {
                tracing::warn!(error = %e, "API key cache deserialization failed, falling back to DB");
                fetch_api_key_from_db(state, &key_hash).await?
            }
        }
    } else {
        let key = fetch_api_key_from_db(state, &key_hash).await?;

        // Populate cache asynchronously (best-effort).
        if let Ok(json) = serde_json::to_string(&key) {
            let mut conn = state.redis.clone();
            let _: () = redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(APIKEY_CACHE_TTL)
                .arg(json)
                .query_async(&mut conn)
                .await
                .unwrap_or(());
        }

        key
    };

    // Constant-time hash verification (defense in depth even after cache/DB lookup)
    if !verify_key_hash(token, &api_key.key_hash) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "Invalid API key.",
        ));
    }

    Ok(AuthContext {
        auth_type: AuthType::ApiKey,
        org_id: api_key.org_id,
        user_id: api_key.user_id,
        key_id: Some(api_key.id),
        role: None,
        permissions: api_key.scopes.0.clone(),
        rate_limit_rps: Some(api_key.rate_limit_rps),
    })
}

/// Fetch API key from DB (fallback when cache misses).
async fn fetch_api_key_from_db(state: &AppState, key_hash: &str) -> Result<ApiKey, ApiError> {
    let repo = ApiKeyRepo::new(state.db_pool.clone());
    repo.find_by_key_hash(key_hash)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_api_key",
                "Invalid API key.",
            )
        })
}

/// Verify a JWT access token and build auth context.
fn session_auth(state: &AppState, token: &str) -> Result<AuthContext, Box<ApiError>> {
    let claims = state.jwt.verify_access(token).map_err(|e| match e {
        gateway_auth::AuthError::TokenExpired => Box::new(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "token_expired",
            "Access token expired",
        )),
        _ => Box::new(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid access token",
        )),
    })?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        Box::new(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid token subject",
        ))
    })?;

    let org_id = Uuid::parse_str(&claims.active_org_id).map_err(|_| {
        Box::new(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid token organization",
        ))
    })?;

    Ok(AuthContext {
        auth_type: AuthType::Session,
        org_id,
        user_id: Some(user_id),
        key_id: None,
        role: Some(claims.role),
        permissions: vec![],
        rate_limit_rps: Some(100),
    })
}

/// Check if a route is public (no auth required).
fn is_public_route(path: &str) -> bool {
    PUBLIC_ROUTES.contains(&path)
}

/// Parse a Bearer token from an Authorization header value.
fn parse_bearer_token(header: &str) -> Option<&str> {
    let parts: Vec<&str> = header.splitn(2, ' ').collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
        Some(parts[1])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_route() {
        assert!(is_public_route("/health"));
        assert!(is_public_route("/v1/auth/login"));
        assert!(!is_public_route("/v1/auth/me"));
        assert!(!is_public_route("/v1/dashboard"));
    }

    #[test]
    fn test_parse_bearer_token_valid() {
        assert_eq!(parse_bearer_token("Bearer token123"), Some("token123"));
        assert_eq!(parse_bearer_token("bearer token123"), Some("token123"));
    }

    #[test]
    fn test_parse_bearer_token_invalid() {
        assert_eq!(parse_bearer_token("Basic dXNlcjpwYXNz"), None);
        assert_eq!(parse_bearer_token("token123"), None);
        assert_eq!(parse_bearer_token(""), None);
    }
}
