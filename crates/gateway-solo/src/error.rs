//! Error types for the SOLO API.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Standard API error response.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self.error.code.as_str() {
            "unauthorized" => StatusCode::UNAUTHORIZED,
            "forbidden" => StatusCode::FORBIDDEN,
            "not_found" => StatusCode::NOT_FOUND,
            "validation_error" => StatusCode::UNPROCESSABLE_ENTITY,
            "rate_limit_exceeded" => StatusCode::TOO_MANY_REQUESTS,
            "quota_exceeded" => StatusCode::PAYMENT_REQUIRED,
            "provider_error" | "provider_config_error" => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        (status, Json(self)).into_response()
    }
}

impl From<gateway_db::error::DbError> for ApiError {
    fn from(e: gateway_db::error::DbError) -> Self {
        Self::new("database_error", e.to_string())
    }
}

impl From<gateway_providers::ProviderError> for ApiError {
    fn from(e: gateway_providers::ProviderError) -> Self {
        Self::new("provider_error", e.to_string())
    }
}

impl From<gateway_core::orchestrator::OrchestratorError> for ApiError {
    fn from(e: gateway_core::orchestrator::OrchestratorError) -> Self {
        Self::new("orchestrator_error", e.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("serialization_error", e.to_string())
    }
}

impl From<validator::ValidationErrors> for ApiError {
    fn from(e: validator::ValidationErrors) -> Self {
        Self::new("validation_error", e.to_string())
    }
}
