//! Chat completion endpoint.

use axum::{extract::State, http::HeaderMap, Json};
use gateway_core::types::ChatCompletionRequest;
use gateway_providers::factory::{create_provider, ProviderConfig, ProviderKind};

use crate::{error::ApiError, state::AppState};

pub async fn chat_completions(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<gateway_core::types::ChatCompletionResponse>, ApiError> {
    // Extract request ID from headers or generate one
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // For MVP: always route to OpenAI using a mock config
    // In production, look up provider by model name from registry
    let provider_config = ProviderConfig {
        kind: ProviderKind::OpenAi,
        provider_id: "openai".to_string(),
        base_url: std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string()),
        api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        default_model: request.model.clone(),
        timeout_ms: 30000,
    };

    // Skip if no API key configured (return mock response for testing)
    if provider_config.api_key.is_empty() {
        return Ok(Json(gateway_core::types::ChatCompletionResponse {
            id: format!("chatcmpl-{}", request_id),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: request.model.clone(),
            choices: vec![gateway_core::types::Choice {
                index: 0,
                message: gateway_core::types::Message {
                    role: gateway_core::types::MessageRole::Assistant,
                    content: Some("This is a mock response. Set OPENAI_API_KEY to use a real provider.".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                logprobs: None,
                finish_reason: Some("stop".to_string()),
            }],
            usage: gateway_core::types::Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
            gateway: Some(gateway_core::types::GatewayMetadata {
                provider: "mock".to_string(),
                latency_ms: 0,
                cache_hit: Some(false),
            }),
        }));
    }

    let provider = create_provider(provider_config)
        .map_err(|e| ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "provider_config_error",
            e.to_string(),
        ))?;

    let start = std::time::Instant::now();
    let mut response = provider.chat_completion(request).await
        .map_err(|e| ApiError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            "provider_error",
            e.to_string(),
        ))?;
    let latency_ms = start.elapsed().as_millis() as u64;

    // Attach gateway metadata
    response.gateway = Some(gateway_core::types::GatewayMetadata {
        provider: provider.name().to_string(),
        latency_ms,
        cache_hit: Some(false),
    });

    Ok(Json(response))
}
