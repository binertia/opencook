//! Shared application state.

use std::sync::Arc;

use gateway_cache::TwoTierCache;
use gateway_core::circuit_breaker::{BreakerConfig, CircuitBreaker};
use gateway_core::profiles::RoutingProfile;
use gateway_db::pool::create_pool;
use gateway_db::DbBackend;
use redis::aio::ConnectionManager;

/// Shared state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db_pool: DbBackend,
    pub redis: ConnectionManager,
    pub cache: TwoTierCache,
    pub circuit_breaker: CircuitBreaker,
    pub config: Arc<AppConfig>,
}

/// Raw configuration loaded from file + env.
#[derive(Debug, serde::Deserialize)]
struct RawConfig {
    #[serde(default)]
    port: u16,
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    redis_url: Option<String>,
    #[serde(default)]
    jwt_private_key_pem: Option<String>,
    #[serde(default)]
    jwt_public_key_pem: Option<String>,
    #[serde(default)]
    profile: Option<RoutingProfile>,
    #[serde(default)]
    master_key: Option<String>,
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
    pub profile: RoutingProfile,
    pub master_key: [u8; 32],
}

impl AppConfig {
    /// Load config from `gateway.toml` (if exists) layered over environment variables.
    pub fn load() -> Self {
        use figment::providers::{Env, Format, Toml};
        use figment::Figment;

        let mut figment = Figment::new();

        // Layer 1: gateway.toml if it exists
        if std::path::Path::new("gateway.toml").exists() {
            figment = figment.merge(Toml::file("gateway.toml"));
        }

        // Layer 2: environment variables with GATEWAY_ prefix
        figment = figment.merge(Env::prefixed("GATEWAY_"));

        let raw: RawConfig = figment.extract().unwrap_or_else(|e| {
            tracing::warn!("Failed to load config file: {}. Using env defaults.", e);
            RawConfig {
                port: 0,
                database_url: None,
                redis_url: None,
                jwt_private_key_pem: None,
                jwt_public_key_pem: None,
                profile: None,
                master_key: None,
            }
        });

        // Master key: fail-closed in production, dev fallback with warning
        let master_key_hex = raw.master_key.or_else(|| {
            std::env::var("GATEWAY_MASTER_KEY").ok()
        });

        let master_key = match master_key_hex {
            Some(hex) => gateway_auth::crypto::parse_master_key(&hex)
                .expect("GATEWAY_MASTER_KEY must be a valid 64-character hex string (32 bytes)"),
            None => {
                // Dev fallback: generate a random key and warn loudly
                let dev_key = {
                    use rand::Rng;
                    let mut k = [0u8; 32];
                    rand::thread_rng().fill(&mut k);
                    k
                };
                tracing::warn!(
                    "GATEWAY_MASTER_KEY not set. Using a random dev key. \
                     Provider config encryption will NOT survive restarts. \
                     Set GATEWAY_MASTER_KEY to a 64-char hex string for production."
                );
                dev_key
            }
        };

        Self {
            port: if raw.port != 0 {
                raw.port
            } else {
                std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080)
            },
            database_url: raw.database_url.or_else(|| {
                std::env::var("DATABASE_URL").ok()
            }).unwrap_or_else(|| "postgres://gateway:gateway_dev_password@localhost:5432/gateway_dev".into()),
            redis_url: raw.redis_url.or_else(|| {
                std::env::var("REDIS_URL").ok()
            }).unwrap_or_else(|| "redis://localhost:6379".into()),
            jwt_private_key_pem: raw.jwt_private_key_pem.or_else(|| {
                std::env::var("GATEWAY_JWT_PRIVATE_KEY").ok()
            }).unwrap_or_default(),
            jwt_public_key_pem: raw.jwt_public_key_pem.or_else(|| {
                std::env::var("GATEWAY_JWT_PUBLIC_KEY").ok()
            }).unwrap_or_default(),
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
            profile: raw.profile.unwrap_or_default(),
            master_key,
        }
    }
}

impl AppState {
    /// Create state from environment config.
    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::load();
        tracing::info!(profile = %config.profile, "Loaded gateway configuration");
        let db_pool = create_pool(&config.database_url).await?;
        let redis = Self::connect_redis(&config.redis_url).await?;
        let cache = TwoTierCache::new(redis.clone());
        let circuit_breaker = CircuitBreaker::new(BreakerConfig::default());
        Ok(Self {
            db_pool,
            redis,
            cache,
            circuit_breaker,
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
