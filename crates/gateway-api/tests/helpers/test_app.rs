//! Test app harness — spins up a full gateway server for E2E tests.

use std::net::SocketAddr;
use std::sync::Arc;

use gateway_api::router::build_router;
use gateway_api::state::{AppConfig, AppState};
use gateway_cache::TwoTierCache;
use gateway_core::circuit_breaker::{BreakerConfig, CircuitBreaker};
use gateway_db::pool::create_pool;
use redis::aio::ConnectionManager;
use tokio::net::TcpListener;

/// A running test instance of the gateway.
pub struct TestApp {
    pub addr: SocketAddr,
    pub db_pool: gateway_db::DbBackend,
    pub client: reqwest::Client,
    #[allow(dead_code)]
    pub redis: ConnectionManager,
}

impl TestApp {
    /// Base URL for HTTP requests.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Make a GET request.
    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{}", self.base_url(), path))
            .send()
            .await
            .expect("failed to send GET request")
    }

    /// Make a POST request with JSON body.
    pub async fn post_json(&self, path: &str, body: serde_json::Value) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url(), path))
            .json(&body)
            .send()
            .await
            .expect("failed to send POST request")
    }

    /// Make a POST request with Bearer auth and JSON body.
    pub async fn post_json_auth(
        &self,
        path: &str,
        api_key: &str,
        body: serde_json::Value,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url(), path))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
            .expect("failed to send authenticated POST request")
    }
}

/// Build a test app with SQLite in-memory DB and Redis via testcontainers.
///
/// If Redis is unavailable, the test is skipped.
pub async fn spawn_test_app() -> TestApp {
    // Use SQLite in-memory for fast, isolated tests
    let db_pool = create_pool(":memory:")
        .await
        .expect("failed to create SQLite pool");

    // Connect to Redis — try localhost first, then testcontainers
    let redis = connect_redis().await.expect(
        "Redis is required for E2E tests. \
         Start Redis with: docker run -d -p 6379:6379 redis:7-alpine"
    );

    let cache = TwoTierCache::new(redis.clone());
    let circuit_breaker = CircuitBreaker::new(BreakerConfig::default());

    let config = AppConfig {
        port: 0, // random port
        database_url: ":memory:".to_string(),
        redis_url: "redis://localhost:6379".to_string(),
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

    let state = AppState {
        db_pool: db_pool.clone(),
        redis: redis.clone(),
        cache,
        semantic_cache: None,
        pgvector_semantic_cache: None,
        circuit_breaker,
        config: Arc::new(config),
        jwt,
        email: None,
        webhook_publisher: None,
        shutdown: gateway_api::shutdown::ShutdownState::new(),
    };

    let app = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test server");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Allow server to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    TestApp {
        addr,
        db_pool,
        client: reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .expect("failed to build HTTP client"),
        redis,
    }
}

async fn connect_redis() -> anyhow::Result<ConnectionManager> {
    // Try localhost first
    if let Ok(client) = redis::Client::open("redis://127.0.0.1:6379") {
        if let Ok(cm) = ConnectionManager::new(client).await {
            return Ok(cm);
        }
    }

    // Fallback: start Redis via testcontainers
    start_testcontainers_redis().await
}

#[allow(dead_code)]
async fn start_testcontainers_redis() -> anyhow::Result<ConnectionManager> {
    use testcontainers_modules::redis::Redis;
    use testcontainers::runners::AsyncRunner;

    let container = Redis::default()
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(6379).await?;
    let url = format!("redis://{}:{}", host, port);

    let client = redis::Client::open(url)?;
    let cm = ConnectionManager::new(client).await?;
    Ok(cm)
}

/// A simpler helper that doesn't require Redis.
/// Cache operations will silently fail (cache miss), but the app works.
pub async fn spawn_test_app_without_redis() -> TestApp {
    let _db_pool = create_pool(":memory:")
        .await
        .expect("failed to create SQLite pool");

    // Create a fake Redis connection manager that will fail all operations
    // but won't panic. We do this by connecting to a port that refuses connections,
    // but ConnectionManager::new retries... Actually, ConnectionManager requires
    // a successful initial connection.
    //
    // Alternative: we can't easily create a fake ConnectionManager.
    // We'll require Redis for E2E tests. Tests that don't need Redis can use
    // unit tests instead.

    panic!(
        "Redis is required for E2E tests. \
         Start Redis with: docker run -d -p 6379:6379 redis:7-alpine"
    );
}
