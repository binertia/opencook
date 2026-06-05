//! Cache metrics wrappers.
//!
//! Thin facade over `gateway_observability::metrics` for cache-specific events.

/// Record an L1 cache hit.
pub fn record_hit_l1() {
    gateway_observability::metrics::record_cache_hit_l1();
}

/// Record an L2 cache hit.
pub fn record_hit_l2() {
    gateway_observability::metrics::record_cache_hit_l2();
}

/// Record a semantic cache hit.
pub fn record_hit_semantic() {
    gateway_observability::metrics::record_cache_hit_semantic();
}

/// Record a cache miss.
pub fn record_miss() {
    gateway_observability::metrics::record_cache_miss();
}

/// Record a cache hit for a specific model.
pub fn record_hit_by_model(model: &str) {
    gateway_observability::metrics::record_cache_hit_by_model(model);
}

/// Record estimated cost saved from a cache hit.
pub fn record_cost_saved(cost_usd: f64) {
    gateway_observability::metrics::record_cache_cost_saved(cost_usd);
}

/// Record a semantic cache hit.
pub fn record_semantic_hit() {
    gateway_observability::metrics::record_semantic_cache_hit();
}

/// Record a semantic cache miss.
pub fn record_semantic_miss() {
    gateway_observability::metrics::record_semantic_cache_miss();
}

/// Set semantic cache entry count gauge.
pub fn set_semantic_entries(count: usize) {
    gateway_observability::metrics::set_semantic_cache_entries(count);
}

/// Record semantic cache embedding generation latency (ms).
pub fn record_semantic_embedding_latency(latency_ms: f64) {
    gateway_observability::metrics::record_semantic_cache_embedding_latency(latency_ms);
}
