//! Structured request logging for LLM API requests.
//!
//! Provides helpers to emit consistently-structured log events with all
//! fields required for debugging, monitoring, and cost tracking.

use serde::Serialize;

/// Structured fields for a single LLM request log entry.
#[derive(Debug, Serialize, Clone)]
pub struct RequestLogEntry {
    pub trace_id: String,
    pub org_id: String,
    pub api_key_id: Option<String>,
    pub model_requested: String,
    pub model_routed: Option<String>,
    pub provider_name: Option<String>,
    pub status: String,
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub latency_ms: u64,
    pub latency_gateway_ms: u64,
    pub latency_provider_ms: u64,
    pub latency_total_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cache_hit: bool,
    pub routing_rule_id: Option<String>,
    pub cost_usd: Option<f64>,
}

impl RequestLogEntry {
    /// Create a new request log entry with minimal required fields.
    pub fn new(
        trace_id: impl Into<String>,
        org_id: impl Into<String>,
        model_requested: impl Into<String>,
    ) -> Self {
        Self {
            trace_id: trace_id.into(),
            org_id: org_id.into(),
            api_key_id: None,
            model_requested: model_requested.into(),
            model_routed: None,
            provider_name: None,
            status: "pending".to_string(),
            status_code: None,
            error_code: None,
            latency_ms: 0,
            latency_gateway_ms: 0,
            latency_provider_ms: 0,
            latency_total_ms: 0,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cache_hit: false,
            routing_rule_id: None,
            cost_usd: None,
        }
    }

    /// Set the API key ID.
    pub fn with_api_key_id(mut self, id: impl Into<String>) -> Self {
        self.api_key_id = Some(id.into());
        self
    }

    /// Set the routed model (may differ from requested).
    pub fn with_model_routed(mut self, model: impl Into<String>) -> Self {
        self.model_routed = Some(model.into());
        self
    }

    /// Set the provider name.
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider_name = Some(provider.into());
        self
    }

    /// Mark the request as successful.
    pub fn with_success(mut self, status_code: u16) -> Self {
        self.status = "success".to_string();
        self.status_code = Some(status_code);
        self
    }

    /// Mark the request as failed with an error code.
    pub fn with_error(mut self, status_code: u16, error_code: impl Into<String>) -> Self {
        self.status = "error".to_string();
        self.status_code = Some(status_code);
        self.error_code = Some(error_code.into());
        self
    }

    /// Mark the request as quota exceeded.
    pub fn with_quota_exceeded(mut self) -> Self {
        self.status = "quota_exceeded".to_string();
        self.status_code = Some(403);
        self.error_code = Some("quota_exceeded".to_string());
        self
    }

    /// Mark the request as rate limited.
    pub fn with_rate_limited(mut self) -> Self {
        self.status = "rate_limited".to_string();
        self.status_code = Some(429);
        self.error_code = Some("rate_limited".to_string());
        self
    }

    /// Mark the request as cancelled.
    pub fn with_cancelled(mut self) -> Self {
        self.status = "cancelled".to_string();
        self.status_code = Some(499);
        self.error_code = Some("request_cancelled".to_string());
        self
    }

    /// Set latency in milliseconds.
    pub fn with_latency_ms(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }

    /// Set latency breakdown: gateway, provider, and total milliseconds.
    pub fn with_latency_breakdown(
        mut self,
        gateway_ms: u64,
        provider_ms: u64,
        total_ms: u64,
    ) -> Self {
        self.latency_gateway_ms = gateway_ms;
        self.latency_provider_ms = provider_ms;
        self.latency_total_ms = total_ms;
        // Also keep the legacy field in sync for backwards compatibility.
        self.latency_ms = total_ms;
        self
    }

    /// Set token usage.
    pub fn with_tokens(mut self, prompt: u64, completion: u64, total: u64) -> Self {
        self.prompt_tokens = Some(prompt);
        self.completion_tokens = Some(completion);
        self.total_tokens = Some(total);
        self
    }

    /// Mark as cache hit.
    pub fn with_cache_hit(mut self) -> Self {
        self.cache_hit = true;
        self
    }

    /// Set routing rule ID.
    pub fn with_routing_rule(mut self, rule_id: impl Into<String>) -> Self {
        self.routing_rule_id = Some(rule_id.into());
        self
    }

    /// Set estimated cost in USD.
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }
}

/// Emit an INFO-level structured log for a completed request.
///
/// ```rust,ignore
/// use gateway_observability::request_log::{RequestLogEntry, log_request};
///
/// log_request(
///     RequestLogEntry::new("trace-123", "org-456", "gpt-4o")
///         .with_success(200)
///         .with_latency_ms(420)
///         .with_tokens(150, 300, 450)
///         .with_provider("openai")
///         .with_cost(0.0123),
/// );
/// ```
pub fn log_request(entry: &RequestLogEntry) {
    let json = match serde_json::to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize request log entry");
            return;
        }
    };

    match entry.status.as_str() {
        "success" => {
            tracing::info!(
                trace_id = %entry.trace_id,
                org_id = %entry.org_id,
                model = %entry.model_requested,
                provider = entry.provider_name.as_deref().unwrap_or("none"),
                latency_ms = entry.latency_ms,
                status = %entry.status,
                status_code = entry.status_code.unwrap_or(0),
                prompt_tokens = entry.prompt_tokens.unwrap_or(0),
                completion_tokens = entry.completion_tokens.unwrap_or(0),
                total_tokens = entry.total_tokens.unwrap_or(0),
                cache_hit = entry.cache_hit,
                cost_usd = entry.cost_usd.unwrap_or(0.0),
                request = %json,
                "LLM request completed",
            );
        }
        "error" | "quota_exceeded" | "rate_limited" => {
            tracing::warn!(
                trace_id = %entry.trace_id,
                org_id = %entry.org_id,
                model = %entry.model_requested,
                provider = entry.provider_name.as_deref().unwrap_or("none"),
                latency_ms = entry.latency_ms,
                status = %entry.status,
                status_code = entry.status_code.unwrap_or(0),
                error_code = entry.error_code.as_deref().unwrap_or("unknown"),
                request = %json,
                "LLM request failed",
            );
        }
        "cancelled" => {
            tracing::warn!(
                trace_id = %entry.trace_id,
                org_id = %entry.org_id,
                model = %entry.model_requested,
                latency_ms = entry.latency_ms,
                status = %entry.status,
                request = %json,
                "LLM request cancelled",
            );
        }
        _ => {
            tracing::debug!(
                trace_id = %entry.trace_id,
                org_id = %entry.org_id,
                request = %json,
                "LLM request event",
            );
        }
    }
}

/// Emit a DEBUG-level log with a truncated request/response body preview.
///
/// Use this for non-production debugging. The body is truncated to `max_len`
/// characters and PII-redacted before logging.
pub fn log_body_preview(direction: &str, trace_id: &str, body: &str, max_len: usize) {
    let preview = if body.len() > max_len {
        format!("{}...", &body[..max_len])
    } else {
        body.to_string()
    };
    let preview = crate::redaction::redact(&preview);
    tracing::debug!(
        trace_id = %trace_id,
        direction = %direction,
        body_preview = %preview,
        "Body preview",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_log_entry_builder() {
        let entry = RequestLogEntry::new("trace-1", "org-1", "gpt-4o")
            .with_success(200)
            .with_latency_ms(420)
            .with_tokens(10, 20, 30)
            .with_provider("openai")
            .with_cost(0.001)
            .with_cache_hit();

        assert_eq!(entry.trace_id, "trace-1");
        assert_eq!(entry.status, "success");
        assert_eq!(entry.status_code, Some(200));
        assert_eq!(entry.latency_ms, 420);
        assert_eq!(entry.prompt_tokens, Some(10));
        assert!(entry.cache_hit);
        assert_eq!(entry.cost_usd, Some(0.001));
    }

    #[test]
    fn test_request_log_entry_error() {
        let entry = RequestLogEntry::new("trace-2", "org-2", "claude-3")
            .with_error(502, "provider_error")
            .with_latency_ms(1200);

        assert_eq!(entry.status, "error");
        assert_eq!(entry.status_code, Some(502));
        assert_eq!(entry.error_code, Some("provider_error".to_string()));
    }

    #[test]
    fn test_request_log_serialization() {
        let entry = RequestLogEntry::new("t", "o", "m")
            .with_success(200)
            .with_latency_ms(100)
            .with_tokens(5, 5, 10);

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"trace_id\":\"t\""));
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"latency_ms\":100"));
    }
}
