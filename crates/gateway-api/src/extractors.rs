//! Custom Axum extractors.
//!
//! `ValidatedJson<T>` — deserializes JSON and runs `validator::Validate`.

use axum::{
    extract::{rejection::JsonRejection, FromRequest},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use validator::Validate;

/// A JSON extractor that also validates the deserialized value.
///
/// Usage: `ValidatedJson(body): ValidatedJson<ChatCompletionRequest>`
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

#[async_trait::async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: Validate + serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ValidationRejection;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(ValidationRejection::Json)?;
        value.validate().map_err(ValidationRejection::Validation)?;
        Ok(ValidatedJson(value))
    }
}

/// Rejection type for validation errors.
#[derive(Debug)]
pub enum ValidationRejection {
    Json(JsonRejection),
    Validation(validator::ValidationErrors),
}

impl IntoResponse for ValidationRejection {
    fn into_response(self) -> Response {
        match self {
            ValidationRejection::Json(j) => j.into_response(),
            ValidationRejection::Validation(errors) => {
                let body = ValidationErrorBody::from(errors);
                (StatusCode::BAD_REQUEST, Json(body)).into_response()
            }
        }
    }
}

/// Structured validation error response.
#[derive(Debug, Serialize)]
pub struct ValidationErrorBody {
    pub code: &'static str,
    pub message: String,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub code: String,
}

impl From<validator::ValidationErrors> for ValidationErrorBody {
    fn from(errors: validator::ValidationErrors) -> Self {
        let mut field_errors = Vec::new();

        for (field, err_list) in errors.field_errors() {
            for err in err_list {
                let message = err
                    .message
                    .as_ref()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| format!("Validation failed for '{}'", field));

                field_errors.push(FieldError {
                    field: field.to_string(),
                    message,
                    code: err.code.to_string(),
                });
            }
        }

        ValidationErrorBody {
            code: "validation_error",
            message: "One or more fields failed validation".to_string(),
            errors: field_errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use validator::Validate;

    #[derive(Debug, Clone, Deserialize, Validate)]
    #[allow(dead_code)]
    struct TestDto {
        #[validate(length(min = 1, max = 10))]
        name: String,
        #[validate(email)]
        email: String,
        #[validate(range(min = 1, max = 100))]
        age: i32,
    }

    #[test]
    fn test_validation_error_body_conversion() {
        let mut errors = validator::ValidationErrors::new();
        errors.add("name", validator::ValidationError::new("length"));
        errors.add("email", validator::ValidationError::new("email"));

        let body = ValidationErrorBody::from(errors);
        assert_eq!(body.code, "validation_error");
        assert_eq!(body.errors.len(), 2);
    }
}
