//! Cache key construction — deterministic SHA-256 hash of normalized request.

use gateway_core::types::ChatCompletionRequest;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::types::CacheKey;

/// Build a deterministic cache key for a chat completion request.
///
/// The key includes:
/// - `org_id` prefix for tenant isolation
/// - `model` name for invalidation granularity
/// - SHA-256 of the *normalized* request fields
pub fn build_cache_key(request: &ChatCompletionRequest, org_id: Uuid) -> CacheKey {
    let normalized = normalize_request(request);
    let canonical = serde_json::to_string(&normalized).unwrap_or_default();
    let hash = hex::encode(Sha256::digest(canonical.as_bytes()));

    let redis_key = format!("cache:{org_id}:{}:{hash}", request.model);

    CacheKey {
        redis_key,
        request_hash: hash,
        model: request.model.clone(),
        org_id,
    }
}

/// Normalize a request into a deterministic JSON value.
///
/// Only fields that affect the response are included.
/// Optional fields are omitted when absent to keep keys stable.
fn normalize_request(request: &ChatCompletionRequest) -> Value {
    let mut map = Map::new();

    // Always include model
    map.insert("model".to_string(), Value::String(request.model.clone()));

    // Messages: canonical JSON with sorted object keys
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|m| {
            let mut msg = Map::new();
            msg.insert(
                "role".to_string(),
                serde_json::to_value(&m.role).unwrap_or_default(),
            );
            if let Some(content) = &m.content {
                msg.insert("content".to_string(), Value::String(content.clone()));
            }
            if let Some(name) = &m.name {
                msg.insert("name".to_string(), Value::String(name.clone()));
            }
            Value::Object(msg)
        })
        .collect();
    map.insert("messages".to_string(), Value::Array(messages));

    // Optional fields — only include when present
    if let Some(v) = request.frequency_penalty {
        map.insert("frequency_penalty".to_string(), json_f32(v));
    }
    if let Some(v) = request.max_tokens {
        map.insert("max_tokens".to_string(), Value::Number(v.into()));
    }
    if let Some(v) = request.n {
        if v != 1 {
            map.insert("n".to_string(), Value::Number(v.into()));
        }
    }
    if let Some(v) = request.presence_penalty {
        map.insert("presence_penalty".to_string(), json_f32(v));
    }
    if let Some(v) = &request.response_format {
        map.insert(
            "response_format".to_string(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = request.seed {
        map.insert("seed".to_string(), Value::Number(v.into()));
    }
    if let Some(v) = &request.stop {
        map.insert(
            "stop".to_string(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = request.temperature {
        map.insert("temperature".to_string(), json_f32(v));
    }
    if let Some(v) = request.top_p {
        map.insert("top_p".to_string(), json_f32(v));
    }
    if let Some(v) = &request.tools {
        map.insert(
            "tools".to_string(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = &request.tool_choice {
        map.insert(
            "tool_choice".to_string(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = &request.user {
        map.insert("user".to_string(), Value::String(v.clone()));
    }

    Value::Object(map)
}

/// Convert f32 to JSON number with stable formatting (no trailing zeros).
fn json_f32(v: f32) -> Value {
    // Use serde_json's default formatting which handles this well
    serde_json::to_value(v).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_core::types::{Message, MessageRole};

    fn dummy_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: Some("You are helpful.".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: Some("Hello".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            frequency_penalty: None,
            max_tokens: Some(100),
            n: None,
            presence_penalty: None,
            response_format: None,
            seed: Some(42),
            stop: None,
            stream: None,
            temperature: Some(0.0),
            top_p: None,
            tools: None,
            tool_choice: None,
            user: None,
        }
    }

    #[test]
    fn test_identical_requests_same_key() {
        let req = dummy_request();
        let org = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let key1 = build_cache_key(&req, org);
        let key2 = build_cache_key(&req, org);
        assert_eq!(key1.request_hash, key2.request_hash);
        assert_eq!(key1.redis_key, key2.redis_key);
    }

    #[test]
    fn test_different_requests_different_key() {
        let req1 = dummy_request();
        let mut req2 = dummy_request();
        req2.messages[1].content = Some("Goodbye".to_string());
        let org = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let key1 = build_cache_key(&req1, org);
        let key2 = build_cache_key(&req2, org);
        assert_ne!(key1.request_hash, key2.request_hash);
    }

    #[test]
    fn test_org_id_prefix() {
        let req = dummy_request();
        let org = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let key = build_cache_key(&req, org);
        assert!(key.redis_key.starts_with(&format!("cache:{org}:")));
    }

    #[test]
    fn test_model_in_key() {
        let req = dummy_request();
        let org = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let key = build_cache_key(&req, org);
        assert!(key.redis_key.contains("gpt-4o"));
    }
}
