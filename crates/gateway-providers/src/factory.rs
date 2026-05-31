//! Provider factory — creates Provider instances from configuration.

use crate::{
    error::ProviderError,
    openai::OpenAiProvider,
    traits::Provider,
};

/// Provider kind discriminator.
#[derive(Debug, Clone)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
    Custom,
}

/// Configuration for creating a provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub provider_id: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub timeout_ms: u64,
}

/// Create a Provider from configuration.
pub fn create_provider(config: ProviderConfig) -> Result<Box<dyn Provider>, ProviderError> {
    match config.kind {
        ProviderKind::OpenAi => Ok(Box::new(OpenAiProvider::new(config)?)),
        _ => Err(ProviderError::Config(format!(
            "Provider {:?} not yet implemented",
            config.kind
        ))),
    }
}
