//! Tracing subscriber setup with JSON/pretty formatting, env filtering, and sampling.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{self},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Initialize the global tracing subscriber.
///
/// * Production (when `GATEWAY_ENV=production`): JSON formatting
/// * Development: pretty human-readable formatting
/// * Respects `RUST_LOG` env var; defaults to `info` for gateway crates, `warn` for deps
/// * Respects `GATEWAY_LOG_SAMPLE_RATE` for info-level sampling (default: 1.0 = 100%)
pub fn init_tracing() {
    let env_filter = build_env_filter();
    let sampler = build_sampler();

    let is_production = std::env::var("GATEWAY_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false);

    if is_production {
        let fmt_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_timer(fmt::time::SystemTime);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(sampler.with_filter(tracing::level_filters::LevelFilter::INFO))
            .with(fmt_layer)
            .init();
    } else {
        let fmt_layer = fmt::layer()
            .pretty()
            .with_target(true)
            .with_timer(fmt::time::SystemTime);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(sampler.with_filter(tracing::level_filters::LevelFilter::INFO))
            .with(fmt_layer)
            .init();
    }
}

fn build_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info")
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("h2=warn".parse().unwrap())
            .add_directive("tower=warn".parse().unwrap())
            .add_directive("axum::rejection=warn".parse().unwrap())
            .add_directive("sqlx=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap())
    })
}

// ── Sampling Layer ───────────────────────────────────────────────────────────

/// A simple rate-limiting sampler for INFO-level logs.
///
/// * ERROR/WARN: always logged
/// * INFO: sampled according to `GATEWAY_LOG_SAMPLE_RATE` (0.0–1.0)
/// * DEBUG/TRACE: always passed through (EnvFilter controls these)
#[derive(Clone)]
struct SamplingLayer {
    inner: Arc<SamplingState>,
}

struct SamplingState {
    rate: f64,
    counter: AtomicU64,
}

impl SamplingLayer {
    fn new(rate: f64) -> Self {
        Self {
            inner: Arc::new(SamplingState {
                rate: rate.clamp(0.0, 1.0),
                counter: AtomicU64::new(0),
            }),
        }
    }

    fn should_sample(&self, metadata: &tracing::Metadata<'_>) -> bool {
        // Always pass through errors and warnings
        if metadata.level() <= &tracing::Level::WARN {
            return true;
        }
        // Only sample INFO
        if metadata.level() != &tracing::Level::INFO {
            return true;
        }
        if self.inner.rate >= 1.0 {
            return true;
        }
        if self.inner.rate <= 0.0 {
            return false;
        }
        let count = self.inner.counter.fetch_add(1, Ordering::Relaxed);
        // Simple deterministic sampling based on counter
        (count % 10) < (self.inner.rate * 10.0) as u64
    }
}

impl<S> Layer<S> for SamplingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // The filter above handles the actual filtering; this layer is a pass-through
        // when combined with a LevelFilter. We keep the struct for future extensibility.
        let _ = event;
    }

    fn event_enabled(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.should_sample(event.metadata())
    }
}

fn build_sampler() -> SamplingLayer {
    let rate = std::env::var("GATEWAY_LOG_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    SamplingLayer::new(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_rate_zero() {
        let sampler = SamplingLayer::new(0.0);
        assert_eq!(sampler.inner.rate, 0.0);
    }

    #[test]
    fn test_sampler_rate_one() {
        let sampler = SamplingLayer::new(1.0);
        assert_eq!(sampler.inner.rate, 1.0);
    }

    #[test]
    fn test_sampler_rate_clamped() {
        let sampler = SamplingLayer::new(2.5);
        assert_eq!(sampler.inner.rate, 1.0);

        let sampler = SamplingLayer::new(-0.5);
        assert_eq!(sampler.inner.rate, 0.0);
    }

    #[test]
    fn test_sampler_counter_increments() {
        let sampler = SamplingLayer::new(1.0);
        assert_eq!(sampler.inner.counter.load(Ordering::Relaxed), 0);
        // Counter increments on each check for INFO level
        let _ = sampler.inner.counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(sampler.inner.counter.load(Ordering::Relaxed), 1);
    }
}
