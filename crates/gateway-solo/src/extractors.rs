//! Custom Axum extractors for SOLO mode.

use axum::{extract::FromRequest, Json};
use validator::Validate;

use crate::error::ApiError;

/// Json extractor that validates the deserialized value.
pub struct ValidatedJson<T>(pub T);

#[async_trait::async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: Validate + serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        req: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| ApiError::new("json_parse_error", e.to_string()))?;
        value.validate().map_err(ApiError::from)?;
        Ok(ValidatedJson(value))
    }
}
