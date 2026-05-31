//! Rate limiting and quota types.

use serde::{Deserialize, Serialize};

/// Result of a rate limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitResult {
    /// Request is allowed. Includes remaining quota and reset timestamp.
    Allowed {
        /// Remaining requests/tokens in the current window.
        remaining: u64,
        /// Unix timestamp (seconds) when the window resets.
        reset_at: u64,
        /// The limit that was checked against.
        limit: u64,
    },
    /// Request exceeds the rate limit.
    Exceeded {
        /// Seconds until the client should retry.
        retry_after: u64,
        /// The limit that was exceeded.
        limit: u64,
    },
}

/// Configuration for a single rate limit window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Maximum number of requests/tokens in the window.
    pub limit: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
    /// For token bucket: maximum burst capacity.
    pub burst: Option<u64>,
}

/// Rate limit tier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitTier {
    /// Tier name.
    pub name: String,
    /// Requests per second (token bucket).
    pub req_per_second: u64,
    /// Burst capacity for requests.
    pub burst: u64,
    /// Tokens per minute (sliding window).
    pub tok_per_minute: u64,
    /// Maximum concurrent requests.
    pub concurrent: u64,
}

impl Default for RateLimitTier {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            req_per_second: 100,
            burst: 200,
            tok_per_minute: 1_000_000,
            concurrent: 20,
        }
    }
}

/// Predefined rate limit tiers.
pub fn default_tiers() -> Vec<RateLimitTier> {
    vec![
        RateLimitTier {
            name: "free".to_string(),
            req_per_second: 10,
            burst: 20,
            tok_per_minute: 100_000,
            concurrent: 5,
        },
        RateLimitTier {
            name: "small_business".to_string(),
            req_per_second: 100,
            burst: 200,
            tok_per_minute: 1_000_000,
            concurrent: 20,
        },
        RateLimitTier {
            name: "business".to_string(),
            req_per_second: 500,
            burst: 1000,
            tok_per_minute: 5_000_000,
            concurrent: 100,
        },
        RateLimitTier {
            name: "enterprise".to_string(),
            req_per_second: 2000,
            burst: 4000,
            tok_per_minute: 20_000_000,
            concurrent: 500,
        },
    ]
}
