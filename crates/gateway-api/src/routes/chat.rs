//! Chat completion endpoint.

use axum::{
    extract::State,
    http::HeaderMap,
    response::{sse::Event, IntoResponse, Sse, Response},
    Extension,
};
use futures::StreamExt;
use gateway_auth::AuthContext;
use gateway_core::orchestrator::{orchestrate_chat_completion, OrchestratorError};
use gateway_core::types::ChatCompletionRequest;
use gateway_core::LoggingStream;
use gateway_db::RequestRepo;
use gateway_db::repos::{provider_config_repo::ProviderConfigRepo, routing_repo::RoutingRepo};
use gateway_providers::factory::{create_provider, ProviderConfig, ProviderKind};
use tokio_stream::wrappers::ReceiverStream;

use crate::{error::ApiError, extractors::ValidatedJson, state::AppState};

/// Map a provider kind string to the ProviderKind enum.
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

/// Build a ProviderConfig from a DB provider config, decrypting the API key.
async fn build_provider_config_from_db(
    state: &AppState,
    auth: &AuthContext,
    target: &gateway_db::Target,
) -> Option<ProviderConfig> {
    // Look up provider by config ID from the database
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let db_config = match repo.get_by_id(target.provider_config_id, auth.org_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(provider_id = %target.provider_config_id, "Provider config not found in DB");
            return build_provider_config_from_env(target).await;
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch provider config from DB");
            return build_provider_config_from_env(target).await;
        }
    };

    let kind = parse_provider_kind(&db_config.kind)?;
    let base_url = db_config
        .api_base
        .clone()
        .unwrap_or_else(|| default_base_url(&kind));

    let api_key = if db_config.api_key_enc.is_empty() {
        String::new()
    } else {
        gateway_auth::crypto::decrypt(&db_config.api_key_enc, &state.config.master_key)
            .unwrap_or_default()
    };

    Some(ProviderConfig {
        kind,
        provider_id: db_config.id.to_string(),
        base_url,
        api_key,
        default_model: target.model_id.clone(),
        timeout_ms: 30000,
    })
}

/// Fallback: build ProviderConfig from environment variables.
async fn build_provider_config_from_env(target: &gateway_db::Target) -> Option<ProviderConfig> {
    let kind_str = target.provider_kind.as_deref()?;
    let kind = parse_provider_kind(kind_str)?;

    let (base_url, api_key) = match kind {
        ProviderKind::OpenAi => (
            std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".to_string()),
            std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Anthropic => (
            std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Gemini => (
            std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string()),
            std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Ollama => (
            std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string()),
            String::new(),
        ),
        ProviderKind::Qwen => (
            std::env::var("QWEN_BASE_URL").unwrap_or_else(|_| "https://dashscope.aliyuncs.com/compatible-mode".to_string()),
            std::env::var("QWEN_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Kimi => (
            std::env::var("KIMI_BASE_URL").unwrap_or_else(|_| "https://api.moonshot.cn".to_string()),
            std::env::var("KIMI_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Tencent => (
            std::env::var("TENCENT_BASE_URL").unwrap_or_else(|_| "https://hunyuan.tencentcloudapi.com".to_string()),
            std::env::var("TENCENT_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Groq => (
            std::env::var("GROQ_BASE_URL").unwrap_or_else(|_| "https://api.groq.com/openai".to_string()),
            std::env::var("GROQ_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Mistral => (
            std::env::var("MISTRAL_BASE_URL").unwrap_or_else(|_| "https://api.mistral.ai".to_string()),
            std::env::var("MISTRAL_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Cohere => (
            std::env::var("COHERE_BASE_URL").unwrap_or_else(|_| "https://api.cohere.ai/compatibility".to_string()),
            std::env::var("COHERE_API_KEY").unwrap_or_default(),
        ),
        ProviderKind::Azure => (
            std::env::var("AZURE_OPENAI_BASE_URL").unwrap_or_else(|_| "https://your-resource.openai.azure.com".to_string()),
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

fn default_base_url(kind: &ProviderKind) -> String {
    match kind {
        ProviderKind::OpenAi => "https://api.openai.com".to_string(),
        ProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
        ProviderKind::Gemini => "https://generativelanguage.googleapis.com".to_string(),
        ProviderKind::Ollama => "http://localhost:11434".to_string(),
        ProviderKind::Qwen => "https://dashscope.aliyuncs.com/compatible-mode".to_string(),
        ProviderKind::Kimi => "https://api.moonshot.cn".to_string(),
        ProviderKind::Tencent => "https://hunyuan.tencentcloudapi.com".to_string(),
        ProviderKind::Groq => "https://api.groq.com/openai".to_string(),
        ProviderKind::Mistral => "https://api.mistral.ai".to_string(),
        ProviderKind::Cohere => "https://api.cohere.ai/compatibility".to_string(),
        ProviderKind::Azure => "https://your-resource.openai.azure.com".to_string(),
        ProviderKind::Custom => String::new(),
    }
}

/// Resolve routing for a request: query rules, evaluate, return provider configs.
async fn resolve_routing(
    state: &AppState,
    auth: &AuthContext,
    request: &ChatCompletionRequest,
) -> Option<(ProviderConfig, Vec<ProviderConfig>)> {
    let repo = RoutingRepo::new(state.db_pool.clone());
    let rules = match repo.get_active_rules(auth.org_id, Some(&request.model)).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch routing rules");
            return None;
        }
    };

    let decision = gateway_core::router::resolve_with_fallback(request, &rules);

    let primary = build_provider_config_from_db(state, auth, &decision.primary).await?;
    let mut fallbacks: Vec<ProviderConfig> = Vec::new();
    for target in &decision.fallback_chain {
        if let Some(config) = build_provider_config_from_db(state, auth, target).await {
            fallbacks.push(config);
        }
    }

    Some((primary, fallbacks))
}

/// Default provider config when no routing rules match.
/// Tries to find an active provider in the DB first, then falls back to env vars.
async fn default_provider_config(
    state: &AppState,
    auth: &AuthContext,
    request: &ChatCompletionRequest,
) -> ProviderConfig {
    // Try to find an active provider in the DB for this org
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    match repo.list_active_by_org(auth.org_id).await {
        Ok(configs) => {
            for config in configs {
                if let Some(kind) = parse_provider_kind(&config.kind) {
                    let base_url = config.api_base.clone().unwrap_or_else(|| default_base_url(&kind));
                    let api_key = if config.api_key_enc.is_empty() {
                        String::new()
                    } else {
                        gateway_auth::crypto::decrypt(&config.api_key_enc, &state.config.master_key)
                            .unwrap_or_default()
                    };
                    return ProviderConfig {
                        kind,
                        provider_id: config.id.to_string(),
                        base_url,
                        api_key,
                        default_model: request.model.clone(),
                        timeout_ms: 30000,
                    };
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch active providers from DB");
        }
    }

    // Fallback to env vars
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
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    ValidatedJson(request): ValidatedJson<ChatCompletionRequest>,
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
            gateway_observability::metrics::record_cache_hit_l2();
            return Ok(resp);
        }
    }

    // ── Routing: resolve provider config(s) ────────────────────────────
    let (primary_config, fallback_configs) = if let Some(result) = resolve_routing(&state, &auth, &request).await {
        result
    } else {
        (default_provider_config(&state, &auth, &request).await, vec![])
    };

    // ── Provider call (with circuit breaker + retry + fallback) ────────
    let provider_call: gateway_core::orchestrator::ProviderCall = {
        let primary = primary_config.clone();
        let fallbacks = fallback_configs.clone();
        let req_model = request.model.clone();
        let circuit_breaker = state.circuit_breaker.clone();

        Box::new(move |req| {
            let primary = primary.clone();
            let fallbacks = fallbacks.clone();
            let req_model = req_model.clone();
            let cb = circuit_breaker.clone();

            Box::pin(async move {
                let configs: Vec<ProviderConfig> = std::iter::once(primary)
                    .chain(fallbacks.into_iter())
                    .collect();

                let mut last_error = String::new();

                for (idx, config) in configs.iter().enumerate() {
                    let is_primary = idx == 0;
                    let provider_key = config.provider_id.clone(); // e.g. "openai", "anthropic"

                    // Check circuit breaker
                    if let Err(e) = cb.check(&provider_key) {
                        tracing::warn!(provider = %config.provider_id, error = %e, "Circuit breaker open, skipping provider");
                        last_error = e.to_string();
                        continue;
                    }

                    // Mock fallback when no API key configured
                    if config.api_key.is_empty() && config.kind != ProviderKind::Ollama {
                        tracing::warn!(provider = %config.provider_id, "No API key configured, using mock response");
                        cb.record_success(&provider_key); // Mock is "successful"
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

                    // Build provider (may fail)
                    let provider = match create_provider(config.clone()) {
                        Ok(p) => p,
                        Err(e) => {
                            last_error = e.to_string();
                            tracing::warn!(provider = %config.provider_id, error = %last_error, "Failed to create provider");
                            cb.record_failure(&provider_key);
                            continue;
                        }
                    };

                    // Call provider with circuit breaker tracking
                    match provider.chat_completion(req.clone()).await {
                        Ok(mut resp) => {
                            resp.gateway = Some(gateway_core::types::GatewayMetadata {
                                provider: provider.name().to_string(),
                                latency_ms: 0,
                                cache_hit: Some(false),
                                quota_warning: None,
                            });
                            if !is_primary {
                                tracing::info!(provider = %config.provider_id, "Request served by fallback provider");
                            }
                            cb.record_success(&provider_key);
                            return Ok(resp);
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            tracing::warn!(provider = %config.provider_id, error = %last_error, "Provider call failed, trying fallback");
                            cb.record_failure(&provider_key);
                            continue;
                        }
                    }
                }

                Err(format!("All providers failed. Last error: {}", last_error))
            })
        })
    };

    // Orchestrate
    let start = std::time::Instant::now();
    let response = orchestrate_chat_completion(state.db_pool.clone(), &auth, &request_id, request.clone(), provider_call)
        .await;
    let duration_ms = start.elapsed().as_millis() as f64;

    let response = match response {
        Ok(r) => r,
        Err(OrchestratorError::QuotaExceeded { metric, limit }) => {
            gateway_observability::metrics::record_quota_exceeded(&metric, "org");
            gateway_observability::metrics::record_request(&request.model, "none", "quota_exceeded", duration_ms);
            return Err(ApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "quota_exceeded",
                format!("Quota exceeded for metric '{}'. Limit: {}", metric, limit),
            ));
        }
        Err(OrchestratorError::Provider(msg)) => {
            gateway_observability::metrics::record_request(&request.model, "none", "error", duration_ms);
            return Err(ApiError::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "provider_error",
                msg,
            ));
        }
        Err(OrchestratorError::Database(err)) => {
            gateway_observability::metrics::record_request(&request.model, "none", "error", duration_ms);
            return Err(ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                err.to_string(),
            ));
        }
    };

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

    // Record metrics
    let provider = response.gateway.as_ref().map(|g| g.provider.clone()).unwrap_or_else(|| "unknown".to_string());
    let model = response.model.clone();
    let duration_ms = response.gateway.as_ref().map(|g| g.latency_ms as f64).unwrap_or(0.0);
    gateway_observability::metrics::record_request(&model, &provider, "success", duration_ms);
    gateway_observability::metrics::record_tokens(&model, response.usage.prompt_tokens as u64, response.usage.completion_tokens as u64);
    let cost = gateway_core::orchestrator::calculate_cost(&model, response.usage.prompt_tokens as u64, response.usage.completion_tokens as u64);
    gateway_observability::metrics::record_cost(&model, &provider, cost);
    gateway_observability::metrics::record_cache_miss();

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

    // ── Routing: resolve provider config ───────────────────────────────
    let (provider_config, _fallback_configs) = if let Some(result) = resolve_routing(&state, &auth, &request).await {
        result
    } else {
        (default_provider_config(&state, &auth, &request).await, vec![])
    };

    // Circuit breaker check for streaming
    let provider_key = provider_config.provider_id.clone();
    if let Err(e) = state.circuit_breaker.check(&provider_key) {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "circuit_breaker_open",
            e.to_string(),
        ));
    }

    // Estimate tokens for logging
    let estimated_prompt_tokens: u64 = request
        .messages
        .iter()
        .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0) as u64)
        .sum::<u64>()
        / 4
        + 1;
    let estimated_completion_tokens = request.max_tokens.unwrap_or(0) as u64;

    let logging_stream = if provider_config.api_key.is_empty() && provider_config.kind != ProviderKind::Ollama {
        // Mock streaming response
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
        let boxed: std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, String>> + Send>> = Box::pin(stream);
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
                return Err(ApiError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "provider_config_error",
                    e.to_string(),
                ));
            }
        };

        let provider_stream = match provider.chat_completion_stream(request).await {
            Ok(s) => {
                state.circuit_breaker.record_success(&provider_key);
                s
            }
            Err(e) => {
                state.circuit_breaker.record_failure(&provider_key);
                return Err(ApiError::new(
                    axum::http::StatusCode::BAD_GATEWAY,
                    "provider_error",
                    e.to_string(),
                ));
            }
        };

        // Map ProviderError → String and wrap directly in LoggingStream
        let mapped = provider_stream.map(|item| match item {
            Ok(event) => Ok(event),
            Err(e) => Err(e.to_string()),
        });
        let boxed: std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, String>> + Send>> = Box::pin(mapped);
        
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

    let sse = Sse::new(logging_stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text(""),
        );

    Ok(sse.into_response())
}
