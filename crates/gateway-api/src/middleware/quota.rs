//! Quota / budget cap middleware.
//!
//! Checks quotas pre-request and rejects with 403 if budget cap is exceeded.
//! Adds X-Quota-Warning header when usage is above warning threshold.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use gateway_auth::AuthContext;
use gateway_db::{QuotaRepo, QuotaUsageRepo};
use gateway_quota::{QuotaEngine, QuotaResult, RequestContext};
use tracing::{debug, warn};

use crate::{error::ApiError, state::AppState};

fn publish_quota_webhook(
    state: &AppState,
    org_id: uuid::Uuid,
    event: gateway_db::WebhookEvent,
    data: serde_json::Value,
) {
    if let Some(ref publisher) = state.webhook_publisher {
        let publisher = publisher.clone();
        tokio::spawn(async move {
            publisher.publish(org_id, event, data).await;
        });
    }
}

/// Quota middleware: checks budget caps and usage quotas.
///
/// Uses `State<AppState>` to access the database pool.
pub async fn quota_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Extract auth context (set by auth_middleware)
    let auth = req
        .extensions()
        .get::<AuthContext>()
        .cloned()
        .unwrap_or_else(|| AuthContext {
            auth_type: gateway_auth::AuthType::ApiKey,
            org_id: uuid::Uuid::nil(),
            user_id: None,
            key_id: None,
            role: None,
            permissions: vec![],
            rate_limit_rps: Some(100),
        });

    let quota_repo = QuotaRepo::new(state.db_pool.clone());
    let usage_repo = QuotaUsageRepo::new(state.db_pool.clone());
    let engine = QuotaEngine::new(quota_repo, usage_repo);

    // Build request context for quota check
    // TODO: Extract model and estimated tokens from request body for accurate checks
    let context = RequestContext {
        org_id: auth.org_id,
        api_key_id: auth.key_id,
        model: "unknown".to_string(),
        provider: "unknown".to_string(),
        estimated_tokens: 0,
        estimated_cost: 0.0,
    };

    let result = engine.check_quota(&context).await;

    match result {
        QuotaResult::Allowed { remaining, limit } => {
            debug!(
                org_id = %auth.org_id,
                remaining = remaining,
                limit = limit,
                "Quota allowed"
            );
            Ok(next.run(req).await)
        }
        QuotaResult::Warning {
            threshold,
            remaining,
        } => {
            warn!(
                org_id = %auth.org_id,
                threshold = threshold,
                remaining = remaining,
                "Quota warning"
            );
            publish_quota_webhook(
                &state,
                auth.org_id,
                gateway_db::WebhookEvent::QuotaWarning,
                serde_json::json!({
                    "threshold": threshold,
                    "remaining": remaining,
                }),
            );
            let mut response = next.run(req).await;
            if let Ok(header_value) = format!(
                "Usage above {}% threshold. Remaining: {}.",
                threshold, remaining
            )
            .parse()
            {
                response
                    .headers_mut()
                    .insert("X-Quota-Warning", header_value);
            }
            Ok(response)
        }
        QuotaResult::Exceeded { metric, limit } => {
            warn!(
                org_id = %auth.org_id,
                metric = %metric,
                limit = limit,
                "Quota exceeded"
            );
            publish_quota_webhook(
                &state,
                auth.org_id,
                gateway_db::WebhookEvent::QuotaExceeded,
                serde_json::json!({
                    "metric": metric,
                    "limit": limit,
                }),
            );
            Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "quota_exceeded",
                format!(
                    "Quota exceeded for metric '{}'. Limit: {}. Please upgrade your plan or wait for the next billing period.",
                    metric, limit
                ),
            ))
        }
    }
}
