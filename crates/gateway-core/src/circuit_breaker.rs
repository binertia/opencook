//! Circuit breaker pattern for provider resilience.
//!
//! Tracks per-provider health and prevents cascading failures by
//! fast-failing requests when a provider is repeatedly unhealthy.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Configuration for a circuit breaker.
#[derive(Debug, Clone, Copy)]
pub struct BreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in half-open before closing.
    pub success_threshold: u32,
    /// How long to stay open before transitioning to half-open.
    pub open_duration: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(30),
        }
    }
}

/// Internal state for a single provider's circuit breaker.
#[derive(Debug, Clone)]
struct BreakerState {
    status: Status,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_at: Option<Instant>,
}

impl Default for BreakerState {
    fn default() -> Self {
        Self {
            status: Status::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_failure_at: None,
        }
    }
}

/// Circuit breaker status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Normal operation — requests allowed.
    Closed,
    /// Failing fast — requests rejected.
    Open,
    /// Testing recovery — limited requests allowed.
    HalfOpen,
}

/// Thread-safe circuit breaker for multiple providers.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<RwLock<HashMap<String, BreakerState>>>,
    config: BreakerConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given config.
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if a request should be allowed for the given provider.
    ///
    /// Returns `Ok(())` if the request should proceed, or `Err` with
    /// the current breaker status if it should be rejected.
    pub fn check(&self, provider_key: &str) -> Result<(), BreakerError> {
        let mut map = self.inner.write().unwrap();
        let state = map.entry(provider_key.to_string()).or_default();

        match state.status {
            Status::Closed => {
                debug!(
                    provider = provider_key,
                    "Circuit breaker closed — request allowed"
                );
                Ok(())
            }
            Status::Open => {
                let opened_at = state
                    .last_failure_at
                    .expect("Open state must have last_failure_at");
                if opened_at.elapsed() >= self.config.open_duration {
                    info!(
                        provider = provider_key,
                        "Circuit breaker transitioning Open → HalfOpen"
                    );
                    state.status = Status::HalfOpen;
                    state.consecutive_successes = 0;
                    Ok(())
                } else {
                    warn!(
                        provider = provider_key,
                        remaining_ms =
                            (self.config.open_duration - opened_at.elapsed()).as_millis() as u64,
                        "Circuit breaker open — request rejected"
                    );
                    Err(BreakerError::Open {
                        retry_after: self.config.open_duration - opened_at.elapsed(),
                    })
                }
            }
            Status::HalfOpen => {
                debug!(
                    provider = provider_key,
                    "Circuit breaker half-open — request allowed (trial)"
                );
                Ok(())
            }
        }
    }

    /// Record a successful call for the given provider.
    pub fn record_success(&self, provider_key: &str) {
        let mut map = self.inner.write().unwrap();
        let state = map.entry(provider_key.to_string()).or_default();

        state.consecutive_failures = 0;

        match state.status {
            Status::Closed => {
                // Nothing to do
            }
            Status::HalfOpen => {
                state.consecutive_successes += 1;
                if state.consecutive_successes >= self.config.success_threshold {
                    info!(
                        provider = provider_key,
                        successes = state.consecutive_successes,
                        "Circuit breaker transitioning HalfOpen → Closed"
                    );
                    state.status = Status::Closed;
                    state.consecutive_successes = 0;
                }
            }
            Status::Open => {
                // Should not happen — Open rejects before the call.
                // If it does, transition to HalfOpen.
                state.status = Status::HalfOpen;
                state.consecutive_successes = 1;
            }
        }
    }

    /// Record a failed call for the given provider.
    pub fn record_failure(&self, provider_key: &str) {
        let mut map = self.inner.write().unwrap();
        let state = map.entry(provider_key.to_string()).or_default();

        state.consecutive_failures += 1;
        state.last_failure_at = Some(Instant::now());

        match state.status {
            Status::Closed => {
                if state.consecutive_failures >= self.config.failure_threshold {
                    warn!(
                        provider = provider_key,
                        failures = state.consecutive_failures,
                        "Circuit breaker transitioning Closed → Open"
                    );
                    state.status = Status::Open;
                }
            }
            Status::HalfOpen => {
                warn!(
                    provider = provider_key,
                    "Circuit breaker transitioning HalfOpen → Open"
                );
                state.status = Status::Open;
            }
            Status::Open => {
                // Already open — just refresh the timer
            }
        }
    }

    /// Get the current status for a provider (without side effects).
    pub fn status(&self, provider_key: &str) -> Status {
        let map = self.inner.read().unwrap();
        map.get(provider_key)
            .map(|s| {
                // Check if Open should transition to HalfOpen
                if s.status == Status::Open {
                    if let Some(opened_at) = s.last_failure_at {
                        if opened_at.elapsed() >= self.config.open_duration {
                            return Status::HalfOpen;
                        }
                    }
                }
                s.status
            })
            .unwrap_or(Status::Closed)
    }

    /// Reset a provider's breaker to Closed.
    pub fn reset(&self, provider_key: &str) {
        let mut map = self.inner.write().unwrap();
        if let Some(state) = map.get_mut(provider_key) {
            info!(
                provider = provider_key,
                "Circuit breaker manually reset to Closed"
            );
            state.status = Status::Closed;
            state.consecutive_failures = 0;
            state.consecutive_successes = 0;
            state.last_failure_at = None;
        }
    }
}

/// Error returned when the circuit breaker is open.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BreakerError {
    #[error("Circuit breaker is OPEN — retry after {retry_after:?}")]
    Open { retry_after: Duration },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            open_duration: Duration::from_secs(60),
        });

        let key = "test-provider";

        // Start closed
        assert!(cb.check(key).is_ok());
        assert_eq!(cb.status(key), Status::Closed);

        // 2 failures — still closed
        cb.record_failure(key);
        cb.record_failure(key);
        assert!(cb.check(key).is_ok());
        assert_eq!(cb.status(key), Status::Closed);

        // 3rd failure — opens
        cb.record_failure(key);
        assert!(cb.check(key).is_err());
        assert_eq!(cb.status(key), Status::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open_recovery() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_duration: Duration::from_millis(10),
        });

        let key = "test-provider";

        // Open the circuit
        cb.record_failure(key);
        assert_eq!(cb.status(key), Status::Open);
        assert!(cb.check(key).is_err());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to HalfOpen on next check
        assert!(cb.check(key).is_ok());
        assert_eq!(cb.status(key), Status::HalfOpen);

        // First success
        cb.record_success(key);
        assert_eq!(cb.status(key), Status::HalfOpen);
        assert!(cb.check(key).is_ok());

        // Second success — closes
        cb.record_success(key);
        assert_eq!(cb.status(key), Status::Closed);
        assert!(cb.check(key).is_ok());
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let cb = CircuitBreaker::new(BreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_duration: Duration::from_millis(10),
        });

        let key = "test-provider";

        cb.record_failure(key);
        std::thread::sleep(Duration::from_millis(20));

        // HalfOpen
        assert!(cb.check(key).is_ok());
        assert_eq!(cb.status(key), Status::HalfOpen);

        // Failure in HalfOpen reopens immediately
        cb.record_failure(key);
        assert_eq!(cb.status(key), Status::Open);
        assert!(cb.check(key).is_err());
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new(BreakerConfig::default());
        let key = "test-provider";

        cb.record_failure(key);
        cb.record_failure(key);
        cb.record_failure(key);
        cb.record_failure(key);
        cb.record_failure(key);
        assert_eq!(cb.status(key), Status::Open);

        cb.reset(key);
        assert_eq!(cb.status(key), Status::Closed);
        assert!(cb.check(key).is_ok());
    }
}
