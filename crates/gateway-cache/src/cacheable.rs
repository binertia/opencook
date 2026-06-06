//! Cacheability rules — determine if a request should be cached.

use gateway_core::types::ChatCompletionRequest;

/// Check whether a request is cacheable.
///
/// Rules:
/// 1. `temperature` must be 0.0 (or absent, which defaults to 1.0 → NOT cacheable).
/// 2. `stream` must be `false` (or absent).
/// 3. No dynamic content patterns in messages (timestamps, UUIDs, etc.).
/// 4. Not explicitly skipped via `X-Cache-No-Store` header.
pub fn is_cacheable(request: &ChatCompletionRequest, skip_header: bool) -> bool {
    if skip_header {
        return false;
    }

    // Rule 1: temperature must be exactly 0.0
    let temperature = request.temperature.unwrap_or(1.0);
    if temperature != 0.0 {
        return false;
    }

    // Rule 2: no streaming
    if request.stream == Some(true) {
        return false;
    }

    // Rule 3: no dynamic content patterns in messages
    for msg in &request.messages {
        if let Some(content) = &msg.content {
            if contains_dynamic_content(content) {
                return false;
            }
        }
    }

    true
}

/// Detect dynamic content patterns that would make caching unsafe.
fn contains_dynamic_content(text: &str) -> bool {
    // ISO 8601 timestamps (e.g. 2024-01-15T10:30:00Z)
    if text.contains("T00:")
        || text.contains("T01:")
        || text.contains("T02:")
        || text.contains("T03:")
        || text.contains("T04:")
        || text.contains("T05:")
        || text.contains("T06:")
        || text.contains("T07:")
        || text.contains("T08:")
        || text.contains("T09:")
        || text.contains("T10:")
        || text.contains("T11:")
        || text.contains("T12:")
        || text.contains("T13:")
        || text.contains("T14:")
        || text.contains("T15:")
        || text.contains("T16:")
        || text.contains("T17:")
        || text.contains("T18:")
        || text.contains("T19:")
        || text.contains("T20:")
        || text.contains("T21:")
        || text.contains("T22:")
        || text.contains("T23:")
    {
        return true;
    }

    // UUID-like patterns (8-4-4-4-12 hex)
    if text.len() >= 36 {
        let lower = text.to_lowercase();
        // Simple heuristic: look for hyphen-separated hex groups
        for window in lower.as_bytes().windows(36) {
            let s = std::str::from_utf8(window).unwrap_or("");
            if looks_like_uuid(s) {
                return true;
            }
        }
    }

    // Template variables like {current_time}, {user_id}, etc.
    if text.contains("{") && text.contains("}") {
        let lower = text.to_lowercase();
        if lower.contains("{current")
            || lower.contains("{user")
            || lower.contains("{time")
            || lower.contains("{date")
            || lower.contains("{now}")
            || lower.contains("{uuid}")
            || lower.contains("{id}")
        {
            return true;
        }
    }

    false
}

fn looks_like_uuid(s: &str) -> bool {
    // 8-4-4-4-12 format with hex chars
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lengths = [8, 4, 4, 4, 12];
    for (i, &expected) in lengths.iter().enumerate() {
        if parts[i].len() != expected {
            return false;
        }
        if !parts[i].chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::types::{ChatCompletionRequest, Message, MessageRole};

    fn make_request(
        temp: Option<f32>,
        stream: Option<bool>,
        content: &str,
    ) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: Some(content.to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            frequency_penalty: None,
            max_tokens: None,
            n: None,
            presence_penalty: None,
            response_format: None,
            seed: None,
            stop: None,
            stream,
            temperature: temp,
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        }
    }

    #[test]
    fn test_temp_zero_cacheable() {
        let req = make_request(Some(0.0), None, "Hello");
        assert!(is_cacheable(&req, false));
    }

    #[test]
    fn test_temp_nonzero_not_cacheable() {
        let req = make_request(Some(0.7), None, "Hello");
        assert!(!is_cacheable(&req, false));
    }

    #[test]
    fn test_streaming_not_cacheable() {
        let req = make_request(Some(0.0), Some(true), "Hello");
        assert!(!is_cacheable(&req, false));
    }

    #[test]
    fn test_skip_header_not_cacheable() {
        let req = make_request(Some(0.0), None, "Hello");
        assert!(!is_cacheable(&req, true));
    }

    #[test]
    fn test_timestamp_not_cacheable() {
        let req = make_request(Some(0.0), None, "The time is 2024-01-15T10:30:00Z");
        assert!(!is_cacheable(&req, false));
    }

    #[test]
    fn test_uuid_not_cacheable() {
        let req = make_request(
            Some(0.0),
            None,
            "User ID: 550e8400-e29b-41d4-a716-446655440000",
        );
        assert!(!is_cacheable(&req, false));
    }

    #[test]
    fn test_template_var_not_cacheable() {
        let req = make_request(Some(0.0), None, "Hello {current_user}");
        assert!(!is_cacheable(&req, false));
    }
}
