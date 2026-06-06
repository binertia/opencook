//! Authentication routes — login, logout, refresh, current user, org switching,
//! password reset, and account lockout.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use gateway_auth::{AuthContext, PasswordHasherService};
use gateway_db::{models::AuditAction, OrgMemberRepo, UserRepo};
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
    #[validate(length(min = 1, message = "Refresh token is required"))]
    pub refresh_token: String,
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

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1, message = "Reset token is required"))]
    pub token: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

// ── Helpers ──────────────────────────────────────────────────────────

async fn build_user_response(
    user: &gateway_db::models::User,
    org_repo: &OrgMemberRepo,
) -> Result<MeResponse, ApiError> {
    let orgs = org_repo.list_orgs_for_user(user.id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

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
        name: user
            .display_name
            .clone()
            .unwrap_or_else(|| "User".to_string()),
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

/// Check per-email login rate limit (10 attempts per hour via Redis).
async fn check_login_rate_limit(state: &AppState, email: &str) -> Result<(), ApiError> {
    let key = format!("rate_limit:login:{}", email);
    let mut conn = state.redis.clone();

    let count: i64 = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap_or(0);

    if count >= 10 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_exceeded",
            "Too many login attempts. Please try again later.",
        ));
    }

    let _: () = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap_or(());

    let _: i32 = redis::cmd("EXPIRE")
        .arg(&key)
        .arg(3600)
        .query_async(&mut conn)
        .await
        .unwrap_or(0);

    Ok(())
}

/// Record a failed login attempt. If 5 consecutive failures, lock the account for 30 minutes.
async fn record_failed_login(
    repo: &UserRepo,
    user: &gateway_db::models::User,
) -> Result<(), ApiError> {
    let attempts = repo.increment_failed_login(user.id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    if attempts >= 5 {
        repo.lock_account(user.id, 30).await.map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;
    }

    Ok(())
}

// ── Token blocklist helpers ──────────────────────────────────────────

const BLOCKLIST_TTL_SEC: i64 = 604_800; // 7 days, matching refresh token expiry

async fn blocklist_refresh_token(state: &AppState, token: &str) -> Result<(), ApiError> {
    let claims = state.jwt.verify_refresh(token).map_err(|e| match e {
        gateway_auth::AuthError::TokenExpired => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "token_expired",
            "Refresh token expired",
        ),
        _ => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid refresh token",
        ),
    })?;

    let mut conn = state.redis.clone();
    let key = format!("auth:blocklist:refresh:{}", claims.jti);
    let _: () = redis::cmd("SETEX")
        .arg(&key)
        .arg(BLOCKLIST_TTL_SEC)
        .arg("1")
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    Ok(())
}

async fn is_refresh_token_blocklisted(state: &AppState, jti: &str) -> Result<bool, ApiError> {
    let mut conn = state.redis.clone();
    let key = format!("auth:blocklist:refresh:{jti}");
    let exists: bool = redis::cmd("EXISTS")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;
    Ok(exists)
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /v1/auth/login
pub async fn login(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuditRequestContext>,
    cookies: Cookies,
    headers: axum::http::HeaderMap,
    ValidatedJson(body): ValidatedJson<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());

    // Sanitize inputs
    let email = sanitize_input(&body.email).to_lowercase();
    let password = sanitize_input(&body.password);

    // Rate limit login attempts per email.
    check_login_rate_limit(&state, &email).await?;

    // Find user by email
    let user = repo
        .find_by_email(&email)
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
                "invalid_credentials",
                "Invalid email or password",
            )
        })?;

    // Check account lockout.
    if let Some(locked_until) = user.locked_until {
        let now = chrono::Utc::now();
        if locked_until > now {
            return Err(ApiError::new(
                StatusCode::LOCKED,
                "account_locked",
                format!("Account is locked until {}", locked_until.to_rfc3339()),
            ));
        }
        // Lock has expired; the failed_login_attempts counter will be reset on successful login.
    }

    // Verify password
    let password_hash = user.password_hash.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Invalid email or password",
        )
    })?;

    let hasher = PasswordHasherService::new();
    if hasher.verify_password(&password, password_hash).is_err() {
        // Record failed attempt and possibly lock account.
        let _ = record_failed_login(&repo, &user).await;

        // Audit log failed login attempt.
        let auth = AuthContext {
            auth_type: gateway_auth::AuthType::Session,
            org_id: user.org_id,
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
            AuditAction::SecurityViolation,
            "user",
            Some(&user.id.to_string()),
            None,
            Some(json!({"email": user.email.clone(), "reason": "invalid_password" })),
            "Failed login attempt",
        )
        .await;

        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Invalid email or password",
        ));
    }

    // Successful login: reset failed attempts.
    repo.reset_failed_login(user.id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    // Update last login
    let _ = repo.update_last_login(user.id).await;

    // Determine active org: use user's legacy org_id for backward compatibility.
    let active_org_id = user.org_id;

    // Issue tokens
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, active_org_id, &user.email, &user.role)
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token_error",
                e.to_string(),
            )
        })?;

    let (refresh_token, _refresh_jti) = state.jwt.issue_refresh(user.id).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "token_error",
            e.to_string(),
        )
    })?;

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
    let secure_cookie = state.config.secure_cookie(Some(&headers));
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

#[derive(Debug, Deserialize, Validate)]
pub struct LogoutRequest {
    #[validate(length(min = 1, message = "Refresh token is required"))]
    pub refresh_token: String,
}

async fn blocklist_access_token(state: &AppState, token: &str) -> Result<(), ApiError> {
    let claims = state.jwt.verify_access(token).map_err(|e| match e {
        gateway_auth::AuthError::TokenExpired => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "token_expired",
            "Access token expired",
        ),
        _ => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid access token",
        ),
    })?;

    let mut conn = state.redis.clone();
    let key = format!("auth:blocklist:access:{}", claims.jti);
    let _: () = redis::cmd("SETEX")
        .arg(&key)
        .arg(900) // 15 minutes, matching access token expiry
        .arg("1")
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    Ok(())
}

/// POST /v1/auth/logout
///
/// Revokes both the access token (from Authorization header) and the
/// provided refresh token by adding their JTIs to a Redis blocklist.
pub async fn logout(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthContext>,
    headers: axum::http::HeaderMap,
    ValidatedJson(body): ValidatedJson<LogoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Blocklist the access token from the Authorization header
    if let Some(auth_header) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ").or_else(|| auth_header.strip_prefix("bearer ")) {
            let _ = blocklist_access_token(&state, token).await;
        }
    }
    // Blocklist the refresh token
    blocklist_refresh_token(&state, &body.refresh_token).await?;
    Ok(Json(json!({ "status": "ok" })))
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
            gateway_auth::AuthError::TokenExpired => ApiError::new(
                StatusCode::UNAUTHORIZED,
                "token_expired",
                "Refresh token expired",
            ),
            _ => ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "Invalid refresh token",
            ),
        })?;

    // Check if the refresh token has been revoked (e.g., via logout).
    if is_refresh_token_blocklisted(&state, &claims.jti).await? {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "token_revoked",
            "Refresh token has been revoked",
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid token subject",
        )
    })?;

    let repo = UserRepo::new(state.db_pool.clone());
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    // Preserve legacy org_id as active org for backward compatibility.
    let active_org_id = user.org_id;

    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, active_org_id, &user.email, &user.role)
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token_error",
                e.to_string(),
            )
        })?;

    let (refresh_token, _refresh_jti) = state.jwt.issue_refresh(user.id).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "token_error",
            e.to_string(),
        )
    })?;

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
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Not authenticated",
        )
    })?;

    let repo = UserRepo::new(state.db_pool.clone());
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
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
    headers: axum::http::HeaderMap,
    ValidatedJson(body): ValidatedJson<SwitchOrgRequest>,
) -> Result<Json<SwitchOrgResponse>, ApiError> {
    let user_id = auth.user_id.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Not authenticated",
        )
    })?;

    let target_org_id = Uuid::parse_str(&body.org_id).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_org_id",
            "Invalid organization ID format",
        )
    })?;

    // SECURITY: Verify the user is actually a member of the target org.
    let org_repo = OrgMemberRepo::new(state.db_pool.clone());
    let membership = org_repo
        .get_membership(user_id, target_org_id)
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
                StatusCode::FORBIDDEN,
                "org_access_denied",
                "You do not have access to this organization",
            )
        })?;

    let user_repo = UserRepo::new(state.db_pool.clone());
    let user = user_repo
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    // SECURITY: Revoke the old refresh token before issuing a new one.
    blocklist_refresh_token(&state, &body.refresh_token).await?;

    // Issue new tokens scoped to the target organization.
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, target_org_id, &user.email, &membership.role)
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token_error",
                e.to_string(),
            )
        })?;

    let (refresh_token, _refresh_jti) = state.jwt.issue_refresh(user.id).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "token_error",
            e.to_string(),
        )
    })?;

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
    let secure_cookie = state.config.secure_cookie(Some(&headers));
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

/// POST /v1/auth/forgot-password
///
/// Generates a secure password reset token and stores it in Redis with a 1-hour TTL.
/// If email service is configured, sends a password reset email.
pub async fn forgot_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<ForgotPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let email = sanitize_input(&body.email).to_lowercase();
    let repo = UserRepo::new(state.db_pool.clone());

    let user = match repo.find_by_email(&email).await {
        Ok(Some(u)) => u,
        Ok(None) | Err(_) => {
            // SECURITY: Return success even if email not found to prevent user enumeration.
            return Ok(Json(json!({ "status": "ok" })));
        }
    };

    // Generate a 32-byte random token (64 hex chars).
    use rand::Rng;
    let token_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen::<u8>()).collect();
    let token = hex::encode(&token_bytes);

    // Store token in Redis with 1-hour TTL.
    let mut conn = state.redis.clone();
    let redis_key = format!("password_reset:{}", token);
    let _: () = redis::cmd("SETEX")
        .arg(&redis_key)
        .arg(3600)
        .arg(user.id.to_string())
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    // Send email if configured.
    if let Some(ref email_svc) = state.email {
        let reset_url = format!(
            "{}/reset-password?token={}",
            state
                .config
                .allowed_origins
                .first()
                .unwrap_or(&"".to_string()),
            token
        );
        if let Err(e) = email_svc.send_password_reset(&user.email, &reset_url).await {
            tracing::warn!(error = %e, user_id = %user.id, "Failed to send password reset email");
        }
    } else {
        tracing::info!(user_id = %user.id, "Email not configured; password reset token generated but not sent");
    }

    // Audit log.
    let auth = AuthContext {
        auth_type: gateway_auth::AuthType::Session,
        org_id: user.org_id,
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
        AuditAction::Update,
        "user",
        Some(&user.id.to_string()),
        None,
        None,
        "Password reset requested",
    )
    .await;

    // Always return generic success to prevent enumeration.
    Ok(Json(json!({ "status": "ok" })))
}

/// POST /v1/auth/reset-password
///
/// Validates a password reset token from Redis, updates the user's password,
/// and clears the token (single-use).
pub async fn reset_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = sanitize_input(&body.token);
    let new_password = sanitize_input(&body.password);

    // Validate password strength.
    gateway_auth::validate_password_strength(&new_password)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "weak_password", e.to_string()))?;

    // Look up token in Redis.
    let mut conn = state.redis.clone();
    let redis_key = format!("password_reset:{}", token);
    let user_id_str: Option<String> = redis::cmd("GET")
        .arg(&redis_key)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    let user_id_str = user_id_str.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Invalid or expired reset token",
        )
    })?;

    let user_id = Uuid::parse_str(&user_id_str).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid_token",
            "Invalid user ID in token",
        )
    })?;

    let repo = UserRepo::new(state.db_pool.clone());
    let user = repo
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "user_not_found", "User not found")
        })?;

    // SECURITY: New password must not match old password.
    if let Some(ref old_hash) = user.password_hash {
        let hasher = PasswordHasherService::new();
        if hasher.verify_password(&new_password, old_hash).is_ok() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "same_password",
                "New password cannot be the same as the old password",
            ));
        }
    }

    // Hash new password.
    let hasher = PasswordHasherService::new();
    let new_hash = hasher.hash_password(&new_password).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "hash_error",
            e.to_string(),
        )
    })?;

    // Update password and clear lockout/failed attempts.
    repo.update_password(user_id, &new_hash)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    // Delete token from Redis (single-use).
    let _: () = redis::cmd("DEL")
        .arg(&redis_key)
        .query_async(&mut conn)
        .await
        .unwrap_or(());

    // Audit log.
    let auth = AuthContext {
        auth_type: gateway_auth::AuthType::Session,
        org_id: user.org_id,
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
        AuditAction::Update,
        "user",
        Some(&user.id.to_string()),
        None,
        None,
        "Password reset completed",
    )
    .await;

    Ok(Json(json!({ "status": "ok" })))
}
