//! E2E tests for graceful shutdown and signal handling.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;

use gateway_api::router::build_router;
use gateway_api::routes::health::{readiness_check, HealthResponse};
use gateway_api::shutdown::ShutdownState;
use gateway_api::state::{AppConfig, AppState};
use gateway_cache::TwoTierCache;
use gateway_core::circuit_breaker::{BreakerConfig, CircuitBreaker};
use gateway_db::pool::create_pool;
use tokio::net::TcpListener;

/// Build a minimal AppState for testing.
async fn test_state(shutdown: ShutdownState) -> AppState {
    let db_pool = create_pool(":memory:")
        .await
        .expect("failed to create SQLite pool");

    let redis = redis::Client::open("redis://127.0.0.1:6379")
        .expect("Redis required")
        .get_connection_manager()
        .await
        .expect("Redis connection failed");

    let cache = TwoTierCache::new(redis.clone());
    let circuit_breaker = CircuitBreaker::new(BreakerConfig::default());

    let config = AppConfig {
        port: 0,
        database_url: ":memory:".to_string(),
        redis_url: "redis://127.0.0.1:6379".to_string(),
        jwt_private_key_pem: String::new(),
        jwt_public_key_pem: String::new(),
        gateway_version: "test".to_string(),
        profile: gateway_core::profiles::RoutingProfile::Balanced,
        master_key: [0u8; 32],
        master_key_previous: None,
        semantic_cache_enabled: false,
        semantic_cache_threshold: 0.95,
        embedding_base_url: "https://api.openai.com".to_string(),
        embedding_api_key: String::new(),
        embedding_model: "text-embedding-3-small".to_string(),
        tls_cert: None,
        tls_key: None,
        allowed_origins: vec![],
        environment: "test".to_string(),
        smtp_host: None,
        smtp_port: 587,
        smtp_user: None,
        smtp_password: None,
        smtp_from: None,
    };

    let jwt = Arc::new(gateway_auth::JwtService::from_secret(&[0u8; 32]));

    AppState {
        db_pool,
        redis,
        cache,
        semantic_cache: None,
        pgvector_semantic_cache: None,
        circuit_breaker,
        config: Arc::new(config),
        jwt,
        email: None,
        webhook_publisher: None,
        shutdown,
    }
}

#[tokio::test]
async fn readiness_returns_200_when_healthy() {
    let state = test_state(ShutdownState::new()).await;
    let (status, _body) = readiness_check(State(state)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn readiness_returns_503_when_shutting_down() {
    let shutdown = ShutdownState::new();
    shutdown.shutdown();
    let state = test_state(shutdown).await;
    let (status, body) = readiness_check(State(state)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body.0.status, "not_ready");
    assert_eq!(body.0.reason, Some("shutting down".to_string()));
}

#[tokio::test]
async fn liveness_returns_200_during_shutdown() {
    let response = gateway_api::routes::health::health_check().await;
    assert_eq!(response.0.status, "ok");
}

#[tokio::test]
async fn shutdown_state_can_be_cloned_and_shared() {
    let s1 = ShutdownState::new();
    let s2 = s1.clone();

    assert!(!s1.is_shutting_down());
    assert!(!s2.is_shutting_down());

    s1.shutdown();

    assert!(s1.is_shutting_down());
    assert!(s2.is_shutting_down());
}

/// Spins up a test server with a controllable shutdown handle.
async fn spawn_test_server() -> (SocketAddr, ShutdownState) {
    let db_pool = create_pool(":memory:")
        .await
        .expect("failed to create SQLite pool");

    let redis = redis::Client::open("redis://127.0.0.1:6379")
        .expect("Redis required")
        .get_connection_manager()
        .await
        .expect("Redis connection failed");

    let cache = TwoTierCache::new(redis.clone());
    let circuit_breaker = CircuitBreaker::new(BreakerConfig::default());

    let config = AppConfig {
        port: 0,
        database_url: ":memory:".to_string(),
        redis_url: "redis://127.0.0.1:6379".to_string(),
        jwt_private_key_pem: String::new(),
        jwt_public_key_pem: String::new(),
        gateway_version: "test".to_string(),
        profile: gateway_core::profiles::RoutingProfile::Balanced,
        master_key: [0u8; 32],
        master_key_previous: None,
        semantic_cache_enabled: false,
        semantic_cache_threshold: 0.95,
        embedding_base_url: "https://api.openai.com".to_string(),
        embedding_api_key: String::new(),
        embedding_model: "text-embedding-3-small".to_string(),
        tls_cert: None,
        tls_key: None,
        allowed_origins: vec![],
        environment: "test".to_string(),
        smtp_host: None,
        smtp_port: 587,
        smtp_user: None,
        smtp_password: None,
        smtp_from: None,
    };

    let jwt = Arc::new(gateway_auth::JwtService::from_secret(&[0u8; 32]));
    let shutdown = ShutdownState::new();

    let state = AppState {
        db_pool,
        redis,
        cache,
        semantic_cache: None,
        pgvector_semantic_cache: None,
        circuit_breaker,
        config: Arc::new(config),
        jwt,
        email: None,
        webhook_publisher: None,
        shutdown: shutdown.clone(),
    };

    let app = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test server");
    let addr = listener.local_addr().unwrap();

    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_clone.notified().await;
            })
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, shutdown)
}

#[tokio::test]
async fn server_stops_after_graceful_shutdown() {
    let (addr, shutdown) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Verify server is running
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);

    // Trigger shutdown
    shutdown.shutdown();

    // Wait a bit for the server to stop accepting connections
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Server should no longer be reachable
    let result = client
        .get(format!("http://{}/health", addr))
        .timeout(Duration::from_secs(1))
        .send()
        .await;
    assert!(result.is_err() || result.unwrap().status().is_server_error());
}
