//! Axum router for SOLO mode — no auth required.

use axum::{
    http::Method,
    middleware,
    routing::{get, post},
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
    routes::{chat, health, metrics, models, quotas, usage, requests, dashboard, auth_solo},
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
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/models", get(models::list_models))
        // Solo-mode dashboard API stubs
        .route("/api/v1/auth/me", get(auth_solo::me))
        .route("/api/v1/dashboard", get(dashboard::get_dashboard))
        .route("/api/v1/requests", get(requests::list_requests))
        .route("/api/v1/requests/:request_id", get(requests::get_request))
        .route("/api/v1/providers", get(dashboard::list_providers))
        .route("/api/v1/analytics", get(dashboard::get_analytics))
        .route("/api/v1/api-keys", get(dashboard::list_api_keys))
        .route("/api/v1/users", get(dashboard::list_users))
        // Quota management (user-configurable in SOLO mode)
        .route("/api/v1/quotas", get(quotas::list_quotas).post(quotas::create_quota))
        .route("/api/v1/quotas/:quota_id", get(quotas::get_quota).put(quotas::update_quota).delete(quotas::delete_quota))
        // Usage analytics
        .route("/api/v1/usage", get(usage::get_usage))
        .route("/api/v1/costs", get(usage::get_costs))
        .layer(trace)
        .layer(body_limit)
        .layer(middleware::from_fn(timing_middleware))
        .layer(cors)
        .with_state(state)
}
