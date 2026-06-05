//! HTTP error responses for the gateway API.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Unified API error type.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    pub error_type: String,
    pub param: Option<String>,
    pub request_id: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            error_type: error_type_from_status(status),
            param: None,
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = request_id.into();
        self
    }

    /// Internal message for logging (PII-redacted but detailed).
    pub fn internal_message(&self) -> String {
        gateway_observability::redaction::redact(&self.message)
    }

    /// Client-facing message: generic for 5xx, redacted for 4xx.
    fn client_message(&self) -> String {
        if self.status.is_server_error() {
            "An internal error occurred. Please try again later.".to_string()
        } else {
            gateway_observability::redaction::redact(&self.message)
        }
    }
}

fn error_type_from_status(status: StatusCode) -> String {
    match status {
        StatusCode::BAD_REQUEST => "invalid_request_error",
        StatusCode::UNAUTHORIZED => "authentication_error",
        StatusCode::FORBIDDEN => "permission_error",
        StatusCode::NOT_FOUND => "not_found_error",
        StatusCode::TOO_MANY_REQUESTS => "rate_limit_error",
        StatusCode::INTERNAL_SERVER_ERROR => "gateway_error",
        _ => "gateway_error",
    }
    .to_string()
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let client_msg = self.client_message();
        let body = Json(json!({
            "error": {
                "code": self.code,
                "message": client_msg,
                "type": self.error_type,
                "param": self.param,
                "status": self.status.as_u16(),
                "request_id": self.request_id,
            }
        }));
        (self.status, body).into_response()
    }
}

impl From<gateway_auth::AuthError> for ApiError {
    fn from(err: gateway_auth::AuthError) -> Self {
        let status = StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        ApiError::new(status, "auth_error", err.to_string())
    }
}

impl From<gateway_providers::ProviderError> for ApiError {
    fn from(err: gateway_providers::ProviderError) -> Self {
        let status = StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        ApiError::new(status, "provider_error", err.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            err.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_5xx_generic() {
        let err = ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            "Connection to postgres://db:5432 failed",
        );
        let msg = err.client_message();
        assert!(!msg.contains("postgres"));
        assert_eq!(msg, "An internal error occurred. Please try again later.");
    }

    #[test]
    fn test_client_message_4xx_redacted() {
        let key = "sk_gw_abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let err = ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            format!("Invalid API key: {}", key),
        );
        let msg = err.client_message();
        assert!(!msg.contains(key));
        assert!(msg.contains("[REDACTED:api_key]"));
    }

    #[test]
    fn test_internal_message_redacted() {
        let err = ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "error",
            "User admin@example.com failed auth",
        );
        let msg = err.internal_message();
        assert!(!msg.contains("admin@example.com"));
        assert!(msg.contains("[REDACTED:email]"));
    }

    #[test]
    fn test_error_response_body() {
        let err = ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "Missing field: email",
        );
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
