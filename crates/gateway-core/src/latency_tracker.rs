//! Latency tracking per provider-model using Redis circular buffers.
//!
//! Stores the last 100 latency measurements per (org, provider, model) in a
//! Redis list.  Provides p50 / p90 / p99 percentiles and an EMA (exponential
//! moving average) for smoothing.

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use tracing::error;

const MAX_SAMPLES: isize = 100;
const MIN_SAMPLES_FOR_ROUTING: usize = 10;
const EMA_ALPHA: f64 = 0.3;

/// Record a latency sample for a provider-model.
///
/// Stores the raw measurement in a Redis list (circular buffer) and
/// updates the EMA in a separate key.
pub async fn record_latency(
    redis: &mut ConnectionManager,
    org_id: &str,
    provider_config_id: &str,
    model_id: &str,
    latency_ms: u64,
) {
    let list_key = latency_list_key(org_id, provider_config_id, model_id);
    let ema_key = latency_ema_key(org_id, provider_config_id, model_id);

    let latency_f = latency_ms as f64;

    let pipe_result: Result<((), ()), redis::RedisError> = redis::pipe()
        .lpush(&list_key, latency_f)
        .ltrim(&list_key, 0, MAX_SAMPLES - 1)
        .query_async(redis)
        .await;

    if let Err(e) = pipe_result {
        error!(error = %e, "Failed to record latency sample");
        return;
    }

    // Update EMA: ema = alpha * new + (1 - alpha) * old_ema
    let old_ema: Result<f64, _> = redis.get(&ema_key).await;
    let new_ema = match old_ema {
        Ok(old) => EMA_ALPHA * latency_f + (1.0 - EMA_ALPHA) * old,
        Err(_) => latency_f, // first sample
    };

    let _: Result<(), _> = redis.set_ex(&ema_key, new_ema, 86400).await;
}

/// Latency statistics for a provider-model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyStats {
    pub sample_count: usize,
    pub p50_ms: u64,
    pub p90_ms: u64,
    pub p99_ms: u64,
    pub ema_ms: u64,
}

impl LatencyStats {
    /// Returns true if there are enough samples for latency-based routing.
    pub fn has_enough_samples(&self) -> bool {
        self.sample_count >= MIN_SAMPLES_FOR_ROUTING
    }
}

/// Fetch latency statistics for a provider-model.
pub async fn get_latency_stats(
    redis: &mut ConnectionManager,
    org_id: &str,
    provider_config_id: &str,
    model_id: &str,
) -> Option<LatencyStats> {
    let list_key = latency_list_key(org_id, provider_config_id, model_id);
    let ema_key = latency_ema_key(org_id, provider_config_id, model_id);

    let samples: Vec<f64> = match redis.lrange(&list_key, 0, MAX_SAMPLES - 1).await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "Failed to fetch latency samples");
            return None;
        }
    };

    if samples.is_empty() {
        return None;
    }

    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p50 = percentile(&sorted, 0.5);
    let p90 = percentile(&sorted, 0.9);
    let p99 = percentile(&sorted, 0.99);
    let ema: f64 = redis.get(&ema_key).await.unwrap_or(p50);

    Some(LatencyStats {
        sample_count: samples.len(),
        p50_ms: p50 as u64,
        p90_ms: p90 as u64,
        p99_ms: p99 as u64,
        ema_ms: ema as u64,
    })
}

/// Compute a percentile from a sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = (sorted.len() - 1) as f64 * p;
    let lower = idx.floor() as usize;
    let upper = idx.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let frac = idx - lower as f64;
        sorted[lower] * (1.0 - frac) + sorted[upper] * frac
    }
}

fn latency_list_key(org_id: &str, provider_config_id: &str, model_id: &str) -> String {
    format!("latency:{org_id}:{provider_config_id}:{model_id}")
}

fn latency_ema_key(org_id: &str, provider_config_id: &str, model_id: &str) -> String {
    format!("latency:ema:{org_id}:{provider_config_id}:{model_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(percentile(&data, 0.5), 5.5);
        assert_eq!(percentile(&data, 0.9), 9.1);
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 1.0), 10.0);
    }

    #[test]
    fn test_ema_smoothing() {
        // EMA with alpha=0.3:
        // sample 1: 100 → ema = 100
        // sample 2: 200 → ema = 0.3*200 + 0.7*100 = 130
        // sample 3: 100 → ema = 0.3*100 + 0.7*130 = 121
        let mut ema = 100.0f64;
        ema = EMA_ALPHA * 200.0 + (1.0 - EMA_ALPHA) * ema;
        assert!((ema - 130.0).abs() < 0.001);
        ema = EMA_ALPHA * 100.0 + (1.0 - EMA_ALPHA) * ema;
        assert!((ema - 121.0).abs() < 0.001);
    }
}
