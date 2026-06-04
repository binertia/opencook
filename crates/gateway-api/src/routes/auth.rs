//! Authentication routes — login, logout, refresh, and current user.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use gateway_auth::{AuthContext, PasswordHasherService};
use gateway_db::UserRepo;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{error::ApiError, state::AppState};

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: MeResponse,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /v1/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    // Validate request
    body.validate()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "validation_error", e.to_string()))?;

    let repo = UserRepo::new(state.db_pool.clone());

    // Find user by email
    let user = repo
        .find_by_email(&body.email)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            )
        })?;

    // Verify password
    let password_hash = user.password_hash.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Invalid email or password",
        )
    })?;

    let hasher = PasswordHasherService::new();
    hasher
        .verify_password(&body.password, password_hash)
        .map_err(|_| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            )
        })?;

    // Update last login
    let _ = repo.update_last_login(user.id).await;

    // Issue tokens
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, user.org_id, &user.email, &user.role)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let (refresh_token, _refresh_jti) = state
        .jwt
        .issue_refresh(user.id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900, // 15 minutes
        user: MeResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.display_name.unwrap_or_else(|| "User".to_string()),
            role: user.role.clone(),
            permissions: gateway_auth::permissions_for_role(
                gateway_auth::Role::from_str(&user.role).unwrap_or(gateway_auth::Role::Viewer),
            )
            .iter()
            .map(|p| format!("{:?}", p))
            .collect(),
        },
    }))
}

/// POST /v1/auth/logout
pub async fn logout() -> impl IntoResponse {
    // Client is responsible for discarding tokens.
    // In a full implementation, add the refresh token JTI to a Redis blocklist.
    Json(json!({ "status": "ok" }))
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /v1/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let claims = state
        .jwt
        .verify_refresh(&body.refresh_token)
        .map_err(|e| match e {
            gateway_auth::AuthError::TokenExpired => {
                ApiError::new(StatusCode::UNAUTHORIZED, "token_expired", "Refresh token expired")
            }
            _ => ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "Invalid refresh token"),
        })?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid_token", "Invalid token subject")
    })?;

    let repo = UserRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, user.org_id, &user.email, &user.role)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let (refresh_token, _refresh_jti) = state
        .jwt
        .issue_refresh(user.id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900,
        user: MeResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.display_name.unwrap_or_else(|| "User".to_string()),
            role: user.role.clone(),
            permissions: gateway_auth::permissions_for_role(
                gateway_auth::Role::from_str(&user.role).unwrap_or(gateway_auth::Role::Viewer),
            )
            .iter()
            .map(|p| format!("{:?}", p))
            .collect(),
        },
    }))
}

/// GET /v1/auth/me
pub async fn me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<MeResponse>, ApiError> {
    let user_id = auth.user_id.ok_or_else(|| {
        ApiError::new(StatusCode::UNAUTHORIZED, "unauthenticated", "Not authenticated")
    })?;

    let repo = UserRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    Ok(Json(MeResponse {
        id: user.id.to_string(),
        email: user.email,
        name: user.display_name.unwrap_or_else(|| "User".to_string()),
        role: user.role.clone(),
        permissions: gateway_auth::permissions_for_role(
            gateway_auth::Role::from_str(&user.role).unwrap_or(gateway_auth::Role::Viewer),
        )
        .iter()
        .map(|p| format!("{:?}", p))
        .collect(),
    }))
}
