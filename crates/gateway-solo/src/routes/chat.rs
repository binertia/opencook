//! Chat completion endpoint for SOLO mode.
//!
//! No authentication required — all requests use the default org context.

use axum::{
    extract::State,
    http::HeaderMap,
    response::{sse::Event, IntoResponse, Response, Sse},
};
use futures::StreamExt;
use gateway_auth::AuthContext;
use gateway_core::orchestrator::{orchestrate_chat_completion, OrchestratorError};
use gateway_core::types::ChatCompletionRequest;
use gateway_core::LoggingStream;
use gateway_db::repos::routing_repo::RoutingRepo;
use gateway_db::RequestRepo;
use gateway_providers::factory::{create_provider, ProviderConfig, ProviderKind};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::{error::ApiError, extractors::ValidatedJson, state::AppState};

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

fn default_auth() -> AuthContext {
    AuthContext {
        auth_type: gateway_auth::AuthType::ApiKey,
        org_id: uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid"),
        user_id: None,
        key_id: None,
        role: None,
        permissions: vec![],
        rate_limit_rps: None,
    }
}

fn parse_provider_kind(kind: &str) -> Option<ProviderKind> {
    match kind.to_lowercase().as_str() {
        "openai" => Some(ProviderKind::OpenAi),
        "anthropic" => Some(ProviderKind::Anthropic),
        "gemini" => Some(ProviderKind::Gemini),
        "ollama" => Some(ProviderKind::Ollama),
        "qwen" | "alibaba" | "dashscope" => Some(ProviderKind::Qwen),
        "kimi" | "moonshot" => Some(ProviderKind::Kimi),
        "tencent" | "hunyuan" => Some(ProviderKind::Tencent),
        "groq" => Some(ProviderKind::Groq),
        "mistral" => Some(ProviderKind::Mistral),
        "cohere" => Some(ProviderKind::Cohere),
        "azure" => Some(ProviderKind::Azure),
        _ => None,
    }
}

fn build_provider_config(target: &gateway_db::Target) -> Option<ProviderConfig> {
    let kind_str = target.provider_kind.as_deref()?;
    let kind = parse_provider_kind(kind_str)?;

    let (base_url, api_key) = match kind {
        ProviderKind::OpenAi => (
            std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com".to_string()),
            std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Anthropic => (
            std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Gemini => (
            std::env::var("GEMINI_BASE_URL")
                .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string()),
            std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Ollama => (
            std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            String::new(),
        ),
        ProviderKind::Qwen => (
            std::env::var("QWEN_BASE_URL")
                .unwrap_or_else(|_| "https://dashscope.aliyuncs.com/compatible-mode".to_string()),
            std::env::var("QWEN_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Kimi => (
            std::env::var("KIMI_BASE_URL")
                .unwrap_or_else(|_| "https://api.moonshot.cn".to_string()),
            std::env::var("KIMI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Tencent => (
            std::env::var("TENCENT_BASE_URL")
                .unwrap_or_else(|_| "https://hunyuan.tencentcloudapi.com".to_string()),
            std::env::var("TENCENT_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Groq => (
            std::env::var("GROQ_BASE_URL")
                .unwrap_or_else(|_| "https://api.groq.com/openai".to_string()),
            std::env::var("GROQ_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Mistral => (
            std::env::var("MISTRAL_BASE_URL")
                .unwrap_or_else(|_| "https://api.mistral.ai".to_string()),
            std::env::var("MISTRAL_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Cohere => (
            std::env::var("COHERE_BASE_URL")
                .unwrap_or_else(|_| "https://api.cohere.ai/compatibility".to_string()),
            std::env::var("COHERE_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Azure => (
            std::env::var("AZURE_OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://your-resource.openai.azure.com".to_string()),
            std::env::var("AZURE_OPENAI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Custom => return None,
    };

    Some(ProviderConfig {
        kind,
        provider_id: kind_str.to_string(),
        base_url,
        api_key,
        default_model: target.model_id.clone(),
        timeout_ms: 30000,
    })
}

async fn resolve_routing(
    state: &AppState,
    request: &ChatCompletionRequest,
) -> Option<(ProviderConfig, Vec<ProviderConfig>)> {
    let repo = RoutingRepo::new(state.db_pool.clone());
    let rules = match repo
        .get_active_rules(default_auth().org_id, Some(&request.model))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch routing rules");
            return None;
        }
    };

    let decision = gateway_core::router::resolve_with_fallback(request, &rules);

    let primary = build_provider_config(&decision.primary)?;
    let fallbacks: Vec<ProviderConfig> = decision
        .fallback_chain
        .iter()
        .filter_map(build_provider_config)
        .collect();

    Some((primary, fallbacks))
}

fn default_provider_config(request: &ChatCompletionRequest) -> ProviderConfig {
    ProviderConfig {
        kind: ProviderKind::OpenAi,
        provider_id: "openai".to_string(),
        base_url: std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string()),
        api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        default_model: request.model.clone(),
        timeout_ms: 30000,
    }
}

pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let auth = default_auth();

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
    let (primary_config, fallback_configs) = resolve_routing(&state, &request)
        .await
        .unwrap_or_else(|| (default_provider_config(&request), vec![]));

    let cancel_token = CancellationToken::new();

    let provider_call: gateway_core::orchestrator::ProviderCall = {
        let primary = primary_config.clone();
        let fallbacks = fallback_configs.clone();
        let req_model = request.model.clone();
        let circuit_breaker = state.circuit_breaker.clone();
        let cancel_clone = cancel_token.clone();

        Box::new(move |req| {
            let primary = primary.clone();
            let fallbacks = fallbacks.clone();
            let req_model = req_model.clone();
            let cb = circuit_breaker.clone();
            let cancel = cancel_clone.clone();

            Box::pin(async move {
                let configs: Vec<ProviderConfig> =
                    std::iter::once(primary).chain(fallbacks).collect();

                let mut last_error = String::new();

                for (idx, config) in configs.iter().enumerate() {
                    if cancel.is_cancelled() {
                        tracing::warn!(provider = %config.provider_id, attempt = idx, "Request cancelled — aborting fallback chain");
                        break;
                    }

                    let is_primary = idx == 0;
                    let provider_key = config.provider_id.clone();

                    if let Err(e) = cb.check(&provider_key) {
                        tracing::warn!(provider = %config.provider_id, error = %e, "Circuit breaker open");
                        last_error = e.to_string();
                        continue;
                    }

                    // Mock fallback when no API key configured
                    if config.api_key.is_empty() && config.kind != ProviderKind::Ollama {
                        tracing::warn!(provider = %config.provider_id, "No API key configured, using mock response");
                        cb.record_success(&provider_key);
                        return Ok(gateway_core::types::ChatCompletionResponse {
                            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                            object: "chat.completion".to_string(),
                            created: chrono::Utc::now().timestamp() as u64,
                            model: req_model.clone(),
                            choices: vec![gateway_core::types::Choice {
                                index: 0,
                                message: gateway_core::types::Message {
                                    role: gateway_core::types::MessageRole::Assistant,
                                    content: Some(format!(
                                        "[{}] Mock response. Set {}_API_KEY to use a real provider.",
                                        config.provider_id,
                                        config.provider_id.to_uppercase()
                                    )),
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
                            system_fingerprint: None,
                            service_tier: None,
                            gateway: Some(gateway_core::types::GatewayMetadata {
                                provider: config.provider_id.clone(),
                                latency_ms: 0,
                                cache_hit: Some(false),
                                quota_warning: None,
                            }),
                        });
                    }

                    let provider = match create_provider(config.clone()) {
                        Ok(p) => p,
                        Err(e) => {
                            last_error = e.to_string();
                            cb.record_failure(&provider_key);
                            continue;
                        }
                    };

                    match provider.chat_completion(req.clone()).await {
                        Ok(mut resp) => {
                            resp.gateway = Some(gateway_core::types::GatewayMetadata {
                                provider: provider.name().to_string(),
                                latency_ms: 0,
                                cache_hit: Some(false),
                                quota_warning: None,
                            });
                            if !is_primary {
                                tracing::info!(provider = %config.provider_id, "Request served by fallback");
                            }
                            cb.record_success(&provider_key);
                            return Ok(resp);
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            cb.record_failure(&provider_key);
                            continue;
                        }
                    }
                }

                Err(format!("All providers failed. Last error: {}", last_error))
            })
        })
    };

    let start = std::time::Instant::now();
    let response = orchestrate_chat_completion(
        state.db_pool.clone(),
        &auth,
        &request_id,
        request.clone(),
        cancel_token,
        provider_call,
    )
    .await;
    let duration_ms = start.elapsed().as_millis() as f64;

    let response = match response {
        Ok(r) => r,
        Err(OrchestratorError::QuotaExceeded { metric, limit }) => {
            gateway_observability::metrics::record_quota_exceeded(&metric, "org");
            gateway_observability::metrics::record_request(
                &request.model,
                "none",
                "quota_exceeded",
                duration_ms,
            );
            return Err(ApiError::new(
                "quota_exceeded",
                format!("Quota exceeded for metric '{}'. Limit: {}", metric, limit),
            ));
        }
        Err(OrchestratorError::Provider(msg)) => {
            gateway_observability::metrics::record_request(
                &request.model,
                "none",
                "error",
                duration_ms,
            );
            return Err(ApiError::new("provider_error", msg));
        }
        Err(OrchestratorError::Cancelled) => {
            gateway_observability::metrics::record_request(
                &request.model,
                "none",
                "cancelled",
                duration_ms,
            );
            return Err(ApiError::new(
                "request_cancelled",
                "Request cancelled by client disconnect",
            ));
        }
        Err(OrchestratorError::Database(err)) => {
            gateway_observability::metrics::record_request(
                &request.model,
                "none",
                "error",
                duration_ms,
            );
            return Err(ApiError::new("database_error", err.to_string()));
        }
    };

    let provider = response
        .gateway
        .as_ref()
        .map(|g| g.provider.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let model = response.model.clone();
    gateway_observability::metrics::record_request(&model, &provider, "success", duration_ms);
    gateway_observability::metrics::record_tokens(
        &model,
        response.usage.prompt_tokens as u64,
        response.usage.completion_tokens as u64,
    );
    let cost = gateway_core::orchestrator::calculate_cost(
        &model,
        response.usage.prompt_tokens as u64,
        response.usage.completion_tokens as u64,
    );
    gateway_observability::metrics::record_cost(&model, &provider, cost);

    Ok(axum::Json(response).into_response())
}

async fn stream_chat_completions(
    state: AppState,
    auth: AuthContext,
    request_id: String,
    request: ChatCompletionRequest,
) -> Result<Response, ApiError> {
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
        .map_err(|e| ApiError::new("database_error", e.to_string()))?;

    let (provider_config, _fallback_configs) = resolve_routing(&state, &request)
        .await
        .unwrap_or_else(|| (default_provider_config(&request), vec![]));

    let provider_key = provider_config.provider_id.clone();
    if let Err(e) = state.circuit_breaker.check(&provider_key) {
        return Err(ApiError::new("circuit_breaker_open", e.to_string()));
    }

    let estimated_prompt_tokens: u64 = request
        .messages
        .iter()
        .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0) as u64)
        .sum::<u64>()
        / 4
        + 1;
    let estimated_completion_tokens = request.max_tokens.unwrap_or(0) as u64;

    let logging_stream = if provider_config.api_key.is_empty()
        && provider_config.kind != ProviderKind::Ollama
    {
        state.circuit_breaker.record_success(&provider_key);
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, String>>(10);
        let model = request.model.clone();
        tokio::spawn(async move {
            let words = ["This", "is", "a", "mock", "streaming", "response."];
            for word in words {
                let chunk = gateway_core::types::StreamingChunk {
                    id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp() as u64,
                    model: model.clone(),
                    choices: vec![gateway_core::types::StreamChoice {
                        index: 0,
                        delta: gateway_core::types::MessageDelta {
                            role: Some(gateway_core::types::MessageRole::Assistant),
                            content: Some(format!("{word} ")),
                        },
                        finish_reason: None,
                    }],
                    system_fingerprint: None,
                    usage: None,
                };
                let data = serde_json::to_string(&chunk).unwrap_or_default();
                if tx.send(Ok(Event::default().data(data))).await.is_err() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
        });
        let stream = ReceiverStream::new(rx);
        let boxed: std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, String>> + Send>> =
            Box::pin(stream);
        LoggingStream::new(
            boxed,
            state.db_pool,
            req_record.id,
            auth.org_id,
            req_record.model_requested.unwrap_or_default(),
            estimated_prompt_tokens,
            estimated_completion_tokens,
        )
    } else {
        let provider = match create_provider(provider_config) {
            Ok(p) => p,
            Err(e) => {
                state.circuit_breaker.record_failure(&provider_key);
                return Err(ApiError::new("provider_config_error", e.to_string()));
            }
        };

        let provider_stream = match provider.chat_completion_stream(request).await {
            Ok(s) => {
                state.circuit_breaker.record_success(&provider_key);
                s
            }
            Err(e) => {
                state.circuit_breaker.record_failure(&provider_key);
                return Err(ApiError::new("provider_error", e.to_string()));
            }
        };

        let mapped = provider_stream.map(|item| match item {
            Ok(event) => Ok(event),
            Err(e) => Err(e.to_string()),
        });
        let boxed: std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, String>> + Send>> =
            Box::pin(mapped);

        LoggingStream::new(
            boxed,
            state.db_pool,
            req_record.id,
            auth.org_id,
            req_record.model_requested.unwrap_or_default(),
            estimated_prompt_tokens,
            estimated_completion_tokens,
        )
    };

    let sse = Sse::new(logging_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text(""),
    );

    Ok(sse.into_response())
}
