//! Axum router with middleware stack.

use axum::{
    http::Method,
    middleware,
    routing::{get, post, put},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::{self, TraceLayer},
};
use tracing::Level;

use crate::{
    middleware::auth::auth_middleware,
    middleware::error_handler::ErrorHandlerLayer,
    middleware::rate_limit::rate_limit_middleware,
    middleware::timing::TimingLayer,
    routes::{auth, chat, dashboard, health, metrics, models, providers, quotas, usage, users},
    state::AppState,
    static_files::build_static_router,
};

/// Build the application router with middleware stack.
pub fn build_router(state: AppState) -> Router {
    // CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
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
    // Middleware order (outer → inner): rate_limit → auth → handler
    // The auth middleware skips /v1/auth/login and /v1/auth/refresh as public routes.
    let api_routes = Router::new()
        // Auth routes
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/me", get(auth::me))
        // Dashboard
        .route("/v1/dashboard", get(dashboard::get_dashboard))
        // User routes
        .route("/v1/users", get(users::list_users).post(users::create_user))
        .route("/v1/users/:user_id", put(users::update_user).delete(users::delete_user))
        // Provider routes
        .route("/v1/providers", get(providers::list_providers).post(providers::create_provider))
        .route("/v1/providers/test", post(providers::test_connection))
        .route("/v1/providers/:provider_id", get(providers::get_provider).put(providers::update_provider).delete(providers::delete_provider))
        .route("/v1/providers/:provider_id/health", get(providers::get_provider_health).post(providers::trigger_health_check))
        .route("/v1/providers/:provider_id/health-history", get(providers::get_health_history))
        .route("/v1/providers/:provider_id/test", post(providers::test_existing_connection))
        // Chat and models
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

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
