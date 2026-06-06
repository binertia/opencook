//! Cost-optimized routing strategy.
//!
//! Selects the cheapest capable provider for a requested model,
//! filtering out unhealthy providers and those exceeding latency SLA.

use gateway_db::Target;
use rust_decimal::Decimal;

/// Health status of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    /// Convert to a numeric score for tie-breaking (higher = better).
    pub fn score(self) -> i32 {
        match self {
            HealthStatus::Healthy => 3,
            HealthStatus::Degraded => 2,
            HealthStatus::Unhealthy => 1,
        }
    }
}

/// Provider candidate with pricing and health info.
#[derive(Debug, Clone)]
pub struct ProviderCandidate {
    pub target: Target,
    pub input_cost_per_1k: Decimal,
    pub output_cost_per_1k: Decimal,
    pub health: HealthStatus,
    /// Latency in milliseconds (p50).
    pub latency_ms: u64,
}

/// Estimate cost for a request given token estimates.
///
/// For prompt-only routing (no completion estimate), uses `max_tokens` as
/// a conservative estimate for output tokens.
pub fn estimate_cost(
    candidate: &ProviderCandidate,
    estimated_prompt_tokens: u64,
    estimated_completion_tokens: u64,
) -> Decimal {
    let input_cost =
        Decimal::from(estimated_prompt_tokens) * candidate.input_cost_per_1k / Decimal::from(1000);
    let output_cost = Decimal::from(estimated_completion_tokens) * candidate.output_cost_per_1k
        / Decimal::from(1000);
    input_cost + output_cost
}

/// Select the cheapest healthy provider for a model.
///
/// # Arguments
/// * `candidates` — list of providers supporting the requested model
/// * `estimated_prompt_tokens` — estimated input tokens
/// * `estimated_completion_tokens` — estimated output tokens (uses max_tokens if None)
/// * `max_latency_ms` — SLA threshold in ms; providers above this are skipped
///
/// # Selection logic
/// 1. Filter out unhealthy providers and those above latency SLA.
/// 2. Sort by estimated cost (ascending).
/// 3. On cost tie: prefer healthier provider.
/// 4. On health tie: prefer lower latency.
pub fn select_cheapest(
    candidates: &[ProviderCandidate],
    estimated_prompt_tokens: u64,
    estimated_completion_tokens: u64,
    max_latency_ms: u64,
) -> Option<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.health != HealthStatus::Unhealthy && c.latency_ms <= max_latency_ms)
        .collect();

    if viable.is_empty() {
        return None;
    }

    viable.sort_by(|a, b| {
        let cost_a = estimate_cost(a, estimated_prompt_tokens, estimated_completion_tokens);
        let cost_b = estimate_cost(b, estimated_prompt_tokens, estimated_completion_tokens);

        cost_a
            .cmp(&cost_b)
            .then_with(|| b.health.score().cmp(&a.health.score()))
            .then_with(|| a.latency_ms.cmp(&b.latency_ms))
    });

    Some(viable[0].target.clone())
}

/// Build a fallback chain from candidates, ordered by cost.
pub fn build_cost_fallback_chain(
    candidates: &[ProviderCandidate],
    estimated_prompt_tokens: u64,
    estimated_completion_tokens: u64,
    max_latency_ms: u64,
) -> Vec<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.health != HealthStatus::Unhealthy && c.latency_ms <= max_latency_ms)
        .collect();

    viable.sort_by(|a, b| {
        let cost_a = estimate_cost(a, estimated_prompt_tokens, estimated_completion_tokens);
        let cost_b = estimate_cost(b, estimated_prompt_tokens, estimated_completion_tokens);

        cost_a
            .cmp(&cost_b)
            .then_with(|| b.health.score().cmp(&a.health.score()))
            .then_with(|| a.latency_ms.cmp(&b.latency_ms))
    });

    viable.into_iter().map(|c| c.target.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_candidate(
        model_id: &str,
        input_cost: &str,
        output_cost: &str,
        health: HealthStatus,
        latency_ms: u64,
    ) -> ProviderCandidate {
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: model_id.to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            input_cost_per_1k: input_cost.parse().unwrap(),
            output_cost_per_1k: output_cost.parse().unwrap(),
            health,
            latency_ms,
        }
    }

    #[test]
    fn test_selects_cheapest_provider() {
        let candidates = vec![
            make_candidate("gpt-4o", "5.00", "15.00", HealthStatus::Healthy, 100),
            make_candidate("gpt-4o-mini", "0.15", "0.60", HealthStatus::Healthy, 150),
        ];

        let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_skips_unhealthy_provider() {
        let candidates = vec![
            make_candidate(
                "cheap-but-dead",
                "0.10",
                "0.20",
                HealthStatus::Unhealthy,
                100,
            ),
            make_candidate(
                "expensive-but-ok",
                "1.00",
                "3.00",
                HealthStatus::Healthy,
                100,
            ),
        ];

        let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "expensive-but-ok");
    }

    #[test]
    fn test_skips_high_latency_provider() {
        let candidates = vec![
            make_candidate("cheap-slow", "0.10", "0.20", HealthStatus::Healthy, 15_000),
            make_candidate("expensive-fast", "1.00", "3.00", HealthStatus::Healthy, 100),
        ];

        let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "expensive-fast");
    }

    #[test]
    fn test_tiebreak_by_health_then_latency() {
        let candidates = vec![
            make_candidate("A", "1.00", "1.00", HealthStatus::Degraded, 200),
            make_candidate("B", "1.00", "1.00", HealthStatus::Healthy, 300),
            make_candidate("C", "1.00", "1.00", HealthStatus::Healthy, 100),
        ];

        let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
        assert!(selected.is_some());
        // B and C have same cost, both healthy, but C has lower latency
        assert_eq!(selected.unwrap().model_id, "C");
    }

    #[test]
    fn test_no_viable_providers() {
        let candidates = vec![
            make_candidate("unhealthy", "0.10", "0.20", HealthStatus::Unhealthy, 100),
            make_candidate("slow", "0.10", "0.20", HealthStatus::Healthy, 20_000),
        ];

        let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
        assert!(selected.is_none());
    }

    #[test]
    fn test_estimate_cost_accuracy() {
        let candidate = make_candidate("gpt-4o", "5.00", "15.00", HealthStatus::Healthy, 100);
        let cost = estimate_cost(&candidate, 2000, 1000);
        // input: 2000 * 5.00 / 1000 = 10.00
        // output: 1000 * 15.00 / 1000 = 15.00
        // total = 25.00
        assert_eq!(cost, Decimal::from(25));
    }

    #[test]
    fn test_fallback_chain_ordered_by_cost() {
        let candidates = vec![
            make_candidate("A", "1.00", "1.00", HealthStatus::Healthy, 100),
            make_candidate("B", "0.50", "0.50", HealthStatus::Healthy, 100),
            make_candidate("C", "2.00", "2.00", HealthStatus::Healthy, 100),
        ];

        let chain = build_cost_fallback_chain(&candidates, 1000, 1000, 10_000);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].model_id, "B");
        assert_eq!(chain[1].model_id, "A");
        assert_eq!(chain[2].model_id, "C");
    }
}
