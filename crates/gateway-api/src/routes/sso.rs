//! SSO routes — SAML 2.0 and OIDC endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Redirect,
    Extension, Json,
};
use gateway_auth::sso::oidc::OidcProvider;
use gateway_auth::sso::saml::SamlProvider;
use gateway_auth::sso::SsoAuthResult;
use gateway_db::{OrgMemberRepo, SsoConfigRepo, SsoProviderType as DbSsoProviderType, UserRepo};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::ApiError,
    extractors::ValidatedJson,
    middleware::csrf::{generate_token, set_csrf_cookie},
    state::AppState,
};
use gateway_auth::rbac::{check_permission, Permission, Role};
use gateway_auth::AuthContext;
use tower_cookies::Cookies;

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SamlAcsPayload {
    #[serde(rename = "SAMLResponse")]
    pub saml_response: String,
    #[serde(rename = "RelayState")]
    pub relay_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OidcAuthorizeQuery {
    pub org_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct SsoProviderResponse {
    pub provider_type: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSsoConfig {
    #[validate(length(min = 1, max = 32, message = "Provider type must be 1-32 characters"))]
    pub provider_type: String,
    #[validate(url(message = "Metadata URL must be a valid URL"))]
    #[validate(custom(function = "crate::validation::validate_url_not_internal"))]
    pub metadata_url: Option<String>,
    #[validate(length(max = 512, message = "Entity ID must be at most 512 characters"))]
    pub entity_id: Option<String>,
    #[validate(length(max = 4096, message = "Certificate must be at most 4096 characters"))]
    pub certificate: Option<String>,
    #[validate(url(message = "SSO URL must be a valid URL"))]
    #[validate(custom(function = "crate::validation::validate_url_not_internal"))]
    pub sso_url: Option<String>,
    #[validate(length(max = 512, message = "Client ID must be at most 512 characters"))]
    pub client_id: Option<String>,
    #[validate(length(max = 512, message = "Client secret must be at most 512 characters"))]
    pub client_secret: Option<String>,
    #[validate(length(max = 512, message = "IDP issuer must be at most 512 characters"))]
    pub idp_issuer: Option<String>,
    #[validate(length(max = 128, message = "Role attribute must be at most 128 characters"))]
    pub role_attribute: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SsoConfigResponse {
    pub id: String,
    pub org_id: String,
    pub provider_type: String,
    pub metadata_url: Option<String>,
    pub entity_id: Option<String>,
    pub sso_url: Option<String>,
    pub client_id: Option<String>,
    pub idp_issuer: Option<String>,
    pub role_attribute: String,
    pub enabled: bool,
}

// ── Public SSO Providers List ────────────────────────────────────────

pub async fn list_sso_providers(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<SsoProviderResponse>>, ApiError> {
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let configs = repo.list_by_org(org_id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    let providers = configs
        .into_iter()
        .filter(|c| c.enabled)
        .map(|c| SsoProviderResponse {
            provider_type: match c.provider_type {
                DbSsoProviderType::Saml => "saml".to_string(),
                DbSsoProviderType::Oidc => "oidc".to_string(),
            },
            enabled: c.enabled,
        })
        .collect();

    Ok(Json(providers))
}

// ── Permission helper ────────────────────────────────────────────────

fn require_permission(auth: &AuthContext, permission: Permission) -> Result<(), Box<ApiError>> {
    let role = auth
        .role
        .as_deref()
        .and_then(Role::from_str)
        .unwrap_or(Role::Viewer);

    if !check_permission(role, permission) {
        return Err(Box::new(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            "You do not have permission to perform this action",
        )));
    }
    Ok(())
}

// ── SAML Authorization ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SamlAuthorizeQuery {
    pub org_id: Uuid,
}

pub async fn saml_authorize(
    State(state): State<AppState>,
    Query(query): Query<SamlAuthorizeQuery>,
) -> Result<Redirect, ApiError> {
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let config = repo
        .get_by_org_and_type(query.org_id, DbSsoProviderType::Saml)
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
                StatusCode::NOT_FOUND,
                "sso_not_configured",
                "SAML not configured for this organization",
            )
        })?;

    // Generate a cryptographically random RelayState for CSRF protection
    let mut random_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let relay_state = hex::encode(random_bytes);

    // Store relay_state -> org_id mapping in Redis with 10-minute TTL
    let redis_key = format!("sso:saml:relay:{}", relay_state);
    let _: () = redis::cmd("SETEX")
        .arg(&redis_key)
        .arg(600)
        .arg(query.org_id.to_string())
        .query_async(&mut state.redis.clone())
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    let provider = SamlProvider::new(
        config.entity_id.unwrap_or_default(),
        format!(
            "{}/api/v1/auth/saml/acs",
            state
                .config
                .allowed_origins
                .first()
                .unwrap_or(&"http://localhost:8080".to_string())
        ),
        config.sso_url.unwrap_or_default(),
        config.certificate,
        config.role_attribute,
    );

    let url = provider.authn_request_url(&relay_state).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "saml_error",
            e.to_string(),
        )
    })?;

    Ok(Redirect::to(&url))
}

// ── SAML ACS ─────────────────────────────────────────────────────────

pub async fn saml_acs(
    State(state): State<AppState>,
    cookies: Cookies,
    axum::Form(payload): axum::Form<SamlAcsPayload>,
) -> Result<Redirect, ApiError> {
    let relay_state = payload.relay_state.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_relay_state",
            "Missing RelayState",
        )
    })?;

    // Verify RelayState against Redis and atomically delete it (CSRF + replay protection)
    let redis_key = format!("sso:saml:relay:{}", relay_state);
    let org_id_str: Option<String> = redis::cmd("GETDEL")
        .arg(&redis_key)
        .query_async(&mut state.redis.clone())
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    let org_id = match org_id_str {
        Some(s) => Uuid::parse_str(&s).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_relay_state",
                "Invalid RelayState",
            )
        })?,
        None => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_relay_state",
                "RelayState expired or invalid",
            ))
        }
    };

    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let config = repo
        .get_by_org_and_type(org_id, DbSsoProviderType::Saml)
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
                StatusCode::NOT_FOUND,
                "sso_not_configured",
                "SAML not configured for this organization",
            )
        })?;

    let provider = SamlProvider::new(
        config.entity_id.unwrap_or_default(),
        format!(
            "{}/api/v1/auth/saml/acs",
            state
                .config
                .allowed_origins
                .first()
                .unwrap_or(&"http://localhost:8080".to_string())
        ),
        config.sso_url.unwrap_or_default(),
        config.certificate,
        config.role_attribute,
    );

    let result = provider
        .parse_response(&payload.saml_response)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "saml_error", e.to_string()))?;

    let user = provision_user(&state, org_id, &result).await?;

    info!(user_id = %user.id, org_id = %org_id, "SAML login successful");

    issue_sso_session_and_redirect(&state, &cookies, user, org_id, &result.email, &result.role.unwrap_or_else(|| "member".to_string())).await
}

// ── OIDC Authorization ───────────────────────────────────────────────

pub async fn oidc_authorize(
    State(state): State<AppState>,
    Query(query): Query<OidcAuthorizeQuery>,
) -> Result<Redirect, ApiError> {
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let config = repo
        .get_by_org_and_type(query.org_id, DbSsoProviderType::Oidc)
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
                StatusCode::NOT_FOUND,
                "sso_not_configured",
                "OIDC not configured for this organization",
            )
        })?;

    let redirect_uri = format!(
        "{}/api/v1/auth/oidc/callback",
        state
            .config
            .allowed_origins
            .first()
            .unwrap_or(&"http://localhost:8080".to_string())
    );

    let idp_issuer = config.idp_issuer.unwrap_or_default();
    let provider = OidcProvider::new(
        config.client_id.unwrap_or_default(),
        config.client_secret_enc.unwrap_or_default(),
        redirect_uri,
        config.sso_url.unwrap_or_default(),
        config.metadata_url.unwrap_or_default(),
        idp_issuer.clone(),
        idp_issuer,
        config.role_attribute,
    );

    // Generate a cryptographically random state nonce for CSRF protection
    let mut random_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let state_param = hex::encode(random_bytes);

    // Store state -> org_id mapping in Redis with 10-minute TTL
    let redis_key = format!("sso:oidc:state:{}", state_param);
    let _: () = redis::cmd("SETEX")
        .arg(&redis_key)
        .arg(600)
        .arg(query.org_id.to_string())
        .query_async(&mut state.redis.clone())
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    let nonce = Uuid::new_v4().to_string();

    let url = provider.authorization_url(&state_param, &nonce);
    Ok(Redirect::to(&url))
}

// ── OIDC Callback ────────────────────────────────────────────────────

pub async fn oidc_callback(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(query): Query<OidcCallbackQuery>,
) -> Result<Redirect, ApiError> {
    // Verify state parameter against Redis and atomically delete it (CSRF + replay protection)
    let redis_key = format!("sso:oidc:state:{}", query.state);
    let org_id_str: Option<String> = redis::cmd("GETDEL")
        .arg(&redis_key)
        .query_async(&mut state.redis.clone())
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "redis_error",
                e.to_string(),
            )
        })?;

    let org_id = match org_id_str {
        Some(s) => Uuid::parse_str(&s).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_state",
                "Invalid state parameter",
            )
        })?,
        None => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_state",
                "State parameter expired or invalid",
            ))
        }
    };

    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let config = repo
        .get_by_org_and_type(org_id, DbSsoProviderType::Oidc)
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
                StatusCode::NOT_FOUND,
                "sso_not_configured",
                "OIDC not configured for this organization",
            )
        })?;

    let redirect_uri = format!(
        "{}/api/v1/auth/oidc/callback",
        state
            .config
            .allowed_origins
            .first()
            .unwrap_or(&"http://localhost:8080".to_string())
    );

    let client_secret = config
        .client_secret_enc
        .as_deref()
        .and_then(|s| decrypt_sso_secret(s, &state.config.master_key))
        .unwrap_or_default();

    let idp_issuer = config.idp_issuer.unwrap_or_default();
    let provider = OidcProvider::new(
        config.client_id.unwrap_or_default(),
        client_secret,
        redirect_uri,
        config.sso_url.unwrap_or_default(),
        config.metadata_url.unwrap_or_default(),
        idp_issuer.clone(),
        idp_issuer,
        config.role_attribute,
    );

    let result = provider
        .exchange_code(&query.code, &query.state)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "oidc_error", e.to_string()))?;

    let user = provision_user(&state, org_id, &result).await?;

    info!(user_id = %user.id, org_id = %org_id, "OIDC login successful");

    issue_sso_session_and_redirect(&state, &cookies, user, org_id, &result.email, &result.role.unwrap_or_else(|| "member".to_string())).await
}

// ── Admin: SSO Config CRUD ───────────────────────────────────────────

pub async fn get_sso_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<SsoConfigResponse>>, ApiError> {
    require_permission(&auth, Permission::SettingsRead)?;
    if auth.org_id != org_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot access SSO config for another organization",
        ));
    }
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);
    let configs = repo.list_by_org(org_id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    Ok(Json(
        configs
            .into_iter()
            .map(|c| SsoConfigResponse {
                id: c.id.to_string(),
                org_id: c.org_id.to_string(),
                provider_type: match c.provider_type {
                    DbSsoProviderType::Saml => "saml".to_string(),
                    DbSsoProviderType::Oidc => "oidc".to_string(),
                },
                metadata_url: c.metadata_url,
                entity_id: c.entity_id,
                sso_url: c.sso_url,
                client_id: c.client_id,
                idp_issuer: c.idp_issuer,
                role_attribute: c.role_attribute,
                enabled: c.enabled,
            })
            .collect(),
    ))
}

pub async fn create_sso_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
    ValidatedJson(body): ValidatedJson<CreateSsoConfig>,
) -> Result<Json<SsoConfigResponse>, ApiError> {
    require_permission(&auth, Permission::SettingsWrite)?;
    if auth.org_id != org_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot configure SSO for another organization",
        ));
    }
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);

    let provider_type = match body.provider_type.as_str() {
        "saml" => DbSsoProviderType::Saml,
        "oidc" => DbSsoProviderType::Oidc,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_type",
                "Must be 'saml' or 'oidc'",
            ))
        }
    };

    let client_secret_enc = body
        .client_secret
        .as_deref()
        .and_then(|s| encrypt_sso_secret(s, &state.config.master_key))
        .unwrap_or_default();

    let config = gateway_db::SsoConfig {
        id: Uuid::new_v4(),
        org_id,
        provider_type,
        metadata_url: body.metadata_url,
        entity_id: body.entity_id,
        certificate: body.certificate,
        sso_url: body.sso_url,
        client_id: body.client_id,
        client_secret_enc: Some(client_secret_enc),
        idp_issuer: body.idp_issuer,
        role_attribute: body.role_attribute.unwrap_or_else(|| "role".to_string()),
        enabled: body.enabled.unwrap_or(true),
    };

    let saved = repo.upsert(&config).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    Ok(Json(SsoConfigResponse {
        id: saved.id.to_string(),
        org_id: saved.org_id.to_string(),
        provider_type: match saved.provider_type {
            DbSsoProviderType::Saml => "saml".to_string(),
            DbSsoProviderType::Oidc => "oidc".to_string(),
        },
        metadata_url: saved.metadata_url,
        entity_id: saved.entity_id,
        sso_url: saved.sso_url,
        client_id: saved.client_id,
        idp_issuer: saved.idp_issuer,
        role_attribute: saved.role_attribute,
        enabled: saved.enabled,
    }))
}

pub async fn delete_sso_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, provider_type)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    require_permission(&auth, Permission::SettingsWrite)?;
    if auth.org_id != org_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot delete SSO config for another organization",
        ));
    }
    let repo = SsoConfigRepo::new(state.db_pool.clone().into_pg()?);

    let pt = match provider_type.as_str() {
        "saml" => DbSsoProviderType::Saml,
        "oidc" => DbSsoProviderType::Oidc,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_type",
                "Must be 'saml' or 'oidc'",
            ))
        }
    };

    repo.delete(org_id, pt).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ──────────────────────────────────────────────────────────

async fn provision_user(
    state: &AppState,
    org_id: Uuid,
    result: &SsoAuthResult,
) -> Result<gateway_db::User, ApiError> {
    let user_repo = UserRepo::new(state.db_pool.clone());
    let member_repo = OrgMemberRepo::new(state.db_pool.clone());

    // Find or create user by email
    let user = match user_repo.find_by_email(&result.email).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Auto-provision new user
            let new_user = user_repo
                .create_sso_user(org_id, &result.email, result.name.as_deref())
                .await
                .map_err(|e| {
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "database_error",
                        e.to_string(),
                    )
                })?;
            info!(user_id = %new_user.id, email = %result.email, "Auto-provisioned SSO user");
            new_user
        }
        Err(e) => {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            ))
        }
    };

    // Ensure user is linked to org
    let members = member_repo.list_by_org(org_id).await.map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        )
    })?;

    let already_member = members.iter().any(|m| m.0.user_id == user.id);

    if !already_member {
        let role = result.role.as_deref().unwrap_or("member");
        let is_first = members.is_empty();
        let assigned_role = if is_first { "owner" } else { role };

        member_repo
            .create(user.id, org_id, assigned_role, None)
            .await
            .map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    e.to_string(),
                )
            })?;

        info!(user_id = %user.id, org_id = %org_id, role = assigned_role, "Linked SSO user to organization");
    }

    Ok(user)
}

// ── SSO Session Helpers ──────────────────────────────────────────────

/// Issue JWT tokens and set CSRF cookie after successful SSO login,
/// then redirect to the dashboard with tokens in query params for the SPA.
async fn issue_sso_session_and_redirect(
    state: &AppState,
    cookies: &Cookies,
    user: gateway_db::User,
    org_id: Uuid,
    email: &str,
    role: &str,
) -> Result<Redirect, ApiError> {
    let (access_token, _access_jti) = state
        .jwt
        .issue_access(user.id, org_id, email, role)
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
    set_csrf_cookie(cookies, &csrf_token, secure_cookie);

    let dashboard_url = state
        .config
        .allowed_origins
        .first()
        .cloned()
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    let redirect_url = format!(
        "{}/admin/login?sso=1&access_token={}&refresh_token={}&expires_in=900",
        dashboard_url, access_token, refresh_token
    );

    Ok(Redirect::to(&redirect_url))
}

fn encrypt_sso_secret(secret: &str, master_key: &[u8; 32]) -> Option<String> {
    if secret.is_empty() {
        return Some(String::new());
    }
    gateway_auth::crypto::encrypt(secret, master_key)
        .ok()
        .map(hex::encode)
}

fn decrypt_sso_secret(secret_enc: &str, master_key: &[u8; 32]) -> Option<String> {
    if secret_enc.is_empty() {
        return Some(String::new());
    }
    let bytes = hex::decode(secret_enc).ok()?;
    gateway_auth::crypto::decrypt_with_keys(
        &bytes,
        &gateway_auth::ActiveKeyPair::new(*master_key),
    )
    .ok()
}

// Helper to convert DbBackend to PgPool
trait IntoPg {
    fn into_pg(self) -> Result<sqlx::PgPool, Box<ApiError>>;
}

impl IntoPg for gateway_db::DbBackend {
    fn into_pg(self) -> Result<sqlx::PgPool, Box<ApiError>> {
        match self {
            gateway_db::DbBackend::Postgres(pg) => Ok(pg),
            gateway_db::DbBackend::Sqlite(_) => Err(Box::new(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "sqlite_not_supported",
                "SSO requires PostgreSQL",
            ))),
        }
    }
}
