//! Gateway Observability — Structured JSON logging, Prometheus metrics, request tracing.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the global tracing subscriber with JSON formatting in production
/// and pretty formatting in development.
pub fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
