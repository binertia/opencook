//! Chat completion endpoint.

use axum::{
    extract::State,
    http::HeaderMap,
    response::{sse::Event, IntoResponse, Sse, Response},
    Extension, Json,
};
use futures::StreamExt;
use gateway_auth::AuthContext;
use gateway_core::orchestrator::{orchestrate_chat_completion, OrchestratorError};
use gateway_core::types::ChatCompletionRequest;
use gateway_core::LoggingStream;
use gateway_db::RequestRepo;
use gateway_providers::factory::{create_provider, ProviderConfig, ProviderKind};
use tokio_stream::wrappers::ReceiverStream;

use crate::{error::ApiError, state::AppState};

pub async fn chat_completions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    // Extract request ID from headers or generate one
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    if request.stream == Some(true) {
        stream_chat_completions(state, auth, request_id, request).await
    } else {
        non_stream_chat_completions(state, auth, request_id, request).await
    }
}

async fn non_stream_chat_completions(
    state: AppState,
    auth: AuthContext,
    request_id: String,
    request: ChatCompletionRequest,
) -> Result<Response, ApiError> {
    // ── Cache check (before provider call) ─────────────────────────────
    let cache_key = gateway_cache::build_cache_key(&request, auth.org_id);
    let is_cacheable = gateway_cache::is_cacheable(&request, false);

    if is_cacheable {
        if let Some(cached) = state.cache.get(&cache_key.redis_key).await {
            // Cache hit: deserialize, log zero-cost request, return with header
            let mut response: gateway_core::types::ChatCompletionResponse =
                serde_json::from_str(&cached.body)
                    .map_err(|e| ApiError::new(
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "cache_deserialize_error",
                        e.to_string(),
                    ))?;

            response.gateway = Some(gateway_core::types::GatewayMetadata {
                provider: cached.provider.clone(),
                latency_ms: 0,
                cache_hit: Some(true),
                quota_warning: None,
            });

            // Log cache hit to DB (fire-and-forget)
            let db_pool = state.db_pool.clone();
            let org_id = auth.org_id;
            let key_id = auth.key_id;
            let model = request.model.clone();
            let trace_id = request_id.clone();
            tokio::spawn(async move {
                let repo = RequestRepo::new(db_pool);
                let _ = repo.insert(
                    org_id,
                    key_id,
                    &trace_id,
                    "POST",
                    "/v1/chat/completions",
                    Some(&model),
                    serde_json::json!({"x-cache": "HIT"}),
                    None,
                ).await;
            });

            let mut resp = axum::Json(response).into_response();
            resp.headers_mut().insert(
                "x-cache",
                axum::http::HeaderValue::from_static("HIT"),
            );
            return Ok(resp);
        }
    }

    // ── Provider call (cache miss or not cacheable) ────────────────────
    let provider_config = ProviderConfig {
        kind: ProviderKind::OpenAi,
        provider_id: "openai".to_string(),
        base_url: std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string()),
        api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        default_model: request.model.clone(),
        timeout_ms: 30000,
    };

    // Provider call closure
    let provider_call: gateway_core::orchestrator::ProviderCall = Box::new(move |req| {
        let config = provider_config.clone();
        Box::pin(async move {
            // Mock fallback when no API key configured
            if config.api_key.is_empty() {
                return Ok(gateway_core::types::ChatCompletionResponse {
                    id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                    object: "chat.completion".to_string(),
                    created: chrono::Utc::now().timestamp() as u64,
                    model: req.model.clone(),
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
                    gateway: None,
                });
            }

            let provider = match create_provider(config) {
                Ok(p) => p,
                Err(e) => return Err(e.to_string()),
            };

            match provider.chat_completion(req).await {
                Ok(mut resp) => {
                    resp.gateway = Some(gateway_core::types::GatewayMetadata {
                        provider: provider.name().to_string(),
                        latency_ms: 0,
                        cache_hit: Some(false),
                        quota_warning: None,
                    });
                    Ok(resp)
                }
                Err(e) => Err(e.to_string()),
            }
        })
    });

    // Orchestrate
    let response = orchestrate_chat_completion(state.db_pool.clone(), &auth, &request_id, request.clone(), provider_call)
        .await
        .map_err(|e| match e {
            OrchestratorError::QuotaExceeded { metric, limit } => ApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "quota_exceeded",
                format!("Quota exceeded for metric '{}'. Limit: {}", metric, limit),
            ),
            OrchestratorError::Provider(msg) => ApiError::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "provider_error",
                msg,
            ),
            OrchestratorError::Database(err) => ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                err.to_string(),
            ),
        })?;

    // ── Cache store (fire-and-forget) ──────────────────────────────────
    if is_cacheable {
        let cache = state.cache.clone();
        let cache_key = cache_key.redis_key;
        let body = serde_json::to_string(&response).unwrap_or_default();
        let cached = gateway_cache::CachedResponse {
            body,
            provider: response.gateway.as_ref().map(|g| g.provider.clone()).unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: response.usage.prompt_tokens,
            completion_tokens: response.usage.completion_tokens,
            total_tokens: response.usage.total_tokens,
            cached_at: chrono::Utc::now(),
        };
        tokio::spawn(async move {
            cache.insert(cache_key, cached, std::time::Duration::from_secs(3600)).await;
        });
    }

    let mut resp = axum::Json(response).into_response();
    resp.headers_mut().insert(
        "x-cache",
        axum::http::HeaderValue::from_static("MISS"),
    );
    Ok(resp)
}

async fn stream_chat_completions(
    state: AppState,
    auth: AuthContext,
    request_id: String,
    request: ChatCompletionRequest,
) -> Result<Response, ApiError> {
    let _start = std::time::Instant::now();

    // Insert request record (pending)
    let request_repo = RequestRepo::new(state.db_pool.clone());
    let req_body = serde_json::to_string(&request).ok();
    let req_record = request_repo
        .insert(
            auth.org_id,
            auth.key_id,
            &request_id,
            "POST",
            "/v1/chat/completions",
            Some(&request.model),
            serde_json::Value::Object(Default::default()),
            req_body.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            e.to_string(),
        ))?;

    // Build provider config
    let provider_config = ProviderConfig {
        kind: ProviderKind::OpenAi,
        provider_id: "openai".to_string(),
        base_url: std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string()),
        api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        default_model: request.model.clone(),
        timeout_ms: 30000,
    };

    // Estimate tokens for logging
    let estimated_prompt_tokens: u64 = request
        .messages
        .iter()
        .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0) as u64)
        .sum::<u64>()
        / 4
        + 1;
    let estimated_completion_tokens = request.max_tokens.unwrap_or(0) as u64;

    let stream: ReceiverStream<Result<Event, String>> = if provider_config.api_key.is_empty() {
        // Mock streaming response
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, String>>(10);
        tokio::spawn(async move {
            let words = ["This", "is", "a", "mock", "streaming", "response."];
            for word in words {
                let chunk = gateway_core::types::StreamingChunk {
                    id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp() as u64,
                    model: request.model.clone(),
                    choices: vec![gateway_core::types::StreamChoice {
                        index: 0,
                        delta: gateway_core::types::MessageDelta {
                            role: Some(gateway_core::types::MessageRole::Assistant),
                            content: Some(format!("{word} ")),
                        },
                        finish_reason: None,
                    }],
                };
                let data = serde_json::to_string(&chunk).unwrap_or_default();
                if tx.send(Ok(Event::default().data(data))).await.is_err() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
        });
        ReceiverStream::new(rx)
    } else {
        let provider = create_provider(provider_config).map_err(|e| ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "provider_config_error",
            e.to_string(),
        ))?;

        let provider_stream = provider
            .chat_completion_stream(request)
            .await
            .map_err(|e| ApiError::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "provider_error",
                e.to_string(),
            ))?;

        // Map ProviderError → String
        let mapped = provider_stream.map(|item| match item {
            Ok(event) => Ok(event),
            Err(e) => Err(e.to_string()),
        });
        
        // We need a ReceiverStream to pass to LoggingStream
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, String>>(100);
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut mapped = mapped;
            while let Some(item) = mapped.next().await {
                if tx.send(item).await.is_err() {
                    return;
                }
            }
        });
        ReceiverStream::new(rx)
    };

    // Wrap with logging
    let logging_stream = LoggingStream::new(
        stream,
        state.db_pool,
        req_record.id,
        auth.org_id,
        req_record.model_requested.unwrap_or_default(),
        estimated_prompt_tokens,
        estimated_completion_tokens,
    );

    let sse = Sse::new(logging_stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text(""),
        );

    Ok(sse.into_response())
}
