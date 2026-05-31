//! Provider trait definition.

use async_trait::async_trait;
use gateway_core::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};

use crate::error::ProviderError;

/// Provider health status.
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Unified LLM provider interface.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &str;

    /// Models supported by this provider instance.
    fn supported_models(&self) -> Vec<String>;

    /// Non-streaming chat completion.
    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    /// Streaming chat completion (returns SSE stream).
    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<
        axum::response::sse::Sse<
            tokio_stream::wrappers::ReceiverStream<Result<axum::response::sse::Event, ProviderError>>,
        >,
        ProviderError,
    >;

    /// Embedding request.
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError>;

    /// Health check.
    async fn health_check(&self) -> HealthStatus;
}
