//! Latency-based routing strategy.
//!
//! Selects the provider with the lowest p50 latency that supports the
//! requested model.  Falls back to random selection when insufficient
//! samples are available.

use gateway_db::Target;

use crate::latency_tracker::LatencyStats;

/// Provider candidate with latency statistics.
#[derive(Debug, Clone)]
pub struct LatencyCandidate {
    pub target: Target,
    pub stats: LatencyStats,
}

/// Select the provider with the lowest p50 latency.
///
/// # Arguments
/// * `candidates` — list of providers with latency stats
/// * `latency_sla_ms` — providers with p50 above this are penalized
///
/// # Selection logic
/// 1. Filter candidates with enough samples.
/// 2. Compute routing score: `score = p50 * penalty_factor` where
///    `penalty_factor = 1.0` if p50 ≤ SLA, else `1.5`.
/// 3. Select candidate with lowest score.
/// 4. If no candidate has enough samples, return `None` (caller should fall back).
pub fn select_lowest_latency(
    candidates: &[LatencyCandidate],
    latency_sla_ms: u64,
) -> Option<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.stats.has_enough_samples())
        .collect();

    if viable.is_empty() {
        return None;
    }

    let sla_f = latency_sla_ms as f64;
    viable.sort_by(|a, b| {
        let score_a = routing_score(a.stats.p50_ms, sla_f);
        let score_b = routing_score(b.stats.p50_ms, sla_f);
        score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    Some(viable[0].target.clone())
}

/// Build a fallback chain ordered by latency (lowest first).
pub fn build_latency_fallback_chain(
    candidates: &[LatencyCandidate],
    latency_sla_ms: u64,
) -> Vec<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.stats.has_enough_samples())
        .collect();

    let sla_f = latency_sla_ms as f64;
    viable.sort_by(|a, b| {
        let score_a = routing_score(a.stats.p50_ms, sla_f);
        let score_b = routing_score(b.stats.p50_ms, sla_f);
        score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    viable.into_iter().map(|c| c.target.clone()).collect()
}

/// Routing score with SLA penalty.
fn routing_score(p50_ms: u64, sla_ms: f64) -> f64 {
    let p50 = p50_ms as f64;
    let penalty = if p50 > sla_ms { 1.5 } else { 1.0 };
    p50 * penalty
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use super::*;

    fn make_candidate(model_id: &str, p50_ms: u64, sample_count: usize) -> LatencyCandidate {
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: model_id.to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            stats: LatencyStats {
                sample_count,
                p50_ms,
                p90_ms: p50_ms * 2,
                p99_ms: p50_ms * 3,
                ema_ms: p50_ms,
            },
        }
    }

    #[test]
    fn test_selects_lowest_latency() {
        let candidates = vec![
            make_candidate("fast", 100, 20),
            make_candidate("slow", 500, 20),
        ];

        let selected = select_lowest_latency(&candidates, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "fast");
    }

    #[test]
    fn test_insufficient_samples_fallback() {
        let candidates = vec![
            make_candidate("A", 100, 5),
            make_candidate("B", 200, 5),
        ];

        let selected = select_lowest_latency(&candidates, 10_000);
        assert!(selected.is_none());
    }

    #[test]
    fn test_sla_penalty() {
        // Provider A: p50=400, SLA=350 → score=400*1.0=400 (within SLA)
        // Provider B: p50=300, SLA=350 → score=300*1.0=300 (within SLA, lowest)
        // Provider C: p50=450, SLA=350 → score=450*1.5=675 (above SLA, penalized)
        let candidates = vec![
            make_candidate("A", 400, 20),
            make_candidate("B", 300, 20),
            make_candidate("C", 450, 20),
        ];

        let selected = select_lowest_latency(&candidates, 350);
        assert!(selected.is_some());
        // B has the best score (300) because it's the lowest and within SLA
        assert_eq!(selected.unwrap().model_id, "B");
    }

    #[test]
    fn test_fallback_chain_ordering() {
        let candidates = vec![
            make_candidate("A", 300, 20),
            make_candidate("B", 100, 20),
            make_candidate("C", 200, 20),
        ];

        let chain = build_latency_fallback_chain(&candidates, 10_000);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].model_id, "B");
        assert_eq!(chain[1].model_id, "C");
        assert_eq!(chain[2].model_id, "A");
    }
}
