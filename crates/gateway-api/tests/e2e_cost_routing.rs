//! E2E tests for cost-optimized routing and pricing management.

mod helpers;

use gateway_core::strategies::cost_optimized::{
    build_cost_fallback_chain, estimate_cost, select_cheapest, HealthStatus, ProviderCandidate,
};
use gateway_db::Target;
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn test_cost_strategy_selects_cheapest() {
    let candidates = vec![
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "gpt-4o".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            input_cost_per_1k: Decimal::from(5),
            output_cost_per_1k: Decimal::from(15),
            health: HealthStatus::Healthy,
            latency_ms: 100,
        },
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "gpt-4o-mini".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            input_cost_per_1k: Decimal::from(15) / Decimal::from(100),
            output_cost_per_1k: Decimal::from(60) / Decimal::from(100),
            health: HealthStatus::Healthy,
            latency_ms: 150,
        },
    ];

    let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
}

#[test]
fn test_cost_strategy_skips_unhealthy() {
    let candidates = vec![
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "cheap".to_string(),
                provider_kind: Some("x".to_string()),
                weight: None,
            },
            input_cost_per_1k: Decimal::from(1),
            output_cost_per_1k: Decimal::from(1),
            health: HealthStatus::Unhealthy,
            latency_ms: 100,
        },
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "expensive".to_string(),
                provider_kind: Some("y".to_string()),
                weight: None,
            },
            input_cost_per_1k: Decimal::from(10),
            output_cost_per_1k: Decimal::from(10),
            health: HealthStatus::Healthy,
            latency_ms: 100,
        },
    ];

    let selected = select_cheapest(&candidates, 1000, 1000, 10_000);
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().model_id, "expensive");
}

#[test]
fn test_estimate_cost_calculation() {
    let candidate = ProviderCandidate {
        target: Target {
            provider_config_id: Uuid::new_v4(),
            model_id: "test".to_string(),
            provider_kind: None,
            weight: None,
        },
        input_cost_per_1k: Decimal::from(5),
        output_cost_per_1k: Decimal::from(15),
        health: HealthStatus::Healthy,
        latency_ms: 0,
    };

    let cost = estimate_cost(&candidate, 2000, 1000);
    // input: 2000 * 5 / 1000 = 10
    // output: 1000 * 15 / 1000 = 15
    assert_eq!(cost, Decimal::from(25));
}

#[test]
fn test_fallback_chain_ordering() {
    let candidates = vec![
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "A".to_string(),
                provider_kind: None,
                weight: None,
            },
            input_cost_per_1k: Decimal::from(1),
            output_cost_per_1k: Decimal::from(1),
            health: HealthStatus::Healthy,
            latency_ms: 100,
        },
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "B".to_string(),
                provider_kind: None,
                weight: None,
            },
            input_cost_per_1k: Decimal::from(5) / Decimal::from(10),
            output_cost_per_1k: Decimal::from(5) / Decimal::from(10),
            health: HealthStatus::Healthy,
            latency_ms: 100,
        },
        ProviderCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "C".to_string(),
                provider_kind: None,
                weight: None,
            },
            input_cost_per_1k: Decimal::from(2),
            output_cost_per_1k: Decimal::from(2),
            health: HealthStatus::Healthy,
            latency_ms: 100,
        },
    ];

    let chain = build_cost_fallback_chain(&candidates, 1000, 1000, 10_000);
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].model_id, "B");
    assert_eq!(chain[1].model_id, "A");
    assert_eq!(chain[2].model_id, "C");
}
