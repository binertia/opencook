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
        let body = Json(json!({
            "error": {
                "code": self.code,
                "message": self.message,
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
