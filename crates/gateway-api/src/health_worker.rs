//! Health check background worker — probes providers and stores results.

use std::sync::Arc;
use std::time::Duration;

use gateway_core::circuit_breaker::CircuitBreaker;
use gateway_db::repos::provider_config_repo::ProviderConfigRepo;
use gateway_db::DbBackend;
use gateway_providers::factory::{create_provider, ProviderConfig, ProviderKind};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub provider_config_id: Uuid,
    pub healthy: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// Background worker that periodically health-checks all providers.
pub struct HealthWorker {
    db_pool: DbBackend,
    redis: ConnectionManager,
    circuit_breaker: Option<CircuitBreaker>,
    shutdown: Arc<Notify>,
    interval_secs: u64,
    check_timeout_secs: u64,
}

impl HealthWorker {
    /// Create a new health worker.
    pub fn new(
        db_pool: DbBackend,
        redis: ConnectionManager,
        interval_secs: u64,
        check_timeout_secs: u64,
    ) -> Self {
        Self {
            db_pool,
            redis,
            circuit_breaker: None,
            shutdown: Arc::new(Notify::new()),
            interval_secs,
            check_timeout_secs,
        }
    }

    /// Attach a circuit breaker to update based on health check results.
    pub fn with_circuit_breaker(mut self, cb: CircuitBreaker) -> Self {
        self.circuit_breaker = Some(cb);
        self
    }

    /// Start the worker as a background task.
    pub fn spawn(self) -> Arc<Notify> {
        let shutdown = self.shutdown.clone();
        tokio::spawn(self.run());
        shutdown
    }

    /// Run the health check loop.
    async fn run(self) {
        let mut ticker = interval(Duration::from_secs(self.interval_secs));
        let repo = ProviderConfigRepo::new(self.db_pool.clone());

        info!(
            interval_secs = self.interval_secs,
            "Health check worker started"
        );

        let cb = self.circuit_breaker.clone();
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = Self::check_all(&repo, &self.redis, cb.as_ref(), self.check_timeout_secs).await {
                        error!(error = %e, "Health check round failed");
                    }
                }
                _ = self.shutdown.notified() => {
                    info!("Health check worker shutting down");
                    // One final check before exit
                    if let Err(e) = Self::check_all(&repo, &self.redis, cb.as_ref(), self.check_timeout_secs).await {
                        error!(error = %e, "Final health check round failed");
                    }
                    break;
                }
            }
        }

        info!("Health check worker stopped");
    }

    /// Check all active providers.
    async fn check_all(
        repo: &ProviderConfigRepo,
        redis: &ConnectionManager,
        circuit_breaker: Option<&CircuitBreaker>,
        timeout_secs: u64,
    ) -> Result<(), HealthWorkerError> {
        let configs = repo.list_all_active().await?;
        debug!(provider_count = configs.len(), "Starting health check round");

        for config in configs {
            let provider_key = config.kind.clone(); // e.g. "openai", "anthropic"
            let start = std::time::Instant::now();
            let result = match Self::check_one(&config, timeout_secs).await {
                Ok(()) => {
                    let latency = start.elapsed().as_millis() as u64;
                    debug!(provider = %config.name, latency_ms = latency, "Health check passed");

                    gateway_observability::metrics::set_provider_health(
                        &provider_key,
                        &config.org_id.to_string(),
                        true,
                    );

                    // Update circuit breaker on success
                    if let Some(cb) = circuit_breaker {
                        cb.record_success(&provider_key);
                    }

                    // Update DB status to active if it was error
                    if config.status != "active" {
                        if let Err(e) = repo.update_status(config.id, "active", None).await {
                            warn!(error = %e, "Failed to update provider status");
                        }
                    }

                    HealthCheckResult {
                        provider_config_id: config.id,
                        healthy: true,
                        latency_ms: latency,
                        error: None,
                        checked_at: chrono::Utc::now(),
                    }
                }
                Err(err) => {
                    let latency = start.elapsed().as_millis() as u64;
                    warn!(provider = %config.name, error = %err, "Health check failed");

                    gateway_observability::metrics::set_provider_health(
                        &provider_key,
                        &config.org_id.to_string(),
                        false,
                    );

                    // Update circuit breaker on failure
                    if let Some(cb) = circuit_breaker {
                        cb.record_failure(&provider_key);
                    }

                    // Update DB status and last error
                    if let Err(e) = repo.update_status(config.id, "error", Some(&err)).await {
                        warn!(error = %e, "Failed to update provider status");
                    }

                    HealthCheckResult {
                        provider_config_id: config.id,
                        healthy: false,
                        latency_ms: latency,
                        error: Some(err),
                        checked_at: chrono::Utc::now(),
                    }
                }
            };

            // Store current health in Redis (TTL 2x interval)
            let key = format!("health:{}", config.id);
            let value = serde_json::to_string(&result).unwrap_or_default();
            let ttl = (timeout_secs * 4).max(120);
            let mut conn = redis.clone();
            let _: Result<(), _> = redis::cmd("SETEX")
                .arg(&key)
                .arg(ttl)
                .arg(&value)
                .query_async(&mut conn)
                .await;

            // Store history in sorted set (score = timestamp, trim to 24h)
            let history_key = format!("health_history:{}", config.id);
            let score = result.checked_at.timestamp() as f64;
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg(&history_key)
                .arg(score)
                .arg(&value)
                .query_async(&mut conn)
                .await;

            // Trim entries older than 24 hours
            let cutoff = (chrono::Utc::now() - chrono::Duration::hours(24)).timestamp() as f64;
            let _: Result<(), _> = redis::cmd("ZREMRANGEBYSCORE")
                .arg(&history_key)
                .arg("-inf")
                .arg(cutoff)
                .query_async(&mut conn)
                .await;
        }

        Ok(())
    }

    /// Check a single provider.
    async fn check_one(
        config: &gateway_db::models::ProviderConfig,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let kind = match config.kind.to_lowercase().as_str() {
            "openai" => ProviderKind::OpenAi,
            "anthropic" => ProviderKind::Anthropic,
            "gemini" => ProviderKind::Gemini,
            "ollama" => ProviderKind::Ollama,
            _ => return Err(format!("Unknown provider kind: {}", config.kind)),
        };

        let base_url = config
            .api_base
            .clone()
            .unwrap_or_else(|| default_base_url(kind.clone()));

        let api_key = match kind {
            ProviderKind::OpenAi => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            ProviderKind::Anthropic => std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            ProviderKind::Gemini => std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            ProviderKind::Ollama => String::new(),
            ProviderKind::Custom => String::new(),
        };

        let provider_config = ProviderConfig {
            kind,
            provider_id: config.id.to_string(),
            base_url,
            api_key,
            default_model: String::new(),
            timeout_ms: timeout_secs * 1000,
        };

        let provider = create_provider(provider_config)
            .map_err(|e| format!("Failed to create provider: {}", e))?;

        // Use timeout for the health check call
        match tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            provider.health_check(),
        )
        .await
        {
            Ok(gateway_providers::traits::HealthStatus::Healthy) => Ok(()),
            Ok(gateway_providers::traits::HealthStatus::Degraded(msg)) => {
                Err(format!("Health check degraded: {}", msg))
            }
            Ok(gateway_providers::traits::HealthStatus::Unhealthy(msg)) => {
                Err(format!("Health check unhealthy: {}", msg))
            }
            Err(_) => Err("Health check timed out".to_string()),
        }
    }

    /// Trigger graceful shutdown.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

fn default_base_url(kind: ProviderKind) -> String {
    match kind {
        ProviderKind::OpenAi => "https://api.openai.com".to_string(),
        ProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
        ProviderKind::Gemini => "https://generativelanguage.googleapis.com".to_string(),
        ProviderKind::Ollama => "http://localhost:11434".to_string(),
        ProviderKind::Custom => String::new(),
    }
}

/// Errors that can occur in the health worker.
#[derive(Debug, thiserror::Error)]
pub enum HealthWorkerError {
    #[error("Database error: {0}")]
    Database(#[from] gateway_db::error::DbError),
}
