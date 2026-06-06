//! E2E tests for latency-based routing.

use gateway_core::latency_tracker::LatencyStats;

const MIN_SAMPLES_FOR_ROUTING: usize = 10;
use gateway_core::strategies::latency::{
    build_latency_fallback_chain, select_lowest_latency, LatencyCandidate,
};
use gateway_db::Target;
use uuid::Uuid;

#[test]
fn test_selects_lowest_latency_provider() {
    let candidates = vec![
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "fast".to_string(),
                provider_kind: Some("a".to_string()),
                weight: None,
            },
            stats: LatencyStats {
                sample_count: MIN_SAMPLES_FOR_ROUTING + 5,
                p50_ms: 100,
                p90_ms: 200,
                p99_ms: 300,
                ema_ms: 105,
            },
        },
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "slow".to_string(),
                provider_kind: Some("b".to_string()),
                weight: None,
            },
            stats: LatencyStats {
                sample_count: MIN_SAMPLES_FOR_ROUTING + 5,
                p50_ms: 500,
                p90_ms: 700,
                p99_ms: 900,
                ema_ms: 510,
            },
        },
    ];

    let selected = select_lowest_latency(&candidates, 10_000);
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().model_id, "fast");
}

#[test]
fn test_insufficient_samples_returns_none() {
    let candidates = vec![LatencyCandidate {
        target: Target {
            provider_config_id: Uuid::new_v4(),
            model_id: "A".to_string(),
            provider_kind: None,
            weight: None,
        },
        stats: LatencyStats {
            sample_count: 3,
            p50_ms: 100,
            p90_ms: 200,
            p99_ms: 300,
            ema_ms: 100,
        },
    }];

    let selected = select_lowest_latency(&candidates, 10_000);
    assert!(selected.is_none());
}

#[test]
fn test_sla_penalty_changes_selection() {
    let _candidates = [
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "within-sla".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 450,
                p90_ms: 600,
                p99_ms: 800,
                ema_ms: 460,
            },
        },
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "outside-sla".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 300,
                p90_ms: 400,
                p99_ms: 500,
                ema_ms: 310,
            },
        },
    ];

    // SLA = 350
    // within-sla: p50=300, score=300
    // outside-sla: p50=400, score=400*1.5=600
    let candidates2 = vec![
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "within".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 300,
                p90_ms: 400,
                p99_ms: 500,
                ema_ms: 300,
            },
        },
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "outside".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 400,
                p90_ms: 500,
                p99_ms: 600,
                ema_ms: 400,
            },
        },
    ];

    let selected = select_lowest_latency(&candidates2, 350);
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().model_id, "within");
}

#[test]
fn test_fallback_chain_ordered() {
    let candidates = vec![
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "mid".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 300,
                p90_ms: 400,
                p99_ms: 500,
                ema_ms: 300,
            },
        },
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "fast".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 100,
                p90_ms: 200,
                p99_ms: 300,
                ema_ms: 100,
            },
        },
        LatencyCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "slow".to_string(),
                provider_kind: None,
                weight: None,
            },
            stats: LatencyStats {
                sample_count: 20,
                p50_ms: 500,
                p90_ms: 600,
                p99_ms: 700,
                ema_ms: 500,
            },
        },
    ];

    let chain = build_latency_fallback_chain(&candidates, 10_000);
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].model_id, "fast");
    assert_eq!(chain[1].model_id, "mid");
    assert_eq!(chain[2].model_id, "slow");
}

#[test]
fn test_latency_stats_has_enough_samples() {
    let stats = LatencyStats {
        sample_count: MIN_SAMPLES_FOR_ROUTING,
        p50_ms: 100,
        p90_ms: 200,
        p99_ms: 300,
        ema_ms: 100,
    };
    assert!(stats.has_enough_samples());

    let stats2 = LatencyStats {
        sample_count: MIN_SAMPLES_FOR_ROUTING - 1,
        p50_ms: 100,
        p90_ms: 200,
        p99_ms: 300,
        ema_ms: 100,
    };
    assert!(!stats2.has_enough_samples());
}
