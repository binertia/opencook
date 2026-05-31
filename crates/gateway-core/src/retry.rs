//! Retry policy with exponential backoff and jitter.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Configuration for retry behavior.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay between retries (before exponential growth).
    pub base_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Whether to add jitter to delays.
    pub jitter: bool,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
            jitter: true,
            backoff_multiplier: 2.0,
        }
    }
}

/// Execute an async operation with retry logic.
///
/// The operation is called initially, and if it returns `Err`, it is
/// retried up to `config.max_retries` times with exponential backoff.
///
/// Returns the result of the first successful attempt, or the error
/// from the final attempt.
pub async fn retry<F, Fut, T, E>(config: RetryConfig, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempt >= config.max_retries {
                    warn!(
                        attempts = attempt + 1,
                        "Operation failed after max retries"
                    );
                    return Err(err);
                }

                let delay = compute_delay(config, attempt);
                warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "Operation failed, retrying after delay"
                );
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Compute the delay for a given retry attempt.
fn compute_delay(config: RetryConfig, attempt: u32) -> Duration {
    let exponential = config.base_delay.as_secs_f64()
        * config.backoff_multiplier.powi(attempt as i32);
    let capped = exponential.min(config.max_delay.as_secs_f64());

    let delay_ms = if config.jitter {
        // Add random jitter between 0% and 25% of the delay
        let jitter_factor = 1.0 + rand::random::<f64>() * 0.25;
        (capped * jitter_factor * 1000.0) as u64
    } else {
        (capped * 1000.0) as u64
    };

    Duration::from_millis(delay_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_success_on_first() {
        let config = RetryConfig {
            max_retries: 2,
            ..Default::default()
        };

        let result = retry(config, || async { Ok::<_, String>(42) }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_retry_eventual_success() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        };

        let mut calls = 0;
        let result = retry(config, || {
            calls += 1;
            async move {
                if calls < 3 {
                    Err("not yet")
                } else {
                    Ok(calls)
                }
            }
        })
        .await;

        assert_eq!(result, Ok(3));
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        };

        let mut calls = 0;
        let result: Result<i32, &str> = retry(config, || {
            calls += 1;
            async move { Err("always fails") }
        })
        .await;

        assert_eq!(result, Err("always fails"));
        assert_eq!(calls, 3); // initial + 2 retries
    }

    #[test]
    fn test_delay_computation() {
        let config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        assert_eq!(compute_delay(config, 0), Duration::from_secs(1));
        assert_eq!(compute_delay(config, 1), Duration::from_secs(2));
        assert_eq!(compute_delay(config, 2), Duration::from_secs(4));
        assert_eq!(compute_delay(config, 3), Duration::from_secs(8));
        assert_eq!(compute_delay(config, 4), Duration::from_secs(10)); // capped
    }
}
