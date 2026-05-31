//! Routing strategies — determines how providers are selected from rule targets.

use gateway_db::{RoutingRule, Target};
use tracing::warn;
use uuid::Uuid;

/// A routing decision produced by the evaluation engine.
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// The ID of the matching routing rule (if any).
    pub rule_id: Option<Uuid>,
    /// Primary target (always present).
    pub primary: Target,
    /// Fallback chain (empty for single/weighted strategies).
    pub fallback_chain: Vec<Target>,
}

/// Build a routing decision from a rule's targets and strategy.
pub fn build_decision(rule: &RoutingRule) -> RoutingDecision {
    let targets: Vec<Target> = match serde_json::from_value(rule.targets.clone()) {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "Failed to parse rule targets, using empty");
            vec![]
        }
    };

    match rule.strategy.as_str() {
        "single" => {
            let primary = targets.into_iter().next().unwrap_or_else(empty_target);
            RoutingDecision {
                rule_id: Some(rule.id),
                primary,
                fallback_chain: vec![],
            }
        }
        "fallback" => {
            let mut iter = targets.into_iter();
            let primary = iter.next().unwrap_or_else(empty_target);
            let fallback_chain: Vec<Target> = iter.collect();
            RoutingDecision {
                rule_id: Some(rule.id),
                primary,
                fallback_chain,
            }
        }
        "weighted" => {
            let primary = pick_weighted(&targets);
            RoutingDecision {
                rule_id: Some(rule.id),
                primary,
                fallback_chain: vec![],
            }
        }
        _ => {
            warn!(strategy = %rule.strategy, "Unknown routing strategy, defaulting to single");
            let primary = targets.into_iter().next().unwrap_or_else(empty_target);
            RoutingDecision {
                rule_id: Some(rule.id),
                primary,
                fallback_chain: vec![],
            }
        }
    }
}

/// Pick a target using weighted random selection.
fn pick_weighted(targets: &[Target]) -> Target {
    if targets.is_empty() {
        return Target {
            provider_config_id: Uuid::nil(),
            model_id: String::new(),
            provider_kind: None,
            weight: None,
        };
    }
    if targets.len() == 1 {
        return targets[0].clone();
    }

    let total_weight: i32 = targets
        .iter()
        .map(|t| t.weight.unwrap_or(1))
        .sum();

    if total_weight <= 0 {
        return targets[0].clone();
    }

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut pick = rng.gen_range(0..total_weight);
    for target in targets {
        let weight = target.weight.unwrap_or(1);
        if pick < weight {
            return target.clone();
        }
        pick -= weight;
    }

    targets[0].clone()
}

fn empty_target() -> Target {
    Target {
        provider_config_id: Uuid::nil(),
        model_id: String::new(),
        provider_kind: None,
        weight: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(strategy: &str, targets: Vec<Target>) -> RoutingRule {
        RoutingRule {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "test".to_string(),
            description: None,
            strategy: strategy.to_string(),
            priority: 0,
            match_model: None,
            match_tags: vec![].into(),
            conditions: serde_json::json!({}),
            targets: serde_json::to_value(targets).unwrap(),
            timeout_ms: 30000,
            retries: 1,
            status: "active".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        }
    }

    #[test]
    fn test_single_strategy() {
        let targets = vec![Target {
            provider_config_id: Uuid::new_v4(),
            model_id: "gpt-4o".to_string(),
            provider_kind: Some("openai".to_string()),
            weight: None,
        }];
        let rule = make_rule("single", targets);
        let decision = build_decision(&rule);
        assert_eq!(decision.primary.model_id, "gpt-4o");
        assert!(decision.fallback_chain.is_empty());
    }

    #[test]
    fn test_fallback_strategy() {
        let targets = vec![
            Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "gpt-4o".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "claude-3-5-sonnet".to_string(),
                provider_kind: Some("anthropic".to_string()),
                weight: None,
            },
        ];
        let rule = make_rule("fallback", targets);
        let decision = build_decision(&rule);
        assert_eq!(decision.primary.model_id, "gpt-4o");
        assert_eq!(decision.fallback_chain.len(), 1);
        assert_eq!(decision.fallback_chain[0].model_id, "claude-3-5-sonnet");
    }

    #[test]
    fn test_weighted_strategy_distribution() {
        let targets = vec![
            Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "A".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: Some(70),
            },
            Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "B".to_string(),
                provider_kind: Some("anthropic".to_string()),
                weight: Some(30),
            },
        ];
        let rule = make_rule("weighted", targets);

        let mut a_count = 0;
        let mut b_count = 0;
        for _ in 0..1000 {
            let decision = build_decision(&rule);
            match decision.primary.model_id.as_str() {
                "A" => a_count += 1,
                "B" => b_count += 1,
                _ => {}
            }
        }

        // Statistical test — should be roughly 70/30
        assert!(a_count > 600, "A should be selected ~70% of time, got {}", a_count);
        assert!(b_count > 200, "B should be selected ~30% of time, got {}", b_count);
    }
}
