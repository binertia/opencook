//! Request orchestrator — coordinates the full request lifecycle.
//!
//! Pipeline: Parse → Auth → Rate Limit → Quota Check → Provider Call →
//!           Record Usage → Respond

use gateway_auth::AuthContext;
use gateway_db::{DbBackend, QuotaRepo, QuotaUsageRepo, RequestRepo};
use gateway_quota::{QuotaEngine, QuotaMetric, QuotaResult, RequestContext};
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::types::{ChatCompletionRequest, ChatCompletionResponse};

/// Hardcoded pricing for common models (fallback until TASK-0024 model registry).
/// Prices in USD per 1M tokens.
pub fn model_pricing(model: &str) -> (f64, f64) {
    // (input_cost_per_1m, output_cost_per_1m)
    match model {
        // OpenAI
        "gpt-5.5" | "gpt-5.5-mini" => (5.00, 15.00),
        "gpt-5" | "gpt-5-mini" => (3.00, 10.00),
        "gpt-4.5-preview" => (30.00, 60.00),
        "gpt-4o" | "gpt-4o-2024-11-20" | "gpt-4o-2024-05-13" => (5.00, 15.00),
        "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => (0.15, 0.60),
        "o1" | "o1-mini" => (15.00, 60.00),
        "o3" => (10.00, 40.00),
        "o3-mini" => (1.10, 4.40),
        "o4-mini" => (1.10, 4.40),
        "gpt-4-turbo" => (10.00, 30.00),
        "gpt-4" => (10.00, 30.00),
        "gpt-3.5-turbo" => (0.50, 1.50),
        // Anthropic
        "claude-4.8-sonnet" | "claude-4.8-opus" => (5.00, 25.00),
        "claude-4.5-sonnet" => (5.00, 25.00),
        "claude-4-opus" => (15.00, 75.00),
        "claude-4-sonnet" => (3.00, 15.00),
        "claude-3-7-sonnet" | "claude-3-7-sonnet-20250219" => (3.00, 15.00),
        "claude-3-5-sonnet" | "claude-3-5-sonnet-20241022" => (3.00, 15.00),
        "claude-3-5-haiku" | "claude-3-5-haiku-20241022" => (0.80, 4.00),
        "claude-3-opus" | "claude-3-opus-20240229" => (15.00, 75.00),
        "claude-3-sonnet" | "claude-3-sonnet-20240229" => (3.00, 15.00),
        "claude-3-haiku" | "claude-3-haiku-20240307" => (0.25, 1.25),
        // Google
        "gemini-2.5-flash" | "gemini-2.5-flash-preview" => (0.15, 0.60),
        "gemini-2.5-pro" | "gemini-2.5-pro-preview" => (1.25, 5.00),
        "gemini-2.0-flash" | "gemini-2.0-flash-lite" | "gemini-2.0-flash-thinking-exp" => {
            (0.10, 0.40)
        }
        "gemini-1.5-flash" | "gemini-1.5-flash-8b" => (0.075, 0.30),
        "gemini-1.5-pro" => (1.25, 5.00),
        // Ollama / local models are free from the gateway's perspective
        m if m.starts_with("llama") => (0.00, 0.00),
        m if m.starts_with("qwen") => (0.00, 0.00),
        m if m.starts_with("mistral") => (0.00, 0.00),
        m if m.starts_with("phi") => (0.00, 0.00),
        m if m.starts_with("gemma") => (0.00, 0.00),
        m if m.starts_with("deepseek") => (0.00, 0.00),
        _ => (1.00, 3.00), // default fallback
    }
}

/// Estimate token count from messages (rough heuristic: 4 chars ≈ 1 token).
fn estimate_tokens_from_messages(messages: &[crate::types::Message]) -> u64 {
    let total_chars: usize = messages
        .iter()
        .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0))
        .sum();
    (total_chars / 4 + 1) as u64
}

/// Calculate actual cost from token usage.
pub fn calculate_cost(model: &str, prompt_tokens: u64, completion_tokens: u64) -> f64 {
    let (input_price, output_price) = model_pricing(model);
    let input_cost = prompt_tokens as f64 * input_price / 1_000_000.0;
    let output_cost = completion_tokens as f64 * output_price / 1_000_000.0;
    input_cost + output_cost
}

/// Provider call closure type.
/// The caller (e.g. gateway-api handler) supplies a closure that performs
/// the actual provider request.  This avoids a circular crate dependency
/// because gateway-core must not depend on gateway-providers.
pub type ProviderCall = Box<
    dyn FnOnce(
            ChatCompletionRequest,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ChatCompletionResponse, String>> + Send>,
        > + Send,
>;

/// Orchestrate a chat completion request.
///
/// 1. Persists request record (pending)
/// 2. Estimates cost from request
/// 3. Checks quota (pre-request)
/// 4. Calls provider via the supplied closure
/// 5. Updates request record with response / error
/// 6. Records quota usage (post-request, non-blocking)
/// 7. Returns enriched response
pub async fn orchestrate_chat_completion(
    db_pool: DbBackend,
    auth: &AuthContext,
    trace_id: &str,
    request: ChatCompletionRequest,
    cancellation: CancellationToken,
    provider_call: ProviderCall,
    provider_id: String,
) -> Result<ChatCompletionResponse, OrchestratorError> {
    let start = std::time::Instant::now();
    let request_repo = RequestRepo::new(db_pool.clone());

    // ── 1. Persist request record (pending) ────────────────────────────
    let request_body = serde_json::to_string(&request).ok();
    let req_record = request_repo
        .insert(
            auth.org_id,
            auth.key_id,
            trace_id,
            "POST",
            "/v1/chat/completions",
            Some(&request.model),
            serde_json::Value::Object(Default::default()),
            request_body.as_deref(),
        )
        .await?;

    // ── 2. Estimate usage ──────────────────────────────────────────────
    let estimated_prompt_tokens = estimate_tokens_from_messages(&request.messages);
    let estimated_completion_tokens = request.max_tokens.unwrap_or(0) as u64;
    let estimated_total_tokens = estimated_prompt_tokens + estimated_completion_tokens;

    let (input_price, output_price) = model_pricing(&request.model);
    let estimated_cost = estimated_prompt_tokens as f64 * input_price / 1_000_000.0
        + estimated_completion_tokens as f64 * output_price / 1_000_000.0;

    debug!(
        org_id = %auth.org_id,
        model = %request.model,
        estimated_tokens = estimated_total_tokens,
        estimated_cost = estimated_cost,
        "Estimated usage"
    );

    // ── 3. Quota check (pre-request) ───────────────────────────────────
    let quota_repo = QuotaRepo::new(db_pool.clone());
    let usage_repo = QuotaUsageRepo::new(db_pool.clone());
    let engine = QuotaEngine::new(quota_repo, usage_repo);

    let context = RequestContext {
        org_id: auth.org_id,
        api_key_id: auth.key_id,
        model: request.model.clone(),
        provider: provider_id.clone(),
        estimated_tokens: estimated_total_tokens,
        estimated_cost,
    };

    let quota_warning = match engine.check_quota(&context).await {
        QuotaResult::Allowed { remaining, limit } => {
            debug!(remaining = remaining, limit = limit, "Quota allowed");
            None
        }
        QuotaResult::Warning {
            threshold,
            remaining,
        } => {
            warn!(
                threshold = threshold,
                remaining = remaining,
                "Quota warning"
            );
            Some(format!("{:.0}% of quota used", threshold * 100.0))
        }
        QuotaResult::Exceeded { metric, limit } => {
            warn!(metric = %metric, limit = limit, "Quota exceeded");
            // Update request record with blocked status before returning error
            let _ = request_repo
                .update_response(
                    req_record.id,
                    auth.org_id,
                    None,
                    0,
                    0,
                    0,
                    Decimal::ZERO,
                    Decimal::ZERO,
                    Decimal::ZERO,
                    "error",
                    Some(403),
                    Some("quota_exceeded"),
                    Some(&format!(
                        "Quota exceeded for metric '{}'. Limit: {}",
                        metric, limit
                    )),
                    start.elapsed().as_millis() as i32,
                    start.elapsed().as_millis() as i32,
                    false,
                )
                .await;
            return Err(OrchestratorError::QuotaExceeded { metric, limit });
        }
    };

    // ── 4. Call provider (with cancellation support) ───────────────────
    let provider_result =
        match crate::cancellation::with_cancellation(&cancellation, provider_call(request.clone()))
            .await
        {
            Ok(result) => result,
            Err(_) => Err("Request cancelled by client disconnect".to_string()),
        };

    let latency_ms = start.elapsed().as_millis() as u64;

    match provider_result {
        Ok(mut response) => {
            // ── 5a. Update request record with success ──────────────────
            let actual_total_tokens = response.usage.total_tokens as u64;
            let actual_prompt_tokens = response.usage.prompt_tokens as u64;
            let actual_completion_tokens = response.usage.completion_tokens as u64;
            let actual_cost = calculate_cost(
                &response.model,
                actual_prompt_tokens,
                actual_completion_tokens,
            );

            let input_cost_dec =
                Decimal::try_from(actual_prompt_tokens as f64 * input_price / 1_000_000.0)
                    .unwrap_or_default();
            let output_cost_dec =
                Decimal::try_from(actual_completion_tokens as f64 * output_price / 1_000_000.0)
                    .unwrap_or_default();
            let total_cost_dec = Decimal::try_from(actual_cost).unwrap_or_default();

            if let Err(e) = request_repo
                .update_response(
                    req_record.id,
                    auth.org_id,
                    Some(&response.model),
                    response.usage.prompt_tokens as i32,
                    response.usage.completion_tokens as i32,
                    response.usage.total_tokens as i32,
                    input_cost_dec,
                    output_cost_dec,
                    total_cost_dec,
                    "success",
                    Some(200),
                    None,
                    None,
                    latency_ms as i32,
                    latency_ms as i32,
                    false,
                )
                .await
            {
                error!(error = %e, "Failed to update request record");
            }

            // ── 6. Post-request: record quota usage (non-blocking) ──────
            let db_pool_clone = db_pool.clone();
            let org_id = auth.org_id;
            let key_id = auth.key_id;
            let model = response.model.clone();

            tokio::spawn(async move {
                let quota_repo = QuotaRepo::new(db_pool_clone.clone());
                let usage_repo = QuotaUsageRepo::new(db_pool_clone);
                let engine = QuotaEngine::new(quota_repo, usage_repo);

                if let Err(e) = engine
                    .record_usage(org_id, key_id, QuotaMetric::Requests, Decimal::from(1))
                    .await
                {
                    error!(error = %e, "Failed to record request quota usage");
                }

                if let Err(e) = engine
                    .record_usage(
                        org_id,
                        key_id,
                        QuotaMetric::Tokens,
                        Decimal::from(actual_total_tokens as i64),
                    )
                    .await
                {
                    error!(error = %e, "Failed to record token quota usage");
                }

                if let Err(e) = engine
                    .record_usage(
                        org_id,
                        key_id,
                        QuotaMetric::CostUsd,
                        Decimal::try_from(actual_cost).unwrap_or_default(),
                    )
                    .await
                {
                    error!(error = %e, "Failed to record cost quota usage");
                }

                info!(
                    org_id = %org_id,
                    model = %model,
                    tokens = actual_total_tokens,
                    cost = actual_cost,
                    "Quota usage recorded"
                );
            });

            // ── 7. Enrich response ──────────────────────────────────────
            // Preserve the actual provider reported by the provider adapter (e.g. after fallback),
            // otherwise use the resolved primary provider id.
            let provider_used = response
                .gateway
                .as_ref()
                .map(|g| g.provider.clone())
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| provider_id.clone());
            response.gateway = Some(crate::types::GatewayMetadata {
                provider: provider_used,
                latency_ms,
                cache_hit: Some(false),
                quota_warning,
            });

            Ok(response)
        }
        Err(provider_err) => {
            // ── 5b. Update request record with error ────────────────────
            if let Err(e) = request_repo
                .update_response(
                    req_record.id,
                    auth.org_id,
                    None,
                    0,
                    0,
                    0,
                    Decimal::ZERO,
                    Decimal::ZERO,
                    Decimal::ZERO,
                    "error",
                    Some(502),
                    Some("provider_error"),
                    Some(&provider_err),
                    latency_ms as i32,
                    latency_ms as i32,
                    false,
                )
                .await
            {
                error!(error = %e, "Failed to update request record with error");
            }

            Err(OrchestratorError::Provider(provider_err))
        }
    }
}

/// Orchestrator error type.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Quota exceeded for metric '{metric}'. Limit: {limit}")]
    QuotaExceeded { metric: String, limit: f64 },

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Request cancelled by client disconnect")]
    Cancelled,

    #[error("Database error: {0}")]
    Database(#[from] gateway_db::error::DbError),
}

impl OrchestratorError {
    pub fn http_status(&self) -> u16 {
        match self {
            OrchestratorError::QuotaExceeded { .. } => 403,
            OrchestratorError::Provider(_) => 502,
            OrchestratorError::Cancelled => 499,
            OrchestratorError::Database(_) => 500,
        }
    }
}
