//! Provider factory — creates Provider instances from configuration.

use crate::{
    anthropic::AnthropicProvider,
    error::ProviderError,
    gemini::GeminiProvider,
    ollama::OllamaProvider,
    openai::OpenAiProvider,
    traits::Provider,
};

/// Provider kind discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
    Qwen,
    Kimi,
    Tencent,
    Groq,
    Mistral,
    Cohere,
    Azure,
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
        ProviderKind::OpenAi
        | ProviderKind::Qwen
        | ProviderKind::Kimi
        | ProviderKind::Tencent
        | ProviderKind::Groq
        | ProviderKind::Mistral
        | ProviderKind::Cohere
        | ProviderKind::Azure => Ok(Box::new(OpenAiProvider::new(config)?)),
        ProviderKind::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        ProviderKind::Gemini => Ok(Box::new(GeminiProvider::new(config)?)),
        ProviderKind::Ollama => Ok(Box::new(OllamaProvider::new(config)?)),
        _ => Err(ProviderError::Config(format!(
            "Provider {:?} not yet implemented",
            config.kind
        ))),
    }
}
