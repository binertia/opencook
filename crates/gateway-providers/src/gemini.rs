//! Google Gemini provider adapter.

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
use uuid::Uuid;

use crate::{
    error::ProviderError,
    factory::ProviderConfig,
    traits::{HealthStatus, Provider},
};

pub struct GeminiProvider {
    client: reqwest::Client,
    config: ProviderConfig,
}

impl GeminiProvider {
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
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }

    fn build_url(&self, model: &str, stream: bool) -> String {
        let endpoint = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        format!(
            "{}/v1beta/models/{}:{}?key={}",
            self.config.base_url, model, endpoint, self.config.api_key
        )
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "gemini-2.0-flash".into(),
            "gemini-2.0-flash-lite".into(),
            "gemini-1.5-flash".into(),
            "gemini-1.5-pro".into(),
            "gemini-1.5-flash-8b".into(),
        ]
    }

    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let body = GeminiRequest::from_canonical(&request)?;
        let url = self.build_url(&request.model, false);

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

        let gemini_resp: GeminiResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

        Ok(gemini_resp.to_canonical(self.name(), &request.model))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ReceiverStream<Result<Event, ProviderError>>, ProviderError> {
        let body = GeminiRequest::from_canonical(&request)?;
        let url = self.build_url(&request.model, true);

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
                            if line.trim().is_empty() {
                                continue;
                            }
                            // Gemini streams JSON objects, not SSE format
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
        _request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Gemini embeddings API exists but has different endpoint/model format
        // For MVP, return not-implemented
        Err(ProviderError::Config(
            "Gemini embeddings not yet implemented. Use OpenAI for embeddings.".to_string(),
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = self.build_url("gemini-1.5-flash", false);
        let body = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}],
            "generationConfig": {"maxOutputTokens": 1}
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

// ── Gemini-native request/response types ─────────────────────────────

#[derive(Debug, Serialize)]
struct GeminiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsage {
    prompt_token_count: u32,
    candidates_token_count: u32,
    total_token_count: u32,
}

impl GeminiRequest {
    fn from_canonical(req: &ChatCompletionRequest) -> Result<Self, ProviderError> {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in &req.messages {
            match msg.role {
                MessageRole::System => {
                    if let Some(text) = &msg.content {
                        system_instruction = Some(GeminiContent {
                            role: "user".to_string(),
                            parts: vec![GeminiPart { text: text.clone() }],
                        });
                    }
                }
                MessageRole::User => {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart {
                            text: msg.content.clone().unwrap_or_default(),
                        }],
                    });
                }
                MessageRole::Assistant => {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts: vec![GeminiPart {
                            text: msg.content.clone().unwrap_or_default(),
                        }],
                    });
                }
                MessageRole::Tool => {
                    warn!("Gemini adapter: tool messages not fully supported, converting to user");
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts: vec![GeminiPart {
                            text: msg.content.clone().unwrap_or_default(),
                        }],
                    });
                }
            }
        }

        let stop_sequences = req.stop.as_ref().map(|stop| match stop {
            gateway_core::types::StopSequence::String(s) => vec![s.clone()],
            gateway_core::types::StopSequence::Array(arr) => arr.clone(),
        });

        let generation_config = GeminiGenerationConfig {
            max_output_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            stop_sequences,
        };

        Ok(Self {
            system_instruction,
            contents,
            generation_config: Some(generation_config),
        })
    }
}

impl GeminiResponse {
    fn to_canonical(self, provider_name: &str, model: &str) -> ChatCompletionResponse {
        let candidate = self.candidates.into_iter().next().unwrap_or_else(|| {
            // Fallback empty candidate
            GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart {
                        text: "".to_string(),
                    }],
                },
                finish_reason: Some("STOP".to_string()),
            }
        });

        let text = candidate
            .content
            .parts
            .into_iter()
            .map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = candidate.finish_reason.map(|r| match r.as_str() {
            "STOP" => "stop".to_string(),
            "MAX_TOKENS" => "length".to_string(),
            "SAFETY" => "content_filter".to_string(),
            other => other.to_string(),
        });

        let usage = self.usage_metadata.unwrap_or(GeminiUsage {
            prompt_token_count: 0,
            candidates_token_count: 0,
            total_token_count: 0,
        });

        ChatCompletionResponse {
            id: format!("gemini-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: model.to_string(),
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
                prompt_tokens: usage.prompt_token_count,
                completion_tokens: usage.candidates_token_count,
                total_tokens: usage.total_token_count,
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
            model: "gemini-1.5-flash".to_string(),
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

        let gemini = GeminiRequest::from_canonical(&req).unwrap();
        assert!(gemini.system_instruction.is_some());
        assert_eq!(gemini.contents.len(), 1);
        assert_eq!(gemini.contents[0].role, "user");
        assert_eq!(gemini.contents[0].parts[0].text, "Hello");
        assert_eq!(
            gemini.generation_config.as_ref().unwrap().max_output_tokens,
            Some(100)
        );
        assert_eq!(
            gemini.generation_config.as_ref().unwrap().temperature,
            Some(0.5)
        );
    }

    #[test]
    fn test_transform_response() {
        let raw = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello there!"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        }"#;

        let gemini: GeminiResponse = serde_json::from_str(raw).unwrap();
        let canonical = gemini.to_canonical("gemini", "gemini-1.5-flash");

        assert_eq!(canonical.choices[0].message.content, Some("Hello there!".to_string()));
        assert_eq!(canonical.choices[0].finish_reason, Some("stop".to_string()));
        assert_eq!(canonical.usage.prompt_tokens, 10);
        assert_eq!(canonical.usage.completion_tokens, 5);
        assert_eq!(canonical.usage.total_tokens, 15);
        assert_eq!(canonical.model, "gemini-1.5-flash");
    }
}
