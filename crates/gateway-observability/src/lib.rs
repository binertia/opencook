//! Gateway Observability — Structured JSON logging, Prometheus metrics, request tracing.

pub mod metrics;
pub mod redaction;
pub mod request_log;
pub mod tracing_setup;

/// Initialize the global tracing subscriber.
///
/// Delegates to [`tracing_setup::init_tracing`] which supports:
/// * JSON formatting in production (`GATEWAY_ENV=production`)
/// * Pretty formatting in development
/// * `RUST_LOG` env var filtering
/// * `GATEWAY_LOG_SAMPLE_RATE` for info-level sampling
pub fn init_tracing() {
    tracing_setup::init_tracing();
}
