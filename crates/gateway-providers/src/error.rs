//! Provider error types.

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ProviderError {
    #[error("HTTP error: {status} — {message}")]
    Http { status: u16, message: String },

    #[error("Request serialization failed: {0}")]
    Serialization(String),

    #[error("Response deserialization failed: {0}")]
    Deserialization(String),

    #[error("Streaming error: {0}")]
    Stream(String),

    #[error("Provider timeout")]
    Timeout,

    #[error("Provider unavailable: {0}")]
    Unavailable(String),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Rate limited by provider")]
    RateLimited,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl ProviderError {
    pub fn http_status(&self) -> u16 {
        match self {
            ProviderError::Http { status, .. } => *status,
            ProviderError::Timeout => 504,
            ProviderError::Unavailable(_) => 502,
            ProviderError::RateLimited => 429,
            _ => 500,
        }
    }
}
