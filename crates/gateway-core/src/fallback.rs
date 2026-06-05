//! Fallback chain execution — try providers sequentially until one succeeds.
//!
//! When a provider fails (circuit open, network error, rate limit, etc.)
//! the gateway transparently tries the next provider in the fallback chain.
//! Each attempt is logged with provider, error, and latency.

use crate::circuit_breaker::CircuitBreaker;
use crate::retry::{retry, RetryConfig};
use std::future::Future;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Result of a single fallback attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackAttempt {
    /// Provider that was tried.
    pub provider: String,
    /// Attempt index (0 = primary).
    pub attempt: usize,
    /// Whether this was the primary provider.
    pub is_primary: bool,
    /// Error message if the attempt failed.
    pub error: Option<String>,
    /// Latency of the attempt in milliseconds.
    pub latency_ms: u64,
}

/// Error returned when every provider in the fallback chain has failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("All providers failed. Last error: {last_error}")]
pub struct FallbackExhausted {
    /// All attempts that were made.
    pub attempts: Vec<FallbackAttempt>,
    /// The last error encountered.
    pub last_error: String,
}

/// Configuration for fallback chain execution.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Retry configuration applied to each individual provider attempt.
    pub retry: RetryConfig,
    /// Whether to log fallback attempts at INFO level.
    pub log_attempts: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            retry: RetryConfig::default(),
            log_attempts: true,
        }
    }
}

/// Execute a fallback chain: try providers sequentially until one succeeds.
///
/// # Arguments
/// * `providers` — list of provider callables (primary first, then fallbacks)
/// * `config` — fallback configuration (retries, logging)
/// * `cb` — circuit breaker to check/record each attempt
/// * `cancellation` — token to abort the chain on client disconnect
/// * `call` — async function that calls a single provider
///
/// # Cancellation
/// If the cancellation token fires, the current attempt is aborted and no
/// further providers are tried. The caller should return a 499 or simply
/// close the connection.
///
/// # Returns
/// * `Ok(T)` — one provider succeeded
/// * `Err(FallbackExhausted)` — all providers failed
pub async fn execute_fallback_chain<P, F, Fut, T, E>(
    providers: Vec<P>,
    config: &FallbackConfig,
    cb: &CircuitBreaker,
    cancellation: &CancellationToken,
    mut call: F,
) -> Result<T, FallbackExhausted>
where
    P: AsRef<str>,
    F: FnMut(&P) -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempts = Vec::new();
    let mut last_error = String::new();

    for (idx, provider) in providers.into_iter().enumerate() {
        let provider_str = provider.as_ref().to_string();
        let is_primary = idx == 0;

        // Check cancellation before starting this attempt
        if cancellation.is_cancelled() {
            warn!(
                provider = %provider_str,
                attempt = idx,
                "Cancellation detected — skipping remaining fallback providers"
            );
            break;
        }

        // Circuit breaker check
        if let Err(e) = cb.check(&provider_str) {
            let msg = format!("Circuit breaker open: {}", e);
            warn!(provider = %provider_str, error = %msg, "Skipping provider (circuit open)");
            attempts.push(FallbackAttempt {
                provider: provider_str.clone(),
                attempt: idx,
                is_primary,
                error: Some(msg.clone()),
                latency_ms: 0,
            });
            last_error = msg;
            continue;
        }

        let start = Instant::now();

        // Try the provider with retries and cancellation
        let attempt_result = crate::cancellation::with_cancellation(
            cancellation,
            retry(config.retry.clone(), || call(&provider)),
        )
        .await;
        let latency_ms = start.elapsed().as_millis() as u64;

        match attempt_result {
            Ok(Ok(result)) => {
                // Success!
                cb.record_success(&provider_str);
                if !is_primary {
                    info!(
                        provider = %provider_str,
                        latency_ms,
                        "Request served by fallback provider"
                    );
                }
                if config.log_attempts {
                    attempts.push(FallbackAttempt {
                        provider: provider_str,
                        attempt: idx,
                        is_primary,
                        error: None,
                        latency_ms,
                    });
                }
                return Ok(result);
            }
            Ok(Err(e)) => {
                // Provider failed after retries
                let msg = e.to_string();
                warn!(
                    provider = %provider_str,
                    attempt = idx,
                    error = %msg,
                    latency_ms,
                    "Provider call failed, trying fallback"
                );
                cb.record_failure(&provider_str);
                last_error = msg.clone();
                attempts.push(FallbackAttempt {
                    provider: provider_str,
                    attempt: idx,
                    is_primary,
                    error: Some(msg),
                    latency_ms,
                });
            }
            Err(_) => {
                // Cancelled
                let msg = "Request cancelled by client disconnect".to_string();
                warn!(
                    provider = %provider_str,
                    attempt = idx,
                    "Request cancelled — aborting fallback chain"
                );
                attempts.push(FallbackAttempt {
                    provider: provider_str,
                    attempt: idx,
                    is_primary,
                    error: Some(msg.clone()),
                    latency_ms,
                });
                last_error = msg;
                break;
            }
        }
    }

    Err(FallbackExhausted {
        attempts,
        last_error,
    })
}

/// Build a provider key for circuit breaker from provider ID + model.
pub fn provider_circuit_key(provider_id: &str, model: &str) -> String {
    format!("{}:{}", provider_id, model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit_breaker::{BreakerConfig, CircuitBreaker};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_fallback_chain_first_succeeds() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(60),
        });
        let token = CancellationToken::new();
        let providers = vec!["provider-a".to_string(), "provider-b".to_string()];

        let result = execute_fallback_chain(
            providers,
            &FallbackConfig::default(),
            &cb,
            &token,
            |p| {
                let name = p.as_str().to_string();
                async move {
                    if name == "provider-a" {
                        Ok::<_, String>(42)
                    } else {
                        Err("should not reach".to_string())
                    }
                }
            },
        )
        .await;

        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_fallback_chain_tries_next_on_failure() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(60),
        });
        let token = CancellationToken::new();
        let providers = vec!["provider-a".to_string(), "provider-b".to_string()];

        let result = execute_fallback_chain(
            providers,
            &FallbackConfig::default(),
            &cb,
            &token,
            |p| {
                let name = p.as_str().to_string();
                async move {
                    if name == "provider-a" {
                        Err::<i32, String>("primary down".to_string())
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;

        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_fallback_chain_all_fail() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(60),
        });
        let token = CancellationToken::new();
        let providers = vec!["provider-a".to_string(), "provider-b".to_string()];

        let result = execute_fallback_chain(
            providers,
            &FallbackConfig::default(),
            &cb,
            &token,
            |_p| async move { Err::<i32, String>("all down".to_string()) },
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.last_error, "all down");
        assert_eq!(err.attempts.len(), 2);
    }

    #[tokio::test]
    async fn test_fallback_chain_respects_cancellation() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(60),
        });
        let token = CancellationToken::new();
        let providers = vec!["provider-a".to_string(), "provider-b".to_string()];

        token.cancel();

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let result = execute_fallback_chain(
            providers,
            &FallbackConfig::default(),
            &cb,
            &token,
            move |_p| {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok::<i32, String>(42)
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_provider_circuit_key() {
        assert_eq!(
            provider_circuit_key("openai", "gpt-4"),
            "openai:gpt-4"
        );
    }
}
