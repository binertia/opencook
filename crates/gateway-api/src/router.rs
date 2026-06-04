//! Axum router with middleware stack.

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
    middleware::api_key_auth::api_key_auth_middleware,
    middleware::error_handler::ErrorHandlerLayer,
    middleware::rate_limit::rate_limit_middleware,
    middleware::timing::TimingLayer,
    routes::{chat, health, metrics, models, quotas, usage},
    state::AppState,
    static_files::build_static_router,
};

/// Build the application router with middleware stack.
pub fn build_router(state: AppState) -> Router {
    // CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Body limit: 10MB
    let body_limit = RequestBodyLimitLayer::new(10 * 1024 * 1024);

    // Trace layer with request ID and timing
    let trace = TraceLayer::new_for_http()
        .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
        .on_response(trace::DefaultOnResponse::new().level(Level::INFO));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route("/metrics", get(metrics::metrics_handler));

    // API routes (auth + rate limit required)
    // Middleware order (outer → inner): rate_limit → api_key_auth → handler
    // Quota check moved into orchestrator (needs request body for cost estimation)
    let api_routes = Router::new()
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/models", get(models::list_models))
        // Quota admin routes
        .route("/api/v1/organizations/:org_id/quotas", get(quotas::list_quotas).post(quotas::create_quota))
        .route("/api/v1/organizations/:org_id/quotas/:quota_id", get(quotas::get_quota).put(quotas::update_quota).delete(quotas::delete_quota))
        // Usage analytics routes
        .route("/api/v1/organizations/:org_id/usage", get(usage::get_usage))
        .route("/api/v1/organizations/:org_id/costs", get(usage::get_costs))
        .layer(middleware::from_fn_with_state(
            state.redis.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn(api_key_auth_middleware));

    // Static file routes for the React SPA dashboard
    let static_routes = build_static_router::<AppState>();

    Router::new()
        .merge(static_routes)
        .merge(public_routes)
        .merge(api_routes)
        .layer(trace)
        .layer(body_limit)
        .layer(TimingLayer)
        .layer(ErrorHandlerLayer)
        .layer(cors)
        .with_state(state)
}
