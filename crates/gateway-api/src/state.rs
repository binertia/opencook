//! Shared application state.

use std::sync::Arc;

use gateway_auth::JwtService;
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
    pub semantic_cache: Option<gateway_cache::SemanticCache>,
    pub circuit_breaker: CircuitBreaker,
    pub config: Arc<AppConfig>,
    pub jwt: Arc<JwtService>,
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
    #[serde(default)]
    semantic_cache_enabled: bool,
    #[serde(default = "default_semantic_threshold")]
    semantic_cache_threshold: f32,
    #[serde(default)]
    embedding_base_url: Option<String>,
    #[serde(default)]
    embedding_api_key: Option<String>,
    #[serde(default = "default_embedding_model")]
    embedding_model: String,
}

fn default_semantic_threshold() -> f32 {
    0.95
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
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
    pub semantic_cache_enabled: bool,
    pub semantic_cache_threshold: f32,
    pub embedding_base_url: String,
    pub embedding_api_key: String,
    pub embedding_model: String,
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
                semantic_cache_enabled: false,
                semantic_cache_threshold: default_semantic_threshold(),
                embedding_base_url: None,
                embedding_api_key: None,
                embedding_model: default_embedding_model(),
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

        let jwt_private_key_pem = raw.jwt_private_key_pem.or_else(|| {
            std::env::var("GATEWAY_JWT_PRIVATE_KEY").ok()
        }).unwrap_or_default();

        let jwt_public_key_pem = raw.jwt_public_key_pem.or_else(|| {
            std::env::var("GATEWAY_JWT_PUBLIC_KEY").ok()
        }).unwrap_or_default();

        let embedding_base_url = raw.embedding_base_url.or_else(|| {
            std::env::var("EMBEDDING_BASE_URL").ok()
        }).unwrap_or_else(|| "https://api.openai.com".to_string());

        let embedding_api_key = raw.embedding_api_key.or_else(|| {
            std::env::var("EMBEDDING_API_KEY").ok()
        }).unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

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
            jwt_private_key_pem,
            jwt_public_key_pem,
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
            profile: raw.profile.unwrap_or_default(),
            master_key,
            semantic_cache_enabled: raw.semantic_cache_enabled,
            semantic_cache_threshold: raw.semantic_cache_threshold,
            embedding_base_url,
            embedding_api_key,
            embedding_model: raw.embedding_model,
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

        // Semantic cache (optional)
        let semantic_cache = if config.semantic_cache_enabled {
            let embedding_client = gateway_cache::EmbeddingClient::new(
                config.embedding_base_url.clone(),
                config.embedding_api_key.clone(),
                config.embedding_model.clone(),
            );
            tracing::info!(
                model = %config.embedding_model,
                threshold = config.semantic_cache_threshold,
                "Semantic cache enabled"
            );
            Some(gateway_cache::SemanticCache::new(
                redis.clone(),
                embedding_client,
                config.semantic_cache_threshold,
            ))
        } else {
            None
        };

        // JWT: RS256 if PEM keys provided, otherwise HS256 with random dev secret
        let jwt = if !config.jwt_private_key_pem.is_empty() && !config.jwt_public_key_pem.is_empty() {
            JwtService::from_pem(config.jwt_private_key_pem.as_bytes(), config.jwt_public_key_pem.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to load JWT keys: {e}"))?
        } else {
            let secret = {
                use rand::Rng;
                let mut s = [0u8; 32];
                rand::thread_rng().fill(&mut s);
                s
            };
            tracing::warn!(
                "JWT keys not configured. Using HS256 with a random dev secret. \
                 Sessions will NOT survive restarts. \
                 Set GATEWAY_JWT_PRIVATE_KEY and GATEWAY_JWT_PUBLIC_KEY for production."
            );
            JwtService::from_secret(&secret)
        };

        Ok(Self {
            db_pool,
            redis,
            cache,
            semantic_cache,
            circuit_breaker,
            config: Arc::new(config),
            jwt: Arc::new(jwt),
        })
    }

    /// Connect to Redis with retry.
    async fn connect_redis(redis_url: &str) -> anyhow::Result<ConnectionManager> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(conn)
    }
}
