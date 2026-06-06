//! Gateway Providers — Provider trait and adapters for OpenAI, Anthropic, Gemini, Ollama.

pub mod anthropic;
pub mod error;
pub mod factory;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod traits;

pub use error::ProviderError;
pub use factory::{create_provider, ProviderConfig, ProviderKind};
pub use traits::{HealthStatus, Provider};
