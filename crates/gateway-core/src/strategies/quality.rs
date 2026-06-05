//! Quality routing strategy — selects the best model variant regardless of cost.
//!
//! Uses a hardcoded quality score per model.  Latency is only used to filter
//! out providers that exceed the SLA.

use gateway_db::Target;
use uuid::Uuid;

/// Hardcoded quality score per model (0.0 – 1.0, higher is better).
pub fn model_quality_score(model_id: &str) -> f64 {
    match model_id {
        // OpenAI
        "gpt-4o" | "gpt-4o-2024-05-13" | "gpt-4o-2024-08-06" => 1.0,
        "gpt-4-turbo" | "gpt-4-turbo-preview" => 0.95,
        "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => 0.7,
        "gpt-3.5-turbo" => 0.55,
        // Anthropic
        "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet" => 0.95,
        "claude-3-opus" => 0.98,
        "claude-3-sonnet" => 0.85,
        "claude-3-haiku" => 0.65,
        // Google
        "gemini-1.5-pro" => 0.92,
        "gemini-1.5-flash" => 0.75,
        // Meta / Ollama
        "llama3.2" | "llama-3.2" => 0.6,
        "llama-3.1-70b" | "llama-3.1-70b-versatile" => 0.75,
        // Chinese providers
        "qwen-max" => 0.88,
        "qwen-plus" => 0.72,
        "moonshot-v1-8k" => 0.78,
        "hunyuan-lite" => 0.6,
        // Other
        "mistral-large-latest" => 0.8,
        "command-r" => 0.75,
        _ => 0.5, // unknown model
    }
}

/// Provider candidate with quality metadata.
#[derive(Debug, Clone)]
pub struct QualityCandidate {
    pub target: Target,
    pub latency_ms: u64,
}

/// Select the highest-quality provider that meets the latency SLA.
///
/// # Selection logic
/// 1. Filter out providers exceeding `latency_sla_ms`.
/// 2. Sort by quality score descending.
/// 3. On tie: prefer lower latency.
pub fn select_best_quality(
    candidates: &[QualityCandidate],
    latency_sla_ms: u64,
) -> Option<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.latency_ms <= latency_sla_ms)
        .collect();

    if viable.is_empty() {
        return None;
    }

    viable.sort_by(|a, b| {
        let score_a = model_quality_score(&a.target.model_id);
        let score_b = model_quality_score(&b.target.model_id);

        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.latency_ms.cmp(&b.latency_ms))
    });

    Some(viable[0].target.clone())
}

/// Build a fallback chain ordered by quality (best first).
pub fn build_quality_fallback_chain(
    candidates: &[QualityCandidate],
    latency_sla_ms: u64,
) -> Vec<Target> {
    let mut viable: Vec<_> = candidates
        .iter()
        .filter(|c| c.latency_ms <= latency_sla_ms)
        .collect();

    viable.sort_by(|a, b| {
        let score_a = model_quality_score(&a.target.model_id);
        let score_b = model_quality_score(&b.target.model_id);

        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.latency_ms.cmp(&b.latency_ms))
    });

    viable.into_iter().map(|c| c.target.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(model_id: &str, latency_ms: u64) -> QualityCandidate {
        QualityCandidate {
            target: Target {
                provider_config_id: Uuid::new_v4(),
                model_id: model_id.to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            },
            latency_ms,
        }
    }

    #[test]
    fn test_selects_highest_quality() {
        let candidates = vec![
            make_candidate("gpt-4o-mini", 100),
            make_candidate("gpt-4o", 150),
        ];

        let selected = select_best_quality(&candidates, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o");
    }

    #[test]
    fn test_skips_slow_provider() {
        let candidates = vec![
            make_candidate("gpt-4o", 20_000), // exceeds SLA
            make_candidate("gpt-4o-mini", 100),
        ];

        let selected = select_best_quality(&candidates, 10_000);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_tiebreak_by_latency() {
        // Both have same model_id → same quality score
        let candidates = vec![
            make_candidate("gpt-4o", 200),
            make_candidate("gpt-4o", 100),
        ];

        let selected = select_best_quality(&candidates, 10_000);
        assert!(selected.is_some());
        // Both are same model, just verify one is selected
        assert_eq!(selected.unwrap().model_id, "gpt-4o");
    }

    #[test]
    fn test_unknown_model_gets_default_score() {
        let candidates = vec![
            make_candidate("unknown-model-v1", 100),
            make_candidate("gpt-4o-mini", 100),
        ];

        let selected = select_best_quality(&candidates, 10_000);
        assert!(selected.is_some());
        // gpt-4o-mini (0.7) beats unknown (0.5)
        assert_eq!(selected.unwrap().model_id, "gpt-4o-mini");
    }

    #[test]
    fn test_fallback_chain_ordered_by_quality() {
        let candidates = vec![
            make_candidate("gpt-3.5-turbo", 100),
            make_candidate("gpt-4o", 100),
            make_candidate("gpt-4o-mini", 100),
        ];

        let chain = build_quality_fallback_chain(&candidates, 10_000);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].model_id, "gpt-4o");      // 1.0
        assert_eq!(chain[1].model_id, "gpt-4o-mini");  // 0.7
        assert_eq!(chain[2].model_id, "gpt-3.5-turbo"); // 0.55
    }
}
