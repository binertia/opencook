//! OpenAI provider adapter (pass-through since canonical format matches OpenAI).

use async_trait::async_trait;
use axum::response::sse::Event;
use futures::StreamExt;
use gateway_core::types::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    StreamingChunk,
};
use reqwest::header::{self, HeaderMap};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    error::ProviderError,
    factory::ProviderConfig,
    traits::{HealthStatus, Provider},
};

pub struct OpenAiProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl OpenAiProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|e| ProviderError::Config(format!("http client: {e}")))?;
        Ok(Self { client, config })
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", self.config.api_key).parse().unwrap(),
        );
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "gpt-5.5".into(),
            "gpt-5.5-mini".into(),
            "gpt-5".into(),
            "gpt-5-mini".into(),
            "gpt-4.5-preview".into(),
            "gpt-4o".into(),
            "gpt-4o-2024-11-20".into(),
            "gpt-4o-mini".into(),
            "o1".into(),
            "o1-mini".into(),
            "o3-mini".into(),
            "o3".into(),
            "o4-mini".into(),
            "chatgpt-4o-latest".into(),
            "gpt-4-turbo".into(),
            "gpt-4".into(),
            "gpt-3.5-turbo".into(),
            "text-embedding-3-small".into(),
            "text-embedding-3-large".into(),
        ]
    }

    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&request)
            .send()
            .await
            .map_err(|e| match e.is_timeout() {
                true => ProviderError::Timeout,
                false => ProviderError::Unavailable(e.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".into());
            return Err(ProviderError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        let mut body: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

        // Attach gateway metadata
        body.gateway = Some(gateway_core::types::GatewayMetadata {
            provider: self.name().to_string(),
            latency_ms: 0, // filled by caller
            cache_hit: Some(false),
            quota_warning: None,
        });

        Ok(body)
    }

    async fn chat_completion_stream(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<ReceiverStream<Result<Event, ProviderError>>, ProviderError> {
        request.stream = Some(true);

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&request)
            .send()
            .await
            .map_err(|e| match e.is_timeout() {
                true => ProviderError::Timeout,
                false => ProviderError::Unavailable(e.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".into());
            return Err(ProviderError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, ProviderError>>(100);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                                    continue;
                                }
                                match serde_json::from_str::<StreamingChunk>(data) {
                                    Ok(_chunk) => {
                                        let event = Event::default().data(data);
                                        if tx.send(Ok(event)).await.is_err() {
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx
                                            .send(Err(ProviderError::Stream(e.to_string())))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(ProviderError::Stream(e.to_string()))).await;
                        return;
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let url = format!("{}/v1/embeddings", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&request)
            .send()
            .await
            .map_err(|e| match e.is_timeout() {
                true => ProviderError::Timeout,
                false => ProviderError::Unavailable(e.to_string()),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".into());
            return Err(ProviderError::Http {
                status: status.as_u16(),
                message: body,
            });
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/v1/models", self.config.base_url);
        match self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(e.to_string()),
        }
    }
}
