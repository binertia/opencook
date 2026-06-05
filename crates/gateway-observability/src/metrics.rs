//! Prometheus metrics for the AI Gateway.
//!
//! Uses the `metrics` facade crate with `metrics-exporter-prometheus` for exposition.
//! All metrics are prefixed with `gateway_`.

use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use std::sync::OnceLock;

/// Global Prometheus recorder handle.
static PROMETHEUS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

/// Initialize the Prometheus metrics exporter.
/// Call once at startup. Returns the handle for scraping metrics text.
pub fn init_metrics() -> metrics_exporter_prometheus::PrometheusHandle {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("gateway_request_duration_ms".to_owned()),
            &[
                1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0,
                1000.0, 2500.0, 5000.0, 10000.0,
            ],
        )
        .expect("Failed to set histogram buckets")
        .install_recorder()
        .expect("Failed to install Prometheus recorder");

    let _ = PROMETHEUS_HANDLE.set(handle.clone());
    handle
}

/// Get the global Prometheus handle for rendering metrics text.
pub fn handle() -> Option<metrics_exporter_prometheus::PrometheusHandle> {
    PROMETHEUS_HANDLE.get().cloned()
}

// ── Request Metrics ──────────────────────────────────────────────────────────

/// Record a completed request.
/// `status`: "success" | "error" | "quota_exceeded" | "rate_limited" | "cancelled"
pub fn record_request(model: &str, provider: &str, status: &str, duration_ms: f64) {
    let model = sanitize_label(model);
    let provider = sanitize_label(provider);
    let status = sanitize_label(status);

    metrics::histogram!("gateway_request_duration_ms", "model" => model.clone(), "provider" => provider.clone(), "status" => status.clone()).record(duration_ms);
    metrics::counter!("gateway_request_total", "model" => model, "provider" => provider, "status" => status).increment(1);
}

// ── Cache Metrics ────────────────────────────────────────────────────────────

/// Record an L1 cache hit.
pub fn record_cache_hit_l1() {
    metrics::counter!("gateway_cache_hit_total", "layer" => "l1").increment(1);
}

/// Record an L2 cache hit.
pub fn record_cache_hit_l2() {
    metrics::counter!("gateway_cache_hit_total", "layer" => "l2").increment(1);
}

/// Record a semantic cache hit.
pub fn record_cache_hit_semantic() {
    metrics::counter!("gateway_cache_hit_total", "layer" => "semantic").increment(1);
}

/// Record a cache miss.
pub fn record_cache_miss() {
    metrics::counter!("gateway_cache_miss_total").increment(1);
}

// ── Token & Cost Metrics ─────────────────────────────────────────────────────

/// Record token usage for a request.
pub fn record_tokens(model: &str, prompt_tokens: u64, completion_tokens: u64) {
    let model = sanitize_label(model);
    metrics::counter!("gateway_tokens_total", "type" => "input", "model" => model.clone()).increment(prompt_tokens);
    metrics::counter!("gateway_tokens_total", "type" => "output", "model" => model).increment(completion_tokens);
}

/// Record cost for a request (in USD).
/// Stored as micro-dollars (USD × 1_000_000) to fit in u64 counter.
pub fn record_cost(model: &str, provider: &str, cost_usd: f64) {
    let model = sanitize_label(model);
    let provider = sanitize_label(provider);
    let micro_dollars = (cost_usd.max(0.0) * 1_000_000.0) as u64;
    metrics::counter!("gateway_cost_total", "model" => model, "provider" => provider).increment(micro_dollars);
}

// ── Quota & Rate Limit Metrics ───────────────────────────────────────────────

/// Record a quota exceeded event.
pub fn record_quota_exceeded(metric: &str, scope: &str) {
    let metric = sanitize_label(metric);
    let scope = sanitize_label(scope);
    metrics::counter!("gateway_quota_exceeded_total", "metric" => metric, "scope" => scope).increment(1);
}

/// Record a rate-limited request.
pub fn record_rate_limited(key_id: &str) {
    // Hash the key_id to avoid unbounded cardinality
    let key_hash = blake3::hash(key_id.as_bytes()).to_hex()[..16].to_string();
    metrics::counter!("gateway_rate_limited_total", "key_hash" => key_hash).increment(1);
}

// ── Provider Health Metrics ──────────────────────────────────────────────────

/// Set provider health gauge. `healthy`: 1.0 = healthy, 0.0 = unhealthy.
pub fn set_provider_health(provider: &str, org_id: &str, healthy: bool) {
    let provider = sanitize_label(provider);
    let org_hash = &blake3::hash(org_id.as_bytes()).to_hex()[..16];
    let value = if healthy { 1.0 } else { 0.0 };
    metrics::gauge!("gateway_provider_health", "provider" => provider, "org" => org_hash.to_string()).set(value);
}

// ── Connection Metrics ───────────────────────────────────────────────────────

/// Increment active connections gauge.
pub fn inc_active_connections() {
    metrics::gauge!("gateway_active_connections").increment(1.0);
}

/// Decrement active connections gauge.
pub fn dec_active_connections() {
    metrics::gauge!("gateway_active_connections").decrement(1.0);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Sanitize a string for use as a Prometheus label value.
/// Replaces non-alphanumeric characters with underscores.
fn sanitize_label(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_label() {
        assert_eq!(sanitize_label("gpt-4o"), "gpt-4o");
        assert_eq!(sanitize_label("claude-3.5-sonnet"), "claude-3_5-sonnet");
        assert_eq!(sanitize_label("model/with/slash"), "model_with_slash");
    }
}
