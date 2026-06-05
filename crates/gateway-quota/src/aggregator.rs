//! Usage aggregation worker — rolls up raw request logs into `usage_records`.
//!
//! Runs periodically as a background task. Aggregates unprocessed requests
//! from completed hours into `usage_records` with hourly granularity.

use std::sync::Arc;
use std::time::Duration;

use gateway_db::repos::usage_repo::UsageRepo;
use gateway_db::DbBackend;
use tokio::sync::Notify;
use tokio::time::interval;
use tracing::{error, info};

/// Background worker that aggregates request logs into usage records.
pub struct AggregationWorker {
    pool: DbBackend,
    shutdown: Arc<Notify>,
    interval_secs: u64,
}

impl AggregationWorker {
    /// Create a new aggregation worker.
    ///
    /// `interval_secs` controls how often aggregation runs (default: 60).
    pub fn new(pool: DbBackend, interval_secs: u64) -> Self {
        Self {
            pool,
            shutdown: Arc::new(Notify::new()),
            interval_secs,
        }
    }

    /// Start the worker as a background task.
    ///
    /// Returns a handle that can be used to trigger graceful shutdown.
    pub fn spawn(self) -> Arc<Notify> {
        let shutdown = self.shutdown.clone();
        tokio::spawn(self.run());
        shutdown
    }

    /// Run the aggregation loop.
    async fn run(self) {
        let mut ticker = interval(Duration::from_secs(self.interval_secs));
        let repo = UsageRepo::new(self.pool);

        info!(
            interval_secs = self.interval_secs,
            "Usage aggregation worker started"
        );

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match Self::aggregate_once(&repo).await {
                        Ok(count) => {
                            if count > 0 {
                                info!(aggregated_rows = count, "Usage aggregation completed");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Usage aggregation failed");
                        }
                    }
                }
                _ = self.shutdown.notified() => {
                    info!("Usage aggregation worker shutting down");
                    // Run one final aggregation before exiting
                    match Self::aggregate_once(&repo).await {
                        Ok(count) => {
                            info!(final_aggregated_rows = count, "Final usage aggregation completed");
                        }
                        Err(e) => {
                            error!(error = %e, "Final usage aggregation failed");
                        }
                    }
                    break;
                }
            }
        }

        info!("Usage aggregation worker stopped");
    }

    /// Perform a single aggregation pass.
    ///
    /// 1. Aggregate unprocessed requests into `usage_records` (ON CONFLICT UPDATE).
    /// 2. Mark those requests as aggregated.
    ///
    /// Returns the number of requests marked as aggregated.
    async fn aggregate_once(repo: &UsageRepo) -> Result<u64, AggregationError> {
        // Step 1: aggregate into usage_records
        let usage_rows = repo.aggregate_hourly().await?;

        // Step 2: mark requests as aggregated
        let request_rows = repo.mark_requests_aggregated().await?;

        if usage_rows > 0 || request_rows > 0 {
            info!(
                usage_records_affected = usage_rows,
                requests_marked = request_rows,
                "Aggregation pass complete"
            );
        }

        Ok(request_rows)
    }

    /// Trigger a graceful shutdown.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

/// Errors that can occur during aggregation.
#[derive(Debug, thiserror::Error)]
pub enum AggregationError {
    #[error("Database error: {0}")]
    Database(#[from] gateway_db::error::DbError),
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_aggregation_worker_new() {
        // Compile-time check only; real tests need a DbBackend.
        // In integration tests, create a test DB, insert requests,
        // run aggregate_once, and verify usage_records contents.
    }
}
