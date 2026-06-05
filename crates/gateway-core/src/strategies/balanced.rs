//! Balanced routing strategy — optimizes across cost, latency, and quality.
//!
//! Computes a composite score from normalized dimensions:
//!   score = weight_cost * norm_cost + weight_latency * norm_latency
//!         + weight_quality * norm_quality
//!
//! where norm_cost and norm_latency are inverted (lower raw = higher norm)
//! because they are "lower is better" dimensions.

use gateway_db::Target;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

use super::quality::model_quality_score;

/// Provider candidate with all three dimensions.
#[derive(Debug, Clone)]
pub struct BalancedCandidate {
    pub target: Target,
    pub input_cost_per_1k: Decimal,
    pub output_cost_per_1k: Decimal,
    pub latency_ms: u64,
    pub estimated_prompt_tokens: u64,
    pub estimated_completion_tokens: u64,
}

/// Weights for the composite score (must sum to 1.0).
#[derive(Debug, Clone, Copy)]
pub struct BalancedWeights {
    pub cost: f64,
    pub latency: f64,
    pub quality: f64,
}

impl Default for BalancedWeights {
    fn default() -> Self {
        Self {
            cost: 0.4,
            latency: 0.4,
            quality: 0.2,
        }
    }
}

impl BalancedWeights {
    /// Validate that weights sum to 1.0 (within floating-point tolerance).
    pub fn is_valid(&self) -> bool {
        let sum = self.cost + self.latency + self.quality;
        (sum - 1.0).abs() < 0.001
    }
}

/// Select the provider with the highest composite score.
///
/// # Normalization
/// For each dimension we compute the raw value, then normalize to [0, 1]
/// where 1.0 = best.
///
/// * Cost: inverted relative to max cost among candidates.
/// * Latency: inverted relative to max latency among candidates.
/// * Quality: direct relative to max quality among candidates.
pub fn select_balanced(
    candidates: &[BalancedCandidate],
    weights: &BalancedWeights,
    latency_sla_ms: u64,
) -> Option<Target> {
    if candidates.is_empty() || !weights.is_valid() {
        return None;
    }

    let mut scored: Vec<_> = candidates
        .iter()
        .filter(|c| c.latency_ms <= latency_sla_ms)
        .map(|c| {
            let cost = estimate_cost(c);
            let quality = model_quality_score(&c.target.model_id);
            (c, cost, quality)
        })
        .collect();

    if scored.is_empty() {
        return None;
    }

    let max_cost = scored.iter().map(|(_, c, _)| *c).fold(0.0, f64::max).max(1e-9);
    let max_latency = scored.iter().map(|(c, _, _)| c.latency_ms).max().unwrap_or(1) as f64;
    let max_quality = scored.iter().map(|(_, _, q)| *q).fold(0.0, f64::max).max(1e-9);

    scored.sort_by(|(a, cost_a, qual_a), (b, cost_b, qual_b)| {
        let score_a = compute_score(
            *cost_a,
            a.latency_ms as f64,
            *qual_a,
            max_cost,
            max_latency,
            max_quality,
            weights,
        );
        let score_b = compute_score(
            *cost_b,
            b.latency_ms as f64,
            *qual_b,
            max_cost,
            max_latency,
            max_quality,
            weights,
        );
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    Some(scored[0].0.target.clone())
}

/// Build a fallback chain ordered by composite score (best first).
pub fn build_balanced_fallback_chain(
    candidates: &[BalancedCandidate],
    weights: &BalancedWeights,
    latency_sla_ms: u64,
) -> Vec<Target> {
    if candidates.is_empty() || !weights.is_valid() {
        return vec![];
    }

    let mut scored: Vec<_> = candidates
        .iter()
        .filter(|c| c.latency_ms <= latency_sla_ms)
        .map(|c| {
            let cost = estimate_cost(c);
            let quality = model_quality_score(&c.target.model_id);
            (c, cost, quality)
        })
        .collect();

    if scored.is_empty() {
        return vec![];
    }

    let max_cost = scored.iter().map(|(_, c, _)| *c).fold(0.0, f64::max).max(1e-9);
    let max_latency = scored.iter().map(|(c, _, _)| c.latency_ms).max().unwrap_or(1) as f64;
    let max_quality = scored.iter().map(|(_, _, q)| *q).fold(0.0, f64::max).max(1e-9);

    scored.sort_by(|(a, cost_a, qual_a), (b, cost_b, qual_b)| {
        let score_a = compute_score(
            *cost_a,
            a.latency_ms as f64,
            *qual_a,
            max_cost,
            max_latency,
            max_quality,
            weights,
        );
        let score_b = compute_score(
            *cost_b,
            b.latency_ms as f64,
            *qual_b,
            max_cost,
            max_latency,
            max_quality,
            weights,
        );
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    scored.into_iter().map(|(c, _, _)| c.target.clone()).collect()
}

fn estimate_cost(candidate: &BalancedCandidate) -> f64 {
    let input_cost = Decimal::from(candidate.estimated_prompt_tokens)
        * candidate.input_cost_per_1k
        / Decimal::from(1000);
    let output_cost = Decimal::from(candidate.estimated_completion_tokens)
        * candidate.output_cost_per_1k
        / Decimal::from(1000);
    let total = input_cost + output_cost;
    total.to_f64().unwrap_or(0.0)
}

fn compute_score(
    cost: f64,
    latency: f64,
    quality: f64,
    max_cost: f64,
    max_latency: f64,
    max_quality: f64,
    weights: &BalancedWeights,
) -> f64 {
    let norm_cost = 1.0 - (cost / max_cost);
    let norm_latency = 1.0 - (latency / max_latency);
    let norm_quality = quality / max_quality;

    weights.cost * norm_cost
        + weights.latency * norm_latency
        + weights.quality * norm_quality
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(
        model_id: &str,
        input_cost: &str,
        output_cost: &str,
        latency_ms: u64,
    ) -> BalancedCandidate {
        BalancedCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: model_id.to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            input_cost_per_1k: input_cost.parse().unwrap(),
            output_cost_per_1k: output_cost.parse().unwrap(),
            latency_ms,
            estimated_prompt_tokens: 1000,
            estimated_completion_tokens: 1000,
        }
    }

    #[test]
    fn test_selects_best_balance() {
        // gpt-4o: high quality, high cost, medium latency
        // gpt-4o-mini: medium quality, low cost, low latency
        let candidates = vec![
            make_candidate("gpt-4o", "5.00", "15.00", 200),
            make_candidate("gpt-4o-mini", "0.15", "0.60", 100),
        ];

        let weights = BalancedWeights::default();
        let selected = select_balanced(&candidates, &weights, 10_000);
        assert!(selected.is_some());
        // With default weights (0.4 cost, 0.4 latency, 0.2 quality),
        // gpt-4o-mini wins on cost and latency, losing on quality.
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_quality_weighted_routing() {
        let candidates = vec![
            make_candidate("gpt-4o", "5.00", "15.00", 200),
            make_candidate("gpt-4o-mini", "0.15", "0.60", 100),
        ];

        // Quality-heavy weights
        let weights = BalancedWeights {
            cost: 0.1,
            latency: 0.1,
            quality: 0.8,
        };
        let selected = select_balanced(&candidates, &weights, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o");
    }

    #[test]
    fn test_cost_weighted_routing() {
        let candidates = vec![
            make_candidate("gpt-4o", "5.00", "15.00", 200),
            make_candidate("gpt-4o-mini", "0.15", "0.60", 100),
        ];

        // Cost-heavy weights
        let weights = BalancedWeights {
            cost: 0.8,
            latency: 0.1,
            quality: 0.1,
        };
        let selected = select_balanced(&candidates, &weights, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_invalid_weights_returns_none() {
        let candidates = vec![make_candidate("gpt-4o", "5.00", "15.00", 200)];
        let weights = BalancedWeights {
            cost: 0.5,
            latency: 0.5,
            quality: 0.5,
        };
        let selected = select_balanced(&candidates, &weights, 10_000);
        assert!(selected.is_none());
    }

    #[test]
    fn test_sla_filtering() {
        let candidates = vec![
            make_candidate("gpt-4o", "5.00", "15.00", 20_000),
            make_candidate("gpt-4o-mini", "0.15", "0.60", 100),
        ];

        let weights = BalancedWeights::default();
        let selected = select_balanced(&candidates, &weights, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_fallback_chain_ordered() {
        let candidates = vec![
            make_candidate("gpt-3.5-turbo", "0.50", "1.50", 80),
            make_candidate("gpt-4o", "5.00", "15.00", 200),
            make_candidate("gpt-4o-mini", "0.15", "0.60", 100),
        ];

        let weights = BalancedWeights::default();
        let chain = build_balanced_fallback_chain(&candidates, &weights, 10_000);
        assert_eq!(chain.len(), 3);
        // gpt-4o-mini should be first (best balance)
        assert_eq!(chain[0].model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_compute_score_bounds() {
        let weights = BalancedWeights::default();
        // Best possible: cost=0, latency=0, quality=max
        let best = compute_score(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, &weights);
        // Worst possible: cost=max, latency=max, quality=0
        let worst = compute_score(1.0, 1.0, 0.0, 1.0, 1.0, 1.0, &weights);

        assert!(best > worst);
        assert!(best <= 1.0);
        assert!(worst >= 0.0);
    }
}
