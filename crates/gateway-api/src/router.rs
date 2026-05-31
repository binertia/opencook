//! Axum router with middleware stack.

use axum::{
    http::Method,
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
    routes::{chat, health, models},
    state::AppState,
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
        .route("/ready", get(health::readiness_check));

    // API routes (auth required in production; currently open for MVP)
    let api_routes = Router::new()
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/models", get(models::list_models));

    Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .layer(trace)
        .layer(body_limit)
        .layer(cors)
        .with_state(state)
}
