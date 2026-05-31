//! Cache types.

use serde::{Deserialize, Serialize};

/// A deterministic cache key for a chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Tenant-scoped Redis key: `cache:{org_id}:{model}:{hash}`
    pub redis_key: String,
    /// Raw SHA-256 hash (hex) of the normalized request.
    pub request_hash: String,
    /// Model name included in key for invalidation granularity.
    pub model: String,
    /// Organization ID for tenant isolation.
    pub org_id: uuid::Uuid,
}

/// A cached response ready to return to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The JSON response body (OpenAI-compatible).
    pub body: String,
    /// Provider name that originally generated the response.
    pub provider: String,
    /// Token usage from the original response.
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// When the entry was cached.
    pub cached_at: chrono::DateTime<chrono::Utc>,
}
