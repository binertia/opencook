//! Ollama local model provider adapter.

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

use crate::{
    error::ProviderError,
    factory::ProviderConfig,
    traits::{HealthStatus, Provider},
};

pub struct OllamaProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl OllamaProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 min for local models
            .build()
            .map_err(|e| ProviderError::Config(format!("http client: {e}")))?;
        Ok(Self { client, config })
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "llama3.2".into(),
            "llama3.1".into(),
            "mistral".into(),
            "qwen2.5".into(),
            "phi4".into(),
            "gemma2".into(),
        ]
    }

    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let body = OllamaChatRequest::from_canonical(request)?;
        let url = format!("{}/api/chat", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .headers(self.headers())
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

        let ollama_resp: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

        Ok(ollama_resp.into_canonical(self.name(), &body.model))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ReceiverStream<Result<Event, ProviderError>>, ProviderError> {
        let mut body = OllamaChatRequest::from_canonical(request)?;
        body.stream = true;

        let url = format!("{}/api/chat", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.headers())
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
                            if line.trim().is_empty() {
                                continue;
                            }
                            // Ollama streams NDJSON
                            if tx.send(Ok(Event::default().data(line))).await.is_err() {
                                return;
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
        let url = format!("{}/api/embeddings", self.config.base_url);
        let body = OllamaEmbedRequest {
            model: request.model,
            prompt: match request.input {
                gateway_core::types::EmbeddingInput::String(s) => s,
                gateway_core::types::EmbeddingInput::StringArray(arr) => arr.join(" "),
            },
        };

        let response = self
            .client
            .post(&url)
            .headers(self.headers())
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

        let ollama_resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: vec![gateway_core::types::EmbeddingData {
                object: "embedding".to_string(),
                embedding: ollama_resp.embedding,
                index: 0,
            }],
            model: body.model,
            usage: gateway_core::types::EmbeddingUsage {
                prompt_tokens: 0,
                total_tokens: 0,
            },
        })
    }

    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/api/tags", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(e.to_string()),
        }
    }
}

// ── Ollama-native request/response types ─────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaChatResponse {
    model: String,
    message: OllamaMessage,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OllamaEmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

impl OllamaChatRequest {
    fn from_canonical(req: ChatCompletionRequest) -> Result<Self, ProviderError> {
        let messages: Vec<OllamaMessage> = req
            .messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "assistant".to_string(),
                },
                content: m.content.unwrap_or_default(),
            })
            .collect();

        let stop = req.stop.map(|stop| match stop {
            gateway_core::types::StopSequence::String(s) => vec![s],
            gateway_core::types::StopSequence::Array(arr) => arr,
        });

        let options = OllamaOptions {
            temperature: req.temperature,
            top_p: req.top_p,
            num_predict: req.max_tokens,
            stop,
            seed: req.seed,
        };

        Ok(Self {
            model: req.model,
            messages,
            options: Some(options),
            stream: false,
        })
    }
}

impl OllamaChatResponse {
    fn into_canonical(self, provider_name: &str, model: &str) -> ChatCompletionResponse {
        let finish_reason = self.done_reason.map(|r| match r.as_str() {
            "stop" => "stop".to_string(),
            "length" => "length".to_string(),
            other => other.to_string(),
        });

        let prompt_tokens = self.prompt_eval_count.unwrap_or(0);
        let completion_tokens = self.eval_count.unwrap_or(0);

        ChatCompletionResponse {
            id: format!("ollama-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: model.to_string(),
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: MessageRole::Assistant,
                    content: Some(self.message.content),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                logprobs: None,
                finish_reason,
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            system_fingerprint: None,
            service_tier: None,
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
            model: "llama3.2".to_string(),
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
            seed: Some(42),
            stop: None,
            stream: None,
            temperature: Some(0.5),
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        };

        let ollama = OllamaChatRequest::from_canonical(req).unwrap();
        assert_eq!(ollama.model, "llama3.2");
        assert_eq!(ollama.messages.len(), 2);
        assert_eq!(ollama.messages[0].role, "system");
        assert_eq!(ollama.messages[1].role, "user");
        assert_eq!(ollama.options.as_ref().unwrap().temperature, Some(0.5));
        assert_eq!(ollama.options.as_ref().unwrap().num_predict, Some(100));
        assert_eq!(ollama.options.as_ref().unwrap().seed, Some(42));
    }

    #[test]
    fn test_transform_response() {
        let ollama_resp = OllamaChatResponse {
            model: "llama3.2".to_string(),
            message: OllamaMessage {
                role: "assistant".to_string(),
                content: "Hello there!".to_string(),
            },
            done_reason: Some("stop".to_string()),
            prompt_eval_count: Some(10),
            eval_count: Some(5),
        };

        let canonical = ollama_resp.into_canonical("ollama", "llama3.2");

        assert_eq!(
            canonical.choices[0].message.content,
            Some("Hello there!".to_string())
        );
        assert_eq!(canonical.choices[0].finish_reason, Some("stop".to_string()));
        assert_eq!(canonical.usage.prompt_tokens, 10);
        assert_eq!(canonical.usage.completion_tokens, 5);
        assert_eq!(canonical.usage.total_tokens, 15);
        assert_eq!(canonical.model, "llama3.2");
    }
}
