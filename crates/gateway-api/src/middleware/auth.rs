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
use gateway_auth::{validate_key_format, AuthContext, AuthType};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Default organization ID used for stub API key auth context.
const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Public routes that skip authentication.
const PUBLIC_ROUTES: &[&str] = &[
    "/health",
    "/ready",
    "/login",
    "/v1/auth/login",
    "/v1/auth/refresh",
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
        api_key_auth(token).await?
    } else {
        // Session JWT path
        session_auth(&state, token)?
    };

    // Attach context to request extensions for downstream handlers
    req.extensions_mut().insert(auth_context);

    Ok(next.run(req).await)
}

/// Validate an API key and build auth context.
async fn api_key_auth(token: &str) -> Result<AuthContext, ApiError> {
    if !validate_key_format(token) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key_format",
            "Invalid API key format.",
        ));
    }

    // TODO: Replace stub with DB-backed lookup (TASK-0015 completion).
    // Current behavior: any valid-format key is accepted with default org context.
    Ok(AuthContext {
        auth_type: AuthType::ApiKey,
        org_id: Uuid::parse_str(DEFAULT_ORG_ID).expect("default org id is valid"),
        user_id: None,
        key_id: None,
        role: None,
        permissions: vec![],
        rate_limit_rps: Some(100),
    })
}

/// Verify a JWT access token and build auth context.
fn session_auth(state: &AppState, token: &str) -> Result<AuthContext, ApiError> {
    let claims = state.jwt.verify_access(token).map_err(|e| match e {
        gateway_auth::AuthError::TokenExpired => {
            ApiError::new(StatusCode::UNAUTHORIZED, "token_expired", "Access token expired")
        }
        _ => ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "Invalid access token"),
    })?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "Invalid token subject")
    })?;

    let org_id = Uuid::parse_str(&claims.org_id).map_err(|_| {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "Invalid token organization")
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
    PUBLIC_ROUTES.iter().any(|&r| path == r)
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
