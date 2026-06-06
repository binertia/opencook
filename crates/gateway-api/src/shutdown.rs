//! Graceful shutdown and Unix signal handling.
//!
//! SIGTERM / SIGINT  → initiate graceful shutdown (drain for up to 30s)
//! SIGHUP            → trigger config reload
//! SIGUSR1           → reopen log files (no-op for stdout/stderr)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{timeout, Instant};

/// Shared shutdown state.
#[derive(Clone, Debug)]
pub struct ShutdownState {
    shutting_down: Arc<AtomicBool>,
    tx: broadcast::Sender<()>,
}

impl ShutdownState {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1);
        Self {
            shutting_down: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    /// Returns true if graceful shutdown has been initiated.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Initiate shutdown and notify waiters.
    pub fn shutdown(&self) {
        if !self.shutting_down.swap(true, Ordering::SeqCst) {
            tracing::info!("shutdown signal received — initiating graceful shutdown");
            let _ = self.tx.send(());
        }
    }

    /// Wait for the shutdown signal.
    pub fn notified(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        let shutting_down = Arc::clone(&self.shutting_down);
        let mut rx = self.tx.subscribe();
        async move {
            if shutting_down.load(Ordering::SeqCst) {
                return;
            }
            match rx.recv().await {
                Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {}
            }
        }
    }
}

impl Default for ShutdownState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the signal handler loop.
///
/// On Unix: listens for SIGTERM, SIGINT, SIGHUP, SIGUSR1.
/// On non-Unix: falls back to Ctrl-C (SIGINT equivalent).
pub fn spawn_signal_handler(shutdown: ShutdownState, reload_tx: tokio::sync::mpsc::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
            let mut sighup =
                signal(SignalKind::hangup()).expect("failed to install SIGHUP handler");
            let mut sigusr1 =
                signal(SignalKind::user_defined1()).expect("failed to install SIGUSR1 handler");

            loop {
                tokio::select! {
                    _ = sigterm.recv() => {
                        tracing::info!("received SIGTERM");
                        shutdown.shutdown();
                        break;
                    }
                    _ = sigint.recv() => {
                        tracing::info!("received SIGINT");
                        shutdown.shutdown();
                        break;
                    }
                    _ = sighup.recv() => {
                        tracing::info!("received SIGHUP — triggering config reload");
                        let _ = reload_tx.send(()).await;
                    }
                    _ = sigusr1.recv() => {
                        tracing::info!("received SIGUSR1 — log rotation (no-op for stdout/stderr)");
                    }
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl-C handler");
            tracing::info!("received Ctrl-C");
            shutdown.shutdown();
        }
    });
}

/// Wait for graceful shutdown with a maximum drain timeout.
pub async fn wait_for_shutdown(shutdown: ShutdownState, drain_timeout: Duration) {
    shutdown.notified().await;

    let start = Instant::now();
    tracing::info!("draining in-flight requests (max {:?})", drain_timeout);

    // Give tokio runtime a chance to finish current tasks.
    // In practice, the HTTP server graceful shutdown handles connection draining.
    let _ = timeout(drain_timeout, tokio::task::yield_now()).await;

    let elapsed = start.elapsed();
    tracing::info!(
        elapsed_ms = elapsed.as_millis(),
        "graceful shutdown complete — exiting"
    );
}
