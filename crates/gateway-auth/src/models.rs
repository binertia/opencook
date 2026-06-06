//! Authentication data models and request/response types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Authentication context attached to requests after successful validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub auth_type: AuthType,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub key_id: Option<Uuid>,
    pub role: Option<String>,
    pub permissions: Vec<String>,
    pub rate_limit_rps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthType {
    ApiKey,
    Session,
}

/// User registration request.
#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
    #[validate(length(max = 128, message = "Display name must be at most 128 characters"))]
    pub display_name: Option<String>,
    #[validate(length(
        min = 1,
        max = 128,
        message = "Organization name must be 1-128 characters"
    ))]
    pub organization_name: String,
}

/// User registration response.
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub status: String,
}

/// Login request.
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

/// Login response with tokens.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// API key creation request.
#[derive(Debug, Deserialize, Validate)]
pub struct ApiKeyCreateRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// API key creation response (full key shown exactly once).
#[derive(Debug, Serialize)]
pub struct ApiKeyCreateResponse {
    pub id: Uuid,
    pub name: String,
    pub key_full: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
