//! Shared application state.

use std::sync::Arc;

use gateway_db::pool::create_pool;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

/// Shared state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub redis: ConnectionManager,
    pub config: Arc<AppConfig>,
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub jwt_private_key_pem: String,
    pub jwt_public_key_pem: String,
    pub gateway_version: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://gateway:gateway_dev_password@localhost:5432/gateway_dev".into()),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".into()),
            jwt_private_key_pem: std::env::var("GATEWAY_JWT_PRIVATE_KEY").unwrap_or_default(),
            jwt_public_key_pem: std::env::var("GATEWAY_JWT_PUBLIC_KEY").unwrap_or_default(),
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl AppState {
    /// Create state from environment config.
    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::default();
        let db_pool = create_pool(&config.database_url).await?;
        let redis = Self::connect_redis(&config.redis_url).await?;
        Ok(Self {
            db_pool,
            redis,
            config: Arc::new(config),
        })
    }

    /// Connect to Redis with retry.
    async fn connect_redis(redis_url: &str) -> anyhow::Result<ConnectionManager> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(conn)
    }
}
