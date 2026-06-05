//! Authentication routes — login, logout, refresh, current user, and org switching.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use gateway_auth::{AuthContext, PasswordHasherService};
use gateway_db::{
    models::AuditAction,
    OrgMemberRepo, UserRepo,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    middleware::csrf::{generate_token, set_csrf_cookie},
    state::AppState,
    validation::sanitize_input,
};
use tower_cookies::Cookies;

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserOrgSummary {
    pub org_id: String,
    pub org_name: String,
    pub slug: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub organizations: Vec<UserOrgSummary>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub csrf_token: String,
    pub user: MeResponse,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SwitchOrgRequest {
    #[validate(length(min = 1, message = "Organization ID is required"))]
    pub org_id: String,
}

#[derive(Debug, Serialize)]
pub struct SwitchOrgResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub csrf_token: String,
    pub user: MeResponse,
}

// ── Helpers ──────────────────────────────────────────────────────────

async fn build_user_response(
    user: &gateway_db::models::User,
    org_repo: &OrgMemberRepo,
) -> Result<MeResponse, ApiError> {
    let orgs = org_repo
        .list_orgs_for_user(user.id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let organizations = orgs
        .into_iter()
        .map(|(org, role)| UserOrgSummary {
            org_id: org.id.to_string(),
            org_name: org.name,
            slug: org.slug,
            role,
        })
        .collect();

    Ok(MeResponse {
        id: user.id.to_string(),
        email: user.email.clone(),
        name: user.display_name.clone().unwrap_or_else(|| "User".to_string()),
        role: user.role.clone(),
        permissions: gateway_auth::permissions_for_role(
            gateway_auth::Role::from_str(&user.role).unwrap_or(gateway_auth::Role::Viewer),
        )
        .iter()
        .map(|p| format!("{:?}", p))
        .collect(),
        organizations,
    })
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /v1/auth/login
pub async fn login(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuditRequestContext>,
    cookies: Cookies,
    ValidatedJson(body): ValidatedJson<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());

    // Sanitize inputs
    let email = sanitize_input(&body.email).to_lowercase();
    let password = sanitize_input(&body.password);

    // Find user by email
    let user = repo
        .find_by_email(&email)
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
        .verify_password(&password, password_hash)
        .map_err(|_| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            )
        })?;

    // Update last login
    let _ = repo.update_last_login(user.id).await;

    // Determine active org: use user's legacy org_id for backward compatibility.
    // In a full multi-org flow, the frontend may present an org picker if the
    // user belongs to more than one organization.
    let active_org_id = user.org_id;

    // Issue tokens
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, active_org_id, &user.email, &user.role)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let (refresh_token, _refresh_jti) = state
        .jwt
        .issue_refresh(user.id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    // Record successful login audit event.
    let auth = AuthContext {
        auth_type: gateway_auth::AuthType::Session,
        org_id: active_org_id,
        user_id: Some(user.id),
        key_id: None,
        role: Some(user.role.clone()),
        permissions: vec![],
        rate_limit_rps: None,
    };
    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::Login,
        "user",
        Some(&user.id.to_string()),
        None,
        Some(json!({"email": user.email.clone() })),
        "User logged in",
    )
    .await;

    let csrf_token = generate_token();
    let secure_cookie = state.config.tls_cert.is_some();
    set_csrf_cookie(&cookies, &csrf_token, secure_cookie);

    let user_resp = build_user_response(&user, &org_repo).await?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900, // 15 minutes
        csrf_token,
        user: user_resp,
    }))
}

/// POST /v1/auth/logout
pub async fn logout() -> impl IntoResponse {
    // Client is responsible for discarding tokens.
    // In a full implementation, add the refresh token JTI to a Redis blocklist.
    Json(json!({ "status": "ok" }))
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1, message = "Refresh token is required"))]
    pub refresh_token: String,
}

/// POST /v1/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    cookies: Cookies,
    ValidatedJson(body): ValidatedJson<RefreshRequest>,
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
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    // Preserve legacy org_id as active org for backward compatibility.
    let active_org_id = user.org_id;

    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, active_org_id, &user.email, &user.role)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let (refresh_token, _refresh_jti) = state
        .jwt
        .issue_refresh(user.id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let csrf_token = generate_token();
    let secure_cookie = state.config.tls_cert.is_some();
    set_csrf_cookie(&cookies, &csrf_token, secure_cookie);

    let user_resp = build_user_response(&user, &org_repo).await?;

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900,
        csrf_token,
        user: user_resp,
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
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    let user_resp = build_user_response(&user, &org_repo).await?;
    Ok(Json(user_resp))
}

/// POST /v1/auth/switch-org
///
/// Switches the user's active organization. Requires that the user is a
/// verified member of the target organization. Issues a new access token
/// with the target org as `active_org_id`.
pub async fn switch_org(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    cookies: Cookies,
    ValidatedJson(body): ValidatedJson<SwitchOrgRequest>,
) -> Result<Json<SwitchOrgResponse>, ApiError> {
    let user_id = auth.user_id.ok_or_else(|| {
        ApiError::new(StatusCode::UNAUTHORIZED, "unauthenticated", "Not authenticated")
    })?;

    let target_org_id = Uuid::parse_str(&body.org_id).map_err(|_| {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid_org_id", "Invalid organization ID format")
    })?;

    // SECURITY: Verify the user is actually a member of the target org.
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let membership = org_repo
        .get_membership(user_id, target_org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::FORBIDDEN,
                "org_access_denied",
                "You do not have access to this organization",
            )
        })?;

    let user_repo = UserRepo::new(state.db_pool.clone());
    let user = user_repo
        .find_by_id(user_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    // Issue new tokens scoped to the target organization.
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, target_org_id, &user.email, &membership.role)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    let (refresh_token, _refresh_jti) = state
        .jwt
        .issue_refresh(user.id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "token_error", e.to_string()))?;

    // Audit log the organization switch.
    let switch_auth = AuthContext {
        auth_type: gateway_auth::AuthType::Session,
        org_id: target_org_id,
        user_id: Some(user.id),
        key_id: None,
        role: Some(membership.role.clone()),
        permissions: vec![],
        rate_limit_rps: None,
    };
    audit::record(
        &state,
        &switch_auth,
        &ctx,
        AuditAction::Update,
        "user",
        Some(&user.id.to_string()),
        Some(json!({ "previous_org_id": auth.org_id.to_string() })),
        Some(json!({ "new_org_id": target_org_id.to_string() })),
        "User switched active organization",
    )
    .await;

    let csrf_token = generate_token();
    let secure_cookie = state.config.tls_cert.is_some();
    set_csrf_cookie(&cookies, &csrf_token, secure_cookie);

    let user_resp = build_user_response(&user, &org_repo).await?;

    Ok(Json(SwitchOrgResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900,
        csrf_token,
        user: user_resp,
    }))
}
