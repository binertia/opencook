//! Shared application state for SOLO mode.

use std::sync::Arc;

use gateway_core::circuit_breaker::{BreakerConfig, CircuitBreaker};
use gateway_core::profiles::RoutingProfile;
use gateway_db::pool::create_pool;
use gateway_db::DbBackend;

/// Shared state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db_pool: DbBackend,
    pub circuit_breaker: CircuitBreaker,
    pub config: Arc<AppConfig>,
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub profile: RoutingProfile,
    pub gateway_version: String,
}

impl AppConfig {
    /// Load config from `gateway-solo.toml` (if exists) layered over environment variables.
    pub fn load() -> Self {
        use figment::providers::{Env, Format, Toml};
        use figment::Figment;

        let mut figment = Figment::new();

        // Layer 1: gateway-solo.toml if it exists
        if std::path::Path::new("gateway-solo.toml").exists() {
            figment = figment.merge(Toml::file("gateway-solo.toml"));
        }
        // Also check gateway.toml for compatibility
        else if std::path::Path::new("gateway.toml").exists() {
            figment = figment.merge(Toml::file("gateway.toml"));
        }

        // Layer 2: environment variables with GATEWAY_ prefix
        figment = figment.merge(Env::prefixed("GATEWAY_"));

        #[derive(Debug, serde::Deserialize)]
        struct RawConfig {
            #[serde(default)]
            port: u16,
            #[serde(default)]
            database_url: Option<String>,
            #[serde(default)]
            profile: Option<RoutingProfile>,
        }

        let raw: RawConfig = figment.extract().unwrap_or_else(|e| {
            tracing::warn!("Failed to load config file: {}. Using env defaults.", e);
            RawConfig {
                port: 0,
                database_url: None,
                profile: None,
            }
        });

        // SOLO mode defaults: SQLite local file
        let database_url = raw.database_url.or_else(|| {
            std::env::var("DATABASE_URL").ok()
        }).unwrap_or_else(|| "sqlite://./data/gateway.db".into());

        Self {
            port: if raw.port != 0 {
                raw.port
            } else {
                std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080)
            },
            database_url,
            profile: raw.profile.unwrap_or_default(),
            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl AppState {
    /// Create state from environment config.
    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::load();
        tracing::info!(profile = %config.profile, "Loaded SOLO gateway configuration");

        let db_pool = create_pool(&config.database_url).await?;
        let circuit_breaker = CircuitBreaker::new(BreakerConfig::default());

        Ok(Self {
            db_pool,
            circuit_breaker,
            config: Arc::new(config),
        })
    }
}
