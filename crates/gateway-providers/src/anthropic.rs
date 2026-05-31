//! Anthropic Claude provider adapter.

use async_trait::async_trait;
use axum::response::sse::Event;
use futures::StreamExt;
use gateway_core::types::{
    ChatCompletionRequest, ChatCompletionResponse, Choice, EmbeddingRequest, EmbeddingResponse,
    Message, MessageRole, Usage,
};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;

use crate::{
    error::ProviderError,
    factory::ProviderConfig,
    traits::{HealthStatus, Provider},
};

pub struct AnthropicProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl AnthropicProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|e| ProviderError::Config(format!("http client: {e}")))?;
        Ok(Self { client, config })
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", self.config.api_key.parse().unwrap());
        headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "claude-3-5-sonnet-20241022".into(),
            "claude-3-5-haiku-20241022".into(),
            "claude-3-opus-20240229".into(),
            "claude-3-sonnet-20240229".into(),
            "claude-3-haiku-20240307".into(),
        ]
    }

    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let body = AnthropicRequest::from_canonical(request)?;
        let url = format!("{}/v1/messages", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&body)
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

        let anthropic_resp: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

        Ok(anthropic_resp.to_canonical(self.name()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ReceiverStream<Result<Event, ProviderError>>, ProviderError> {
        let mut body = AnthropicRequest::from_canonical(request)?;
        body.stream = Some(true);

        let url = format!("{}/v1/messages", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&body)
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
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                // Anthropic stream events are wrapped in data: lines
                                if tx.send(Ok(Event::default().data(data))).await.is_err() {
                                    return;
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
        _request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::Config(
            "Anthropic does not provide embeddings. Use OpenAI or a dedicated embedding provider."
                .to_string(),
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Anthropic doesn't have a simple models endpoint that works without auth scope checks
        // Use a minimal messages request as health check
        let url = format!("{}/v1/messages", self.config.base_url);
        let body = serde_json::json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}]
        });
        match self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) if resp.status().as_u16() == 429 => {
                HealthStatus::Degraded("Rate limited (429)".to_string())
            }
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(e.to_string()),
        }
    }
}

// ── Anthropic-native request/response types ──────────────────────────

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContent>,
    model: String,
    #[serde(default)]
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

impl AnthropicRequest {
    fn from_canonical(req: ChatCompletionRequest) -> Result<Self, ProviderError> {
        // Extract system message(s) into the `system` field
        let mut system_parts = Vec::new();
        let mut messages = Vec::new();

        for msg in req.messages {
            match msg.role {
                MessageRole::System => {
                    if let Some(content) = msg.content {
                        system_parts.push(content);
                    }
                }
                MessageRole::User => {
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: msg.content.unwrap_or_default(),
                    });
                }
                MessageRole::Assistant => {
                    messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: msg.content.unwrap_or_default(),
                    });
                }
                MessageRole::Tool => {
                    warn!("Anthropic adapter: tool messages not fully supported, converting to user");
                    messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: msg.content.unwrap_or_default(),
                    });
                }
            }
        }

        let max_tokens = req.max_tokens.unwrap_or(4096);

        // Build stop sequences
        let stop_sequences = req.stop.map(|stop| match stop {
            gateway_core::types::StopSequence::String(s) => vec![s],
            gateway_core::types::StopSequence::Array(arr) => arr,
        });

        Ok(Self {
            model: req.model,
            system: if system_parts.is_empty() {
                None
            } else {
                Some(system_parts.join("\n"))
            },
            messages,
            max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            stop_sequences,
            stream: None,
        })
    }
}

impl AnthropicResponse {
    fn to_canonical(self, provider_name: &str) -> ChatCompletionResponse {
        let text = self
            .content
            .into_iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = self.stop_reason.map(|r| match r.as_str() {
            "end_turn" => "stop".to_string(),
            "max_tokens" => "length".to_string(),
            other => other.to_string(),
        });

        ChatCompletionResponse {
            id: self.id,
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: self.model,
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: MessageRole::Assistant,
                    content: Some(text),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                logprobs: None,
                finish_reason,
            }],
            usage: Usage {
                prompt_tokens: self.usage.input_tokens,
                completion_tokens: self.usage.output_tokens,
                total_tokens: self.usage.input_tokens + self.usage.output_tokens,
            },
            gateway: Some(gateway_core::types::GatewayMetadata {
                provider: provider_name.to_string(),
                latency_ms: 0,
                cache_hit: Some(false),
                quota_warning: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::types::{Message, MessageRole};

    #[test]
    fn test_transform_request() {
        let req = ChatCompletionRequest {
            model: "claude-3-haiku".to_string(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: Some("Be helpful.".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: Some("Hello".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            frequency_penalty: None,
            max_tokens: Some(100),
            n: None,
            presence_penalty: None,
            response_format: None,
            seed: None,
            stop: None,
            stream: None,
            temperature: Some(0.5),
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = AnthropicRequest::from_canonical(req).unwrap();
        assert_eq!(anthropic.model, "claude-3-haiku");
        assert_eq!(anthropic.system, Some("Be helpful.".to_string()));
        assert_eq!(anthropic.messages.len(), 1);
        assert_eq!(anthropic.messages[0].role, "user");
        assert_eq!(anthropic.messages[0].content, "Hello");
        assert_eq!(anthropic.max_tokens, 100);
        assert_eq!(anthropic.temperature, Some(0.5));
    }

    #[test]
    fn test_transform_response() {
        let raw = r#"{
            "id": "msg_01ABC",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello there!"}],
            "model": "claude-3-haiku",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }"#;

        let anthropic: AnthropicResponse = serde_json::from_str(raw).unwrap();
        let canonical = anthropic.to_canonical("anthropic");

        assert_eq!(canonical.id, "msg_01ABC");
        assert_eq!(canonical.choices[0].message.content, Some("Hello there!".to_string()));
        assert_eq!(canonical.choices[0].finish_reason, Some("stop".to_string()));
        assert_eq!(canonical.usage.prompt_tokens, 10);
        assert_eq!(canonical.usage.completion_tokens, 5);
        assert_eq!(canonical.usage.total_tokens, 15);
    }

    #[test]
    fn test_default_max_tokens() {
        let req = ChatCompletionRequest {
            model: "claude-3-haiku".to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: Some("Hi".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            frequency_penalty: None,
            max_tokens: None,
            n: None,
            presence_penalty: None,
            response_format: None,
            seed: None,
            stop: None,
            stream: None,
            temperature: None,
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = AnthropicRequest::from_canonical(req).unwrap();
        assert_eq!(anthropic.max_tokens, 4096);
    }
}
