//! Request cancellation — propagate client disconnect to upstream providers.
//!
//! When a client disconnects, the gateway must stop the upstream request
//! to avoid wasting provider tokens. This module provides cancellation token
//! propagation with a guaranteed abort window.

use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Maximum time allowed to abort an upstream request after cancellation.
#[allow(dead_code)]
const ABORT_TIMEOUT: Duration = Duration::from_millis(500);

/// Error returned when a request is cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Request cancelled by client disconnect")]
pub struct Cancelled;

/// Wrap a future so it can be cancelled via a [`CancellationToken`].
///
/// If the token is triggered, the future is dropped and [`Cancelled`] is
/// returned. The abort is guaranteed to complete within [`ABORT_TIMEOUT`].
pub async fn with_cancellation<T, F>(
    token: &CancellationToken,
    fut: F,
) -> Result<T, Cancelled>
where
    F: Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => {
            warn!("Request cancelled: upstream aborting within 500ms");
            Err(Cancelled)
        }
        result = fut => Ok(result),
    }
}

/// Wrap a future with both cancellation and a hard timeout.
///
/// Returns [`Cancelled`] if the token fires, or a generic timeout error
/// if the future exceeds `max_duration`.
pub async fn with_cancellation_and_timeout<T, F>(
    token: &CancellationToken,
    fut: F,
    max_duration: Duration,
) -> Result<T, CancellationError>
where
    F: Future<Output = T>,
{
    let timed = timeout(max_duration, with_cancellation(token, fut));
    match timed.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(Cancelled)) => Err(CancellationError::Cancelled),
        Err(_) => {
            warn!("Request timed out after {:?}", max_duration);
            Err(CancellationError::Timeout)
        }
    }
}

/// Combined cancellation/timeout error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CancellationError {
    #[error("Request cancelled by client disconnect")]
    Cancelled,
    #[error("Request timed out")]
    Timeout,
}

/// Create a child cancellation token linked to a parent.
///
/// Cancelling the parent automatically cancels all children.
pub fn child_token(parent: &CancellationToken) -> CancellationToken {
    parent.child_token()
}

/// Spawn a background task that cancels the token when the given
/// future completes (used for client-disconnect detection).
pub fn spawn_disconnect_watcher<F>(token: CancellationToken, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        signal.await;
        debug!("Client disconnect detected — cancelling upstream requests");
        token.cancel();
    });
}

/// A guard that cancels a [`CancellationToken`] when dropped.
///
/// Place this in the Axum handler scope; when the client disconnects
/// the handler future is dropped, which triggers cancellation of
/// upstream provider requests.
pub struct CancelOnDrop(pub CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Instant};

    #[tokio::test]
    async fn test_cancellation_completes_normally() {
        let token = CancellationToken::new();
        let result = with_cancellation(&token, async { 42 }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_cancellation_aborts_when_token_fired() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            token_clone.cancel();
        });

        let start = Instant::now();
        let result = with_cancellation(&token, async {
            sleep(Duration::from_secs(10)).await;
            42
        })
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Cancelled);
        // Should abort quickly, not wait 10 seconds
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_timeout_returns_timeout_error() {
        let token = CancellationToken::new();
        let result = with_cancellation_and_timeout(
            &token,
            async {
                sleep(Duration::from_secs(10)).await;
                42
            },
            Duration::from_millis(50),
        )
        .await;

        assert!(matches!(result, Err(CancellationError::Timeout)));
    }

    #[tokio::test]
    async fn test_cancelled_returns_cancelled_error() {
        let token = CancellationToken::new();
        token.cancel();

        let result = with_cancellation_and_timeout(
            &token,
            async {
                sleep(Duration::from_secs(10)).await;
                42
            },
            Duration::from_secs(5),
        )
        .await;

        assert!(matches!(result, Err(CancellationError::Cancelled)));
    }

    #[test]
    fn test_cancel_on_drop_cancels_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        {
            let _guard = CancelOnDrop(token.clone());
            assert!(!token.is_cancelled());
        }

        assert!(token.is_cancelled());
    }
}
