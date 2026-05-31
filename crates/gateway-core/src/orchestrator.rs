//! Request orchestrator — coordinates the full request lifecycle.
//!
//! Pipeline: Parse → Auth → Rate Limit → Quota Check → Provider Call →
//!           Record Usage → Respond

use gateway_auth::AuthContext;
use gateway_db::{DbBackend, QuotaRepo, QuotaUsageRepo, RequestRepo};
use gateway_quota::{QuotaEngine, QuotaMetric, QuotaResult, RequestContext};
use rust_decimal::Decimal;
use tracing::{debug, error, info, warn};

use crate::types::{ChatCompletionRequest, ChatCompletionResponse};

/// Hardcoded pricing for common models (fallback until TASK-0024 model registry).
/// Prices in USD per 1M tokens.
pub fn model_pricing(model: &str) -> (f64, f64) {
    // (input_cost_per_1m, output_cost_per_1m)
    match model {
        "gpt-4o" | "gpt-4o-2024-05-13" => (5.00, 15.00),
        "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => (0.15, 0.60),
        "gpt-4-turbo" => (10.00, 30.00),
        "gpt-3.5-turbo" => (0.50, 1.50),
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
    provider_call: ProviderCall,
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
        provider: "openai".to_string(), // TODO: routing
        estimated_tokens: estimated_total_tokens,
        estimated_cost,
    };

    let quota_warning = match engine.check_quota(&context).await {
        QuotaResult::Allowed { remaining, limit } => {
            debug!(remaining = remaining, limit = limit, "Quota allowed");
            None
        }
        QuotaResult::Warning { threshold, remaining } => {
            warn!(threshold = threshold, remaining = remaining, "Quota warning");
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
                    Some(&format!("Quota exceeded for metric '{}'. Limit: {}", metric, limit)),
                    start.elapsed().as_millis() as i32,
                    start.elapsed().as_millis() as i32,
                    false,
                )
                .await;
            return Err(OrchestratorError::QuotaExceeded { metric, limit });
        }
    };

    // ── 4. Call provider ───────────────────────────────────────────────
    let provider_result = provider_call(request.clone()).await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match provider_result {
        Ok(mut response) => {
            // ── 5a. Update request record with success ──────────────────
            let actual_total_tokens = response.usage.total_tokens as u64;
            let actual_prompt_tokens = response.usage.prompt_tokens as u64;
            let actual_completion_tokens = response.usage.completion_tokens as u64;
            let actual_cost =
                calculate_cost(&response.model, actual_prompt_tokens, actual_completion_tokens);

            let input_cost_dec = Decimal::try_from(
                actual_prompt_tokens as f64 * input_price / 1_000_000.0,
            )
            .unwrap_or_default();
            let output_cost_dec = Decimal::try_from(
                actual_completion_tokens as f64 * output_price / 1_000_000.0,
            )
            .unwrap_or_default();
            let total_cost_dec =
                Decimal::try_from(actual_cost).unwrap_or_default();

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
            response.gateway = Some(crate::types::GatewayMetadata {
                provider: "openai".to_string(), // TODO: derive from actual provider
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

    #[error("Database error: {0}")]
    Database(#[from] gateway_db::error::DbError),
}

impl OrchestratorError {
    pub fn http_status(&self) -> u16 {
        match self {
            OrchestratorError::QuotaExceeded { .. } => 403,
            OrchestratorError::Provider(_) => 502,
            OrchestratorError::Database(_) => 500,
        }
    }
}
