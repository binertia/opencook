//! Axum router with middleware stack.

use axum::{
    http::Method,
    middleware,
    routing::{get, post, put},
    Router,
};
use tower_cookies::CookieManagerLayer;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::{self, TraceLayer},
};
use tracing::Level;

use crate::{
    middleware::audit_context::audit_context_middleware,
    middleware::auth::auth_middleware,
    middleware::auth_rate_limit::auth_rate_limit_middleware,
    middleware::connections::ConnectionLayer,
    middleware::csrf::csrf_middleware,
    middleware::error_handler::ErrorHandlerLayer,
    middleware::rate_limit::rate_limit_middleware,
    middleware::security_headers::SecurityHeadersLayer,
    middleware::timing::TimingLayer,
    routes::{
        analytics, api_keys, audit, auth, cache, chat, dashboard, health, metrics, models,
        organizations, providers, quotas, requests, routing, scim, sso, usage, users, webhooks,
    },
    state::AppState,
    static_files::build_static_router,
};

/// Build the application router with middleware stack.
pub fn build_router(state: AppState) -> Router {
    let is_production = state.config.environment == "production";

    // CORS layer: explicit origins required in production.
    let allowed_origins: Vec<axum::http::HeaderValue> = if state.config.allowed_origins.is_empty() {
        if is_production {
            tracing::error!(
                "GATEWAY_ALLOWED_ORIGINS must be set in production. \
                 CORS will deny all cross-origin requests."
            );
            vec![]
        } else {
            vec![axum::http::HeaderValue::from_static("*")]
        }
    } else {
        state
            .config
            .allowed_origins
            .iter()
            .filter_map(|o| axum::http::HeaderValue::from_str(o).ok())
            .collect()
    };

    let has_wildcard = allowed_origins
        .iter()
        .any(|h| h == axum::http::HeaderValue::from_static("*"));
    if is_production && has_wildcard {
        tracing::error!("Wildcard '*' is not allowed in GATEWAY_ALLOWED_ORIGINS in production.");
    }

    let cors = if allowed_origins.is_empty() {
        // Deny all cross-origin requests (safe default for production misconfig)
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .max_age(std::time::Duration::from_secs(86400))
    } else if has_wildcard && !is_production {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(Any)
            .max_age(std::time::Duration::from_secs(86400))
    } else {
        CorsLayer::new()
            .allow_origin(allowed_origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(Any)
            .allow_credentials(true)
            .max_age(std::time::Duration::from_secs(86400))
    };

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

    // SCIM routes (SCIM token auth, no standard auth)
    let scim_routes = Router::new()
        .route(
            "/scim/v2/ServiceProviderConfig",
            get(scim::service_provider_config),
        )
        .route("/scim/v2/ResourceTypes", get(scim::resource_types))
        .route("/scim/v2/Schemas", get(scim::schemas))
        .route(
            "/scim/v2/Users",
            get(scim::list_users).post(scim::create_user),
        )
        .route(
            "/scim/v2/Users/:user_id",
            get(scim::get_user)
                .put(scim::update_user)
                .patch(scim::patch_user)
                .delete(scim::delete_user),
        )
        .route("/scim/v2/Groups", get(scim::list_groups))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            scim::scim_auth_middleware,
        ));

    // Auth routes: public, but with strict per-IP rate limiting.
    let auth_routes = Router::new()
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/forgot-password", post(auth::forgot_password))
        .route("/v1/auth/reset-password", post(auth::reset_password))
        // SSO public endpoints (IdP callbacks must not require authentication)
        .route("/api/v1/auth/sso/providers", get(sso::list_sso_providers))
        .route("/api/v1/auth/saml/authorize", get(sso::saml_authorize))
        .route("/api/v1/auth/saml/acs", post(sso::saml_acs))
        .route("/api/v1/auth/oidc/authorize", get(sso::oidc_authorize))
        .route("/api/v1/auth/oidc/callback", get(sso::oidc_callback))
        .layer(middleware::from_fn_with_state(
            state.redis.clone(),
            auth_rate_limit_middleware,
        ));

    // Standard API routes (auth + rate limit required, no CSRF)
    let api_routes = Router::new()
        // Auth routes requiring authentication
        .route("/v1/auth/me", get(auth::me))
        .route("/v1/auth/switch-org", post(auth::switch_org))
        // Organization routes
        .route(
            "/v1/organizations",
            post(organizations::create_organization),
        )
        // Dashboard
        .route("/v1/dashboard", get(dashboard::get_dashboard))
        // User routes
        .route("/v1/users", get(users::list_users).post(users::create_user))
        .route(
            "/v1/users/:user_id",
            put(users::update_user).delete(users::delete_user),
        )
        // API key routes
        .route(
            "/v1/api-keys",
            get(api_keys::list_api_keys).post(api_keys::create_api_key),
        )
        .route(
            "/v1/api-keys/:key_id",
            put(api_keys::update_api_key).delete(api_keys::delete_api_key),
        )
        // Analytics routes
        .route("/v1/analytics", get(analytics::get_analytics))
        .route("/v1/analytics/keys", get(analytics::get_key_usage))
        // Request logs
        .route("/v1/requests", get(requests::list_requests))
        // Webhook routes
        .route(
            "/v1/webhooks",
            get(webhooks::list_webhooks).post(webhooks::create_webhook),
        )
        .route(
            "/v1/webhooks/:webhook_id",
            get(webhooks::get_webhook)
                .put(webhooks::update_webhook)
                .delete(webhooks::delete_webhook),
        )
        .route(
            "/v1/webhooks/:webhook_id/deliveries",
            get(webhooks::list_webhook_deliveries),
        )
        .route(
            "/v1/webhooks/:webhook_id/deliveries/:delivery_id/retry",
            post(webhooks::retry_webhook_delivery),
        )
        // Provider routes
        .route(
            "/v1/providers",
            get(providers::list_providers).post(providers::create_provider),
        )
        .route("/v1/providers/test", post(providers::test_connection))
        .route(
            "/v1/providers/:provider_id",
            get(providers::get_provider)
                .put(providers::update_provider)
                .delete(providers::delete_provider),
        )
        .route(
            "/v1/providers/:provider_id/health",
            get(providers::get_provider_health).post(providers::trigger_health_check),
        )
        .route(
            "/v1/providers/:provider_id/health-history",
            get(providers::get_health_history),
        )
        .route(
            "/v1/providers/:provider_id/test",
            post(providers::test_existing_connection),
        )
        // Chat and models
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/models", get(models::list_models))
        .route("/v1/models/:model_id", get(models::get_model))
        .route(
            "/v1/providers/:provider_id/models/:model_id/pricing",
            put(models::update_pricing),
        )
        // Routing rules
        .route(
            "/api/v1/routing-rules",
            get(routing::list_rules).post(routing::create_rule),
        )
        .route(
            "/api/v1/routing-rules/:rule_id",
            get(routing::get_rule)
                .put(routing::update_rule)
                .delete(routing::delete_rule),
        )
        // Cache stats
        .route("/api/v1/cache/stats", get(cache::get_cache_stats))
        .route(
            "/api/v1/cache/semantic-stats",
            get(cache::get_semantic_cache_stats),
        )
        .layer(middleware::from_fn_with_state(
            state.redis.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Admin API routes (auth + rate limit + CSRF required)
    let admin_api_routes = Router::new()
        // Quota admin routes
        .route(
            "/api/v1/organizations/:org_id/quotas",
            get(quotas::list_quotas).post(quotas::create_quota),
        )
        .route(
            "/api/v1/organizations/:org_id/quotas/:quota_id",
            get(quotas::get_quota)
                .put(quotas::update_quota)
                .delete(quotas::delete_quota),
        )
        // Usage analytics routes
        .route("/api/v1/organizations/:org_id/usage", get(usage::get_usage))
        .route("/api/v1/organizations/:org_id/costs", get(usage::get_costs))
        // Audit log routes
        .route(
            "/api/v1/organizations/:org_id/audit-log",
            get(audit::list_audit_entries),
        )
        .route(
            "/api/v1/organizations/:org_id/audit-log/:entry_id",
            get(audit::get_audit_entry),
        )
        // SSO admin routes
        .route(
            "/api/v1/organizations/:org_id/sso",
            get(sso::get_sso_config).post(sso::create_sso_config),
        )
        .route(
            "/api/v1/organizations/:org_id/sso/:provider_type",
            axum::routing::delete(sso::delete_sso_config),
        )
        .layer(middleware::from_fn(csrf_middleware))
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
        .merge(auth_routes)
        .merge(api_routes)
        .merge(admin_api_routes)
        .merge(scim_routes)
        .layer(CookieManagerLayer::default())
        .layer(trace)
        .layer(body_limit)
        .layer(axum::middleware::from_fn(audit_context_middleware))
        .layer(TimingLayer)
        .layer(ConnectionLayer)
        .layer(ErrorHandlerLayer)
        .layer(SecurityHeadersLayer)
        .layer(cors)
        .with_state(state)
}
