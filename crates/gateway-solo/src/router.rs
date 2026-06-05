//! Axum router for SOLO mode — no auth required.

use axum::{
    http::Method,
    middleware,
    routing::{get, post, put, delete},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::{self, TraceLayer},
};
use tracing::Level;

use crate::{
    middleware::timing::timing_middleware,
    routes::{
        analytics_solo, auth_solo, chat, dashboard, health, metrics, models,
        quotas, requests, routing_solo, usage, webhooks_solo,
    },
    state::AppState,
    static_files::build_static_router,
};

/// Build the application router.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    let body_limit = RequestBodyLimitLayer::new(10 * 1024 * 1024);

    let trace = TraceLayer::new_for_http()
        .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
        .on_response(trace::DefaultOnResponse::new().level(Level::INFO));

    // Static file routes for the React SPA dashboard
    let static_routes = build_static_router::<AppState>();

    Router::new()
        .merge(static_routes)
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route("/metrics", get(metrics::metrics_handler))
        // Core API
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/models", get(models::list_models))
        // Auth (frontend expects /v1/auth/*)
        .route("/v1/auth/login", post(auth_solo::login))
        .route("/v1/auth/logout", post(auth_solo::logout))
        .route("/v1/auth/refresh", post(auth_solo::refresh))
        .route("/v1/auth/me", get(auth_solo::me))
        // Dashboard
        .route("/v1/dashboard", get(dashboard::get_dashboard))
        // Users
        .route("/v1/users", get(dashboard::list_users))
        // API Keys
        .route("/v1/api-keys", get(dashboard::list_api_keys))
        // Providers
        .route("/v1/providers", get(dashboard::list_providers))
        // Analytics
        .route("/v1/analytics", get(dashboard::get_analytics))
        .route("/v1/analytics/keys", get(analytics_solo::get_key_usage))
        // Requests
        .route("/v1/requests", get(requests::list_requests))
        .route("/v1/requests/:request_id", get(requests::get_request))
        // Webhooks
        .route("/v1/webhooks", get(webhooks_solo::list_webhooks).post(webhooks_solo::create_webhook))
        .route("/v1/webhooks/:webhook_id", get(webhooks_solo::get_webhook).put(webhooks_solo::update_webhook).delete(webhooks_solo::delete_webhook))
        .route("/v1/webhooks/:webhook_id/deliveries", get(webhooks_solo::list_deliveries))
        .route("/v1/webhooks/:webhook_id/deliveries/:delivery_id/retry", post(webhooks_solo::retry_delivery))
        // Routing rules
        .route("/v1/routing-rules", get(routing_solo::list_routing_rules))
        // Quota management (also available under /api/v1 for compatibility)
        .route("/api/v1/quotas", get(quotas::list_quotas).post(quotas::create_quota))
        .route("/api/v1/quotas/:quota_id", get(quotas::get_quota).put(quotas::update_quota).delete(quotas::delete_quota))
        // Usage analytics (also available under /api/v1 for compatibility)
        .route("/api/v1/usage", get(usage::get_usage))
        .route("/api/v1/costs", get(usage::get_costs))
        .layer(trace)
        .layer(body_limit)
        .layer(middleware::from_fn(timing_middleware))
        .layer(cors)
        .with_state(state)
}
