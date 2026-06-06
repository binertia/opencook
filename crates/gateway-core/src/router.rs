//! Routing engine — evaluates routing rules and selects the best provider.

use gateway_db::RoutingRule;
use tracing::{debug, warn};

use crate::strategies::{build_decision, RoutingDecision};
use crate::types::ChatCompletionRequest;

/// Evaluate routing rules against a request and return a routing decision.
///
/// Rules are checked in priority order (lower number = higher priority).
/// The first matching rule wins.
///
/// Returns `None` if no rules match — caller should use default behavior.
pub fn evaluate_rules(
    request: &ChatCompletionRequest,
    rules: &[RoutingRule],
) -> Option<RoutingDecision> {
    // Defensive sort — DB should return in order, but we ensure it here
    let mut sorted: Vec<_> = rules.iter().collect();
    sorted.sort_by_key(|r| r.priority);

    for rule in sorted {
        if rule_matches(request, rule) {
            debug!(
                rule_id = %rule.id,
                rule_name = %rule.name,
                strategy = %rule.strategy,
                "Routing rule matched"
            );
            return Some(build_decision(rule));
        }
    }

    debug!(model = %request.model, "No routing rules matched");
    None
}

/// Check if a single rule matches a request.
fn rule_matches(request: &ChatCompletionRequest, rule: &RoutingRule) -> bool {
    // Check model match (NULL means wildcard)
    if let Some(match_model) = &rule.match_model {
        if *match_model != request.model {
            return false;
        }
    }

    // Check conditions JSONB
    if let Some(conditions) = rule.conditions.as_object() {
        // require_streaming
        if let Some(req_stream) = conditions
            .get("require_streaming")
            .and_then(|v| v.as_bool())
        {
            let is_streaming = request.stream == Some(true);
            if req_stream && !is_streaming {
                return false;
            }
        }

        // require_tools
        if let Some(req_tools) = conditions.get("require_tools").and_then(|v| v.as_bool()) {
            let has_tools = request.tools.is_some();
            if req_tools && !has_tools {
                return false;
            }
        }

        // min_context_length
        if let Some(min_ctx) = conditions
            .get("min_context_length")
            .and_then(|v| v.as_i64())
        {
            let total_chars: usize = request
                .messages
                .iter()
                .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0))
                .sum();
            if (total_chars as i64) < min_ctx {
                return false;
            }
        }

        // max_temperature
        if let Some(max_temp) = conditions.get("max_temperature").and_then(|v| v.as_f64()) {
            let temp = request.temperature.unwrap_or(1.0) as f64;
            if temp > max_temp {
                return false;
            }
        }
    }

    true
}

/// Resolve a model ID to a provider config using routing rules.
/// If no rules match, returns the original model as the default.
pub fn resolve_with_fallback(
    request: &ChatCompletionRequest,
    rules: &[RoutingRule],
) -> RoutingDecision {
    match evaluate_rules(request, rules) {
        Some(decision) => decision,
        None => {
            // Default: route directly to the model name as provider hint
            // In practice, the caller will look up the provider by model name
            warn!(
                model = %request.model,
                "No routing rule matched, using direct model routing"
            );
            RoutingDecision {
                rule_id: None,
                primary: gateway_db::Target {
                    provider_config_id: uuid::Uuid::nil(),
                    model_id: request.model.clone(),
                    provider_kind: None,
                    weight: None,
                },
                fallback_chain: vec![],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_db::Target;
    use uuid::Uuid;

    fn make_request(model: &str, stream: Option<bool>) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![],
            frequency_penalty: None,
            max_tokens: None,
            n: None,
            presence_penalty: None,
            response_format: None,
            seed: None,
            stop: None,
            stream,
            temperature: None,
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        }
    }

    fn make_rule(
        priority: i32,
        match_model: Option<&str>,
        strategy: &str,
        targets: Vec<Target>,
    ) -> RoutingRule {
        RoutingRule {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "test-rule".to_string(),
            description: None,
            strategy: strategy.to_string(),
            priority,
            match_model: match_model.map(|s| s.to_string()),
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
    fn test_model_match() {
        let req = make_request("gpt-4o", None);
        let rules = vec![make_rule(
            0,
            Some("gpt-4o"),
            "single",
            vec![Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "gpt-4o".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            }],
        )];

        let decision = evaluate_rules(&req, &rules);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().primary.model_id, "gpt-4o");
    }

    #[test]
    fn test_wildcard_match() {
        let req = make_request("gpt-4o", None);
        let rules = vec![make_rule(
            0,
            None, // wildcard
            "single",
            vec![Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "fallback-model".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            }],
        )];

        let decision = evaluate_rules(&req, &rules);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().primary.model_id, "fallback-model");
    }

    #[test]
    fn test_no_match() {
        let req = make_request("gpt-4o", None);
        let rules = vec![make_rule(
            0,
            Some("claude-3"),
            "single",
            vec![Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "claude".to_string(),
                provider_kind: Some("anthropic".to_string()),
                weight: None,
            }],
        )];

        let decision = evaluate_rules(&req, &rules);
        assert!(decision.is_none());
    }

    #[test]
    fn test_priority_order() {
        let req = make_request("gpt-4o", None);
        let rules = vec![
            make_rule(
                1,
                Some("gpt-4o"),
                "single",
                vec![Target {
                    provider_config_id: Uuid::new_v4(),
                    model_id: "second".to_string(),
                    provider_kind: Some("openai".to_string()),
                    weight: None,
                }],
            ),
            make_rule(
                0,
                Some("gpt-4o"),
                "single",
                vec![Target {
                    provider_config_id: Uuid::new_v4(),
                    model_id: "first".to_string(),
                    provider_kind: Some("openai".to_string()),
                    weight: None,
                }],
            ),
        ];

        let decision = evaluate_rules(&req, &rules);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().primary.model_id, "first");
    }

    #[test]
    fn test_condition_require_streaming() {
        let req_stream = make_request("gpt-4o", Some(true));
        let req_no_stream = make_request("gpt-4o", Some(false));

        let mut rule = make_rule(
            0,
            Some("gpt-4o"),
            "single",
            vec![Target {
                provider_config_id: Uuid::new_v4(),
                model_id: "stream-model".to_string(),
                provider_kind: Some("openai".to_string()),
                weight: None,
            }],
        );
        rule.conditions = serde_json::json!({"require_streaming": true});

        assert!(evaluate_rules(&req_stream, &[rule.clone()]).is_some());
        assert!(evaluate_rules(&req_no_stream, &[rule]).is_none());
    }
}
