//! Authentication and authorization error types.

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid password format: {0}")]
    InvalidPasswordFormat(String),

    #[error("Registration failed")]
    RegistrationFailed,

    #[error("Duplicate email")]
    DuplicateEmail,

    #[error("Invalid API key format")]
    InvalidApiKeyFormat,

    #[error("API key not found")]
    ApiKeyNotFound,

    #[error("API key revoked")]
    ApiKeyRevoked,

    #[error("API key expired")]
    ApiKeyExpired,

    #[error("IP not allowed")]
    IpNotAllowed,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token revoked")]
    TokenRevoked,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AuthError {
    pub fn http_status(&self) -> u16 {
        match self {
            AuthError::InvalidCredentials => 401,
            AuthError::InvalidPasswordFormat(_) => 400,
            AuthError::RegistrationFailed => 400,
            AuthError::DuplicateEmail => 409,
            AuthError::InvalidApiKeyFormat => 401,
            AuthError::ApiKeyNotFound => 401,
            AuthError::ApiKeyRevoked => 401,
            AuthError::ApiKeyExpired => 401,
            AuthError::IpNotAllowed => 403,
            AuthError::InvalidToken => 401,
            AuthError::TokenExpired => 401,
            AuthError::TokenRevoked => 401,
            AuthError::SessionNotFound => 401,
            AuthError::PermissionDenied => 403,
            AuthError::RateLimitExceeded => 429,
            AuthError::Database(_) => 500,
            AuthError::Internal(_) => 500,
        }
    }
}
