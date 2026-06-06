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
    pub pgvector_semantic_cache: Option<gateway_cache::PgvectorSemanticCache>,
    pub circuit_breaker: CircuitBreaker,
    pub config: Arc<AppConfig>,
    pub jwt: Arc<JwtService>,
    pub email: Option<gateway_auth::EmailService>,
    pub webhook_publisher: Option<gateway_core::webhook_publisher::WebhookPublisher>,
    pub shutdown: crate::shutdown::ShutdownState,
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
    #[serde(default)]
    tls_cert: Option<String>,
    #[serde(default)]
    tls_key: Option<String>,
    #[serde(default)]
    allowed_origins: Option<String>,
    #[serde(default = "default_environment")]
    environment: String,
    #[serde(default)]
    smtp_host: Option<String>,
    #[serde(default)]
    smtp_port: Option<u16>,
    #[serde(default)]
    smtp_user: Option<String>,
    #[serde(default)]
    smtp_password: Option<String>,
    #[serde(default)]
    smtp_from: Option<String>,
    #[serde(default)]
    trust_x_forwarded_proto: bool,
    #[serde(default)]
    trusted_proxy_count: usize,
}

fn default_environment() -> String {
    "development".to_string()
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
    pub master_key_previous: Option<[u8; 32]>,
    pub semantic_cache_enabled: bool,
    pub semantic_cache_threshold: f32,
    pub embedding_base_url: String,
    pub embedding_api_key: String,
    pub embedding_model: String,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub allowed_origins: Vec<String>,
    pub environment: String,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub trust_x_forwarded_proto: bool,
    pub trusted_proxy_count: usize,
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
                tls_cert: None,
                tls_key: None,
                allowed_origins: None,
                environment: default_environment(),
                smtp_host: None,
                smtp_port: None,
                smtp_user: None,
                smtp_password: None,
                smtp_from: None,
                trust_x_forwarded_proto: false,
                trusted_proxy_count: 0,
            }
        });

        // Master key: fail-closed in production, dev fallback with warning
        // Supports comma-separated rotation: "new_key,old_key"
        let master_key_hex = raw
            .master_key
            .or_else(|| std::env::var("GATEWAY_MASTER_KEY").ok());

        let (master_key, master_key_previous) = match master_key_hex {
            Some(hex) => {
                let pair = gateway_auth::key_rotation::parse_master_key_pair(&hex)
                    .expect("GATEWAY_MASTER_KEY must be a valid 64-character hex string (32 bytes). Supports comma-separated rotation: new_key,old_key");
                (pair.primary, pair.secondary)
            }
            None => {
                if raw.environment == "production" {
                    panic!(
                        "GATEWAY_MASTER_KEY is required in production. \
                         Set it to a 64-character hex string (32 bytes)."
                    );
                }
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
                (dev_key, None)
            }
        };

        let jwt_private_key_pem = raw
            .jwt_private_key_pem
            .or_else(|| std::env::var("GATEWAY_JWT_PRIVATE_KEY").ok())
            .unwrap_or_default();

        let jwt_public_key_pem = raw
            .jwt_public_key_pem
            .or_else(|| std::env::var("GATEWAY_JWT_PUBLIC_KEY").ok())
            .unwrap_or_default();

        let embedding_base_url = raw
            .embedding_base_url
            .or_else(|| std::env::var("EMBEDDING_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com".to_string());

        let embedding_api_key = raw
            .embedding_api_key
            .or_else(|| std::env::var("EMBEDDING_API_KEY").ok())
            .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

        Self {
            port: if raw.port != 0 {
                raw.port
            } else {
                std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080)
            },
            database_url: raw
                .database_url
                .or_else(|| std::env::var("DATABASE_URL").ok())
                .expect("DATABASE_URL must be set"),
            redis_url: raw
                .redis_url
                .or_else(|| std::env::var("REDIS_URL").ok())
                .unwrap_or_else(|| "redis://localhost:6379".into()),
            jwt_private_key_pem,
            jwt_public_key_pem,
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
            profile: raw.profile.unwrap_or_default(),
            master_key,
            master_key_previous,
            semantic_cache_enabled: raw.semantic_cache_enabled,
            semantic_cache_threshold: raw.semantic_cache_threshold,
            embedding_base_url,
            embedding_api_key,
            embedding_model: raw.embedding_model,
            tls_cert: raw.tls_cert,
            tls_key: raw.tls_key,
            allowed_origins: raw
                .allowed_origins
                .map(|s| s.split(',').map(|o| o.trim().to_string()).collect())
                .unwrap_or_default(),
            environment: raw.environment,
            smtp_host: raw.smtp_host,
            smtp_port: raw.smtp_port.unwrap_or(587),
            smtp_user: raw.smtp_user,
            smtp_password: raw.smtp_password,
            smtp_from: raw.smtp_from,
            trust_x_forwarded_proto: raw.trust_x_forwarded_proto,
            trusted_proxy_count: raw.trusted_proxy_count,
        }
    }
}

impl AppConfig {
    /// Decrypt ciphertext trying primary master key first, then previous.
    pub fn decrypt_master(
        &self,
        ciphertext: &[u8],
    ) -> Result<String, gateway_auth::crypto::CryptoError> {
        let pair = gateway_auth::ActiveKeyPair {
            primary: self.master_key,
            secondary: self.master_key_previous,
        };
        gateway_auth::crypto::decrypt_with_keys(ciphertext, &pair)
    }

    /// Determine whether cookies should be marked Secure.
    /// Returns true if TLS is configured locally, or if the deployment is
    /// behind a TLS terminator and `trust_x_forwarded_proto` is enabled.
    pub fn secure_cookie(&self, headers: Option<&axum::http::HeaderMap>) -> bool {
        if self.tls_cert.is_some() {
            return true;
        }
        if let Some(headers) = headers {
            if self.trust_x_forwarded_proto {
                return headers
                    .get("x-forwarded-proto")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.eq_ignore_ascii_case("https"))
                    .unwrap_or(false);
            }
        }
        false
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
        let embedding_client = gateway_cache::EmbeddingClient::new(
            config.embedding_base_url.clone(),
            config.embedding_api_key.clone(),
            config.embedding_model.clone(),
        );

        let semantic_cache = if config.semantic_cache_enabled {
            tracing::info!(
                model = %config.embedding_model,
                threshold = config.semantic_cache_threshold,
                "Semantic cache enabled (Redis)"
            );
            Some(gateway_cache::SemanticCache::new(
                redis.clone(),
                embedding_client.clone(),
                config.semantic_cache_threshold,
            ))
        } else {
            None
        };

        let pgvector_semantic_cache = if config.semantic_cache_enabled {
            match &db_pool {
                DbBackend::Postgres(pg) => {
                    tracing::info!(
                        model = %config.embedding_model,
                        threshold = config.semantic_cache_threshold,
                        "Semantic cache enabled (pgvector)"
                    );
                    let cache = gateway_cache::PgvectorSemanticCache::new(
                        pg.clone(),
                        redis.clone(),
                        embedding_client,
                        config.semantic_cache_threshold,
                    );
                    // Spawn background maintenance task (detached)
                    let _handle = gateway_cache::semantic_pg::spawn_maintenance(
                        cache.clone(),
                        std::time::Duration::from_secs(300),
                        100_000,
                    );
                    Some(cache)
                }
                DbBackend::Sqlite(_) => {
                    tracing::debug!("pgvector semantic cache unavailable in SQLite mode");
                    None
                }
            }
        } else {
            None
        };

        // JWT: RS256 if PEM keys provided, otherwise HS256 with random dev secret
        let jwt = if !config.jwt_private_key_pem.is_empty() && !config.jwt_public_key_pem.is_empty()
        {
            JwtService::from_pem(
                config.jwt_private_key_pem.as_bytes(),
                config.jwt_public_key_pem.as_bytes(),
            )
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

        // Email service (optional)
        let email = config.smtp_host.as_ref().map(|host| {
            let from = config
                .smtp_from
                .clone()
                .unwrap_or_else(|| "noreply@localhost".to_string());
            let email_config = gateway_auth::EmailConfig {
                host: host.clone(),
                port: config.smtp_port,
                user: config.smtp_user.clone(),
                password: config.smtp_password.clone(),
                from,
            };
            gateway_auth::EmailService::new(email_config)
        });

        // Webhook publisher (always available, but optional if no webhooks configured)
        let webhook_publisher = Some(gateway_core::webhook_publisher::WebhookPublisher::new(
            db_pool.clone(),
            config.master_key,
        ));

        Ok(Self {
            db_pool,
            redis,
            cache,
            semantic_cache,
            pgvector_semantic_cache,
            circuit_breaker,
            config: Arc::new(config),
            jwt: Arc::new(jwt),
            email,
            webhook_publisher,
            shutdown: crate::shutdown::ShutdownState::new(),
        })
    }

    /// Connect to Redis with retry.
    async fn connect_redis(redis_url: &str) -> anyhow::Result<ConnectionManager> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(conn)
    }
}
