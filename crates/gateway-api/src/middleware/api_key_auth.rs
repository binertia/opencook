//! API key authentication middleware.
//!
//! Extracts `Authorization: Bearer sk_gw_...` headers, validates key format,
//! and attaches an `AuthContext` to request extensions.
//!
//! **Current state:** Format validation + stub context (no DB lookup yet).
//! Full DB-backed validation will replace the stub when TASK-0015 is completed.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use gateway_auth::{validate_key_format, AuthContext, AuthType};
use uuid::Uuid;

use crate::error::ApiError;

/// Default organization ID used for stub auth context.
/// TODO: Remove once DB-backed key lookup is implemented.
const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Tower middleware: validate API key and attach AuthContext.
///
/// Skips auth for public routes (/health, /ready).
/// Returns 401 for missing or malformed API keys.
pub async fn api_key_auth_middleware(
    req: Request,
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
                "Missing Authorization header. Expected: Bearer sk_gw_...",
            )
        })?;

    // Parse Bearer token
    let api_key = parse_bearer_token(auth_header).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_auth_format",
            "Invalid Authorization header format. Expected: Bearer sk_gw_...",
        )
    })?;

    // Validate key format and checksum
    if !validate_key_format(api_key) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key_format",
            "Invalid API key format.",
        ));
    }

    // TODO: Replace stub with DB-backed lookup (TASK-0015 completion).
    // Current behavior: any valid-format key is accepted with default org context.
    // This is sufficient for middleware wiring and rate limiter key extraction.
    let auth_context = AuthContext {
        auth_type: AuthType::ApiKey,
        org_id: Uuid::parse_str(DEFAULT_ORG_ID).expect("default org id is valid"),
        user_id: None,
        key_id: None, // TODO: Look up key_id from DB using SHA-256 hash
        role: None,
        permissions: vec![],
        rate_limit_rps: Some(100),
    };

    // Attach context to request extensions for downstream handlers
    let mut req = req;
    req.extensions_mut().insert(auth_context);

    Ok(next.run(req).await)
}

/// Check if a route is public (no auth required).
fn is_public_route(path: &str) -> bool {
    matches!(path, "/health" | "/ready")
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
    fn test_parse_bearer_token_valid() {
        assert_eq!(
            parse_bearer_token("Bearer sk_gw_abc123"),
            Some("sk_gw_abc123")
        );
    }

    #[test]
    fn test_parse_bearer_token_case_insensitive() {
        assert_eq!(
            parse_bearer_token("bearer sk_gw_abc123"),
            Some("sk_gw_abc123")
        );
    }

    #[test]
    fn test_parse_bearer_token_missing_prefix() {
        assert_eq!(parse_bearer_token("sk_gw_abc123"), None);
    }

    #[test]
    fn test_parse_bearer_token_empty() {
        assert_eq!(parse_bearer_token(""), None);
    }

    #[test]
    fn test_is_public_route() {
        assert!(is_public_route("/health"));
        assert!(is_public_route("/ready"));
        assert!(!is_public_route("/v1/chat/completions"));
        assert!(!is_public_route("/v1/models"));
    }
}
