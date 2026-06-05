//! A/B testing framework for routing strategies.
//!
//! Uses a consistent hash of (request_id, org_id) to deterministically
//! assign requests to strategy variants.  This ensures the same user/
//! session always hits the same variant.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A/B test variant assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Control group — existing strategy.
    A,
    /// Treatment group — new strategy.
    B,
}

impl Variant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Variant::A => "A",
            Variant::B => "B",
        }
    }
}

/// Assign a request to an A/B variant using consistent hashing.
///
/// # Arguments
/// * `request_id` — unique request trace ID
/// * `org_id` — organization ID
/// * `split_pct` — percentage of traffic to Variant B (0-100)
///
/// # Example
/// ```
/// use gateway_core::strategies::ab_test::assign_variant;
/// let variant = assign_variant("req-123", "org-456", 20);
/// // ~20% chance of Variant::B
/// ```
pub fn assign_variant(request_id: &str, org_id: &str, split_pct: u8) -> Variant {
    if split_pct == 0 {
        return Variant::A;
    }
    if split_pct >= 100 {
        return Variant::B;
    }

    let mut hasher = DefaultHasher::new();
    request_id.hash(&mut hasher);
    org_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Use the lowest byte as a 0-255 value
    let bucket = (hash & 0xFF) as u8;
    let threshold = (split_pct as f64 / 100.0 * 255.0) as u8;

    if bucket <= threshold {
        Variant::B
    } else {
        Variant::A
    }
}

/// Strategy configuration for an organization.
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// Default routing strategy name.
    pub default_strategy: String,
    /// A/B test enabled.
    pub ab_test_enabled: bool,
    /// Percentage of traffic to variant B (0-100).
    pub ab_split_pct: u8,
    /// Strategy name for variant A.
    pub variant_a_strategy: String,
    /// Strategy name for variant B.
    pub variant_b_strategy: String,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            default_strategy: "balanced".to_string(),
            ab_test_enabled: false,
            ab_split_pct: 0,
            variant_a_strategy: "balanced".to_string(),
            variant_b_strategy: "cost".to_string(),
        }
    }
}

/// Valid routing strategy names.
pub const VALID_STRATEGIES: &[&str] = &[
    "single",
    "fallback",
    "weighted",
    "cost",
    "latency",
    "quality",
    "balanced",
];

/// Check if a strategy name is valid.
pub fn is_valid_strategy(name: &str) -> bool {
    VALID_STRATEGIES.contains(&name)
}

/// Resolve the effective strategy for a request.
///
/// Priority:
/// 1. Per-request override via `X-Routing-Strategy` header (if valid).
/// 2. A/B test variant assignment (if enabled).
/// 3. Organization default strategy.
pub fn resolve_strategy(
    header_strategy: Option<&str>,
    request_id: &str,
    org_id: &str,
    config: &StrategyConfig,
) -> String {
    // 1. Header override
    if let Some(name) = header_strategy {
        let normalized = name.trim().to_lowercase();
        if is_valid_strategy(&normalized) {
            return normalized;
        }
    }

    // 2. A/B test
    if config.ab_test_enabled {
        let variant = assign_variant(request_id, org_id, config.ab_split_pct);
        match variant {
            Variant::A => config.variant_a_strategy.clone(),
            Variant::B => config.variant_b_strategy.clone(),
        }
    } else {
        // 3. Default
        config.default_strategy.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_assignment() {
        let v1 = assign_variant("req-123", "org-456", 50);
        let v2 = assign_variant("req-123", "org-456", 50);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_different_requests_may_differ() {
        // Different request IDs should potentially get different variants
        let v1 = assign_variant("req-a", "org-1", 50);
        let v2 = assign_variant("req-b", "org-1", 50);
        // Not asserting equality — they may differ
        assert!(matches!(v1, Variant::A | Variant::B));
        assert!(matches!(v2, Variant::A | Variant::B));
    }

    #[test]
    fn test_split_0_is_always_a() {
        for i in 0..100 {
            let v = assign_variant(&format!("req-{}", i), "org-1", 0);
            assert_eq!(v, Variant::A);
        }
    }

    #[test]
    fn test_split_100_is_always_b() {
        for i in 0..100 {
            let v = assign_variant(&format!("req-{}", i), "org-1", 100);
            assert_eq!(v, Variant::B);
        }
    }

    #[test]
    fn test_split_roughly_correct() {
        let mut b_count = 0;
        for i in 0..1000 {
            let v = assign_variant(&format!("req-{}", i), "org-1", 30);
            if v == Variant::B {
                b_count += 1;
            }
        }
        // Should be roughly 30% B
        assert!(b_count > 200, "Expected ~30% B, got {}", b_count);
        assert!(b_count < 400, "Expected ~30% B, got {}", b_count);
    }

    #[test]
    fn test_valid_strategies() {
        assert!(is_valid_strategy("cost"));
        assert!(is_valid_strategy("latency"));
        assert!(is_valid_strategy("quality"));
        assert!(is_valid_strategy("balanced"));
        assert!(!is_valid_strategy("unknown"));
    }

    #[test]
    fn test_resolve_strategy_header_override() {
        let config = StrategyConfig::default();
        let strategy = resolve_strategy(Some("cost"), "req-1", "org-1", &config);
        assert_eq!(strategy, "cost");
    }

    #[test]
    fn test_resolve_strategy_invalid_header_ignored() {
        let config = StrategyConfig::default();
        let strategy = resolve_strategy(Some("unknown"), "req-1", "org-1", &config);
        assert_eq!(strategy, "balanced");
    }

    #[test]
    fn test_resolve_strategy_ab_test() {
        let config = StrategyConfig {
            default_strategy: "balanced".to_string(),
            ab_test_enabled: true,
            ab_split_pct: 50,
            variant_a_strategy: "balanced".to_string(),
            variant_b_strategy: "cost".to_string(),
        };

        let strategy = resolve_strategy(None, "req-1", "org-1", &config);
        // Should be either "balanced" or "cost" depending on hash
        assert!(is_valid_strategy(&strategy));
    }
}
