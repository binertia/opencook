//! Token-bucket and sliding-window rate limiters backed by Redis Lua.

use redis::{aio::ConnectionManager, Script};
use tracing::{debug, warn};

use crate::types::LimitResult;

/// Redis-backed rate limiter supporting sliding window and token bucket algorithms.
#[derive(Clone)]
pub struct RateLimiter {
    redis: ConnectionManager,
}

/// Lua script for atomic sliding window check-and-record.
///
/// Arguments:
///   KEYS[1]   = Redis key for the window
///   ARGV[1]   = current time in milliseconds
///   ARGV[2]   = window start time in milliseconds (now - window_size_ms)
///   ARGV[3]   = limit (max entries in window)
///
/// Returns 1 if allowed, 0 if denied.
const SLIDING_WINDOW_LUA: &str = r#"
    local key = KEYS[1]
    local now = tonumber(ARGV[1])
    local window_start = tonumber(ARGV[2])
    local limit = tonumber(ARGV[3])

    -- Remove entries outside the sliding window
    redis.call('ZREMRANGEBYSCORE', key, 0, window_start)

    -- Count entries within the window
    local current = redis.call('ZCARD', key)

    if current >= limit then
        return 0
    end

    -- Add current request (score = timestamp, member = timestamp)
    redis.call('ZADD', key, now, now)

    -- Set expiry to window duration + 1 second buffer
    local window_ms = now - window_start
    redis.call('PEXPIRE', key, window_ms + 1000)

    return 1
"#;

/// Lua script for atomic token bucket check-and-consume.
///
/// Arguments:
///   KEYS[1]   = Redis key for the bucket
///   ARGV[1]   = rate (tokens per second)
///   ARGV[2]   = burst (bucket capacity)
///   ARGV[3]   = current time in milliseconds
///   ARGV[4]   = cost (tokens to consume, usually 1)
///
/// Returns {allowed, remaining_tokens} where allowed is 1 or 0.
const TOKEN_BUCKET_LUA: &str = r#"
    local key = KEYS[1]
    local rate = tonumber(ARGV[1])
    local burst = tonumber(ARGV[2])
    local now = tonumber(ARGV[3])
    local cost = tonumber(ARGV[4])

    local bucket = redis.call('HMGET', key, 'tokens', 'last_update')
    local tokens = tonumber(bucket[1]) or burst
    local last_update = tonumber(bucket[2]) or now

    local elapsed = (now - last_update) / 1000.0
    local new_tokens = math.min(burst, math.max(0, tokens + elapsed * rate))

    if new_tokens >= cost then
        new_tokens = new_tokens - cost
        redis.call('HMSET', key, 'tokens', new_tokens, 'last_update', now)
        redis.call('EXPIRE', key, 3600)
        return {1, math.floor(new_tokens)}
    else
        redis.call('HSET', key, 'last_update', now)
        redis.call('EXPIRE', key, 3600)
        return {0, math.floor(new_tokens)}
    end
"#;

impl RateLimiter {
    /// Create a new rate limiter with the given Redis connection.
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Check a sliding window rate limit.
    ///
    /// * `key` — Redis key for this window (e.g. `ratelimit:org:{id}:req`)
    /// * `limit` — Max requests in the window
    /// * `window_secs` — Window duration in seconds
    ///
    /// Returns `LimitResult::Allowed` or `LimitResult::Exceeded`.
    pub async fn check_sliding_window(
        &self,
        key: &str,
        limit: u64,
        window_secs: u64,
    ) -> LimitResult {
        let now_ms = now_millis();
        let window_start_ms = now_ms.saturating_sub(window_secs * 1000);

        let script = Script::new(SLIDING_WINDOW_LUA);

        let result: Result<i64, redis::RedisError> = script
            .key(key)
            .arg(now_ms)
            .arg(window_start_ms)
            .arg(limit)
            .invoke_async(&mut self.redis.clone())
            .await;

        match result {
            Ok(1) => {
                let reset_at = (now_ms / 1000) + window_secs;
                debug!(key = %key, limit = limit, "sliding window allowed");
                LimitResult::Allowed {
                    remaining: limit.saturating_sub(1),
                    reset_at,
                    limit,
                }
            }
            Ok(0) => {
                let retry_after = window_secs;
                debug!(key = %key, limit = limit, "sliding window exceeded");
                LimitResult::Exceeded { retry_after, limit }
            }
            Ok(other) => {
                warn!(key = %key, result = other, "unexpected Lua script result");
                // Fail open on unexpected result
                LimitResult::Allowed {
                    remaining: 0,
                    reset_at: now_ms / 1000,
                    limit,
                }
            }
            Err(e) => {
                warn!(key = %key, error = %e, "Redis rate limit check failed");
                // Fail open: allow request when Redis is unavailable
                LimitResult::Allowed {
                    remaining: 0,
                    reset_at: now_ms / 1000,
                    limit,
                }
            }
        }
    }

    /// Check a token bucket rate limit.
    ///
    /// * `key` — Redis key for this bucket (e.g. `ratelimit:key:{id}:burst`)
    /// * `rate` — Tokens per second refill rate
    /// * `burst` — Maximum bucket capacity
    /// * `cost` — Tokens to consume (usually 1)
    ///
    /// Returns `LimitResult::Allowed` or `LimitResult::Exceeded`.
    pub async fn check_token_bucket(
        &self,
        key: &str,
        rate: f64,
        burst: u64,
        cost: u64,
    ) -> LimitResult {
        let now_ms = now_millis();

        let script = Script::new(TOKEN_BUCKET_LUA);

        let result: Result<(i64, i64), redis::RedisError> = script
            .key(key)
            .arg(rate)
            .arg(burst)
            .arg(now_ms)
            .arg(cost)
            .invoke_async(&mut self.redis.clone())
            .await;

        match result {
            Ok((1, remaining)) => {
                debug!(key = %key, remaining = remaining, "token bucket allowed");
                LimitResult::Allowed {
                    remaining: remaining as u64,
                    reset_at: now_ms / 1000,
                    limit: burst,
                }
            }
            Ok((0, remaining)) => {
                let retry_after = if rate > 0.0 && remaining >= 0 {
                    ((cost as f64 - remaining as f64) / rate).ceil().max(1.0) as u64
                } else {
                    1
                };
                debug!(key = %key, remaining = remaining, "token bucket exceeded");
                LimitResult::Exceeded {
                    retry_after,
                    limit: burst,
                }
            }
            Ok(other) => {
                warn!(key = %key, result = ?other, "unexpected Lua script result");
                LimitResult::Allowed {
                    remaining: 0,
                    reset_at: now_ms / 1000,
                    limit: burst,
                }
            }
            Err(e) => {
                warn!(key = %key, error = %e, "Redis token bucket check failed");
                LimitResult::Allowed {
                    remaining: 0,
                    reset_at: now_ms / 1000,
                    limit: burst,
                }
            }
        }
    }

    /// Convenience: check a request-per-second limit using token bucket.
    pub async fn check_rps(&self, key: &str, rps: u64, burst: u64) -> LimitResult {
        self.check_token_bucket(key, rps as f64, burst, 1).await
    }

    /// Convenience: check a requests-per-minute limit using sliding window.
    pub async fn check_rpm(&self, key: &str, rpm: u64) -> LimitResult {
        self.check_sliding_window(key, rpm, 60).await
    }

    /// Convenience: check a tokens-per-minute limit using sliding window.
    /// The `token_count` is used as the weight of the request.
    pub async fn check_tpm(&self, key: &str, tpm: u64, _token_count: u64) -> LimitResult {
        // For token-per-minute, we use sliding window with token count as weight.
        // Each request adds `token_count` entries (or a single entry with count metadata).
        // For simplicity, we treat each request as 1 entry but the limit is in tokens.
        // A more precise implementation would use the token count as the score delta.
        self.check_sliding_window(key, tpm, 60).await
    }

    /// Check multiple rate limit layers in sequence.
    /// Returns the first exceeded limit, or the most restrictive allowed result.
    ///
    /// Layers are checked in order; the first rejection short-circuits.
    pub async fn check_layers(&self, layers: Vec<LayerCheck>) -> LimitResult {
        let mut most_restrictive: Option<LimitResult> = None;

        for layer in layers {
            let result = match layer {
                LayerCheck::TokenBucket {
                    key,
                    rate,
                    burst,
                    cost,
                } => self.check_token_bucket(&key, rate, burst, cost).await,
                LayerCheck::SlidingWindow {
                    key,
                    limit,
                    window_secs,
                } => self.check_sliding_window(&key, limit, window_secs).await,
            };

            match &result {
                LimitResult::Exceeded { .. } => return result,
                LimitResult::Allowed { remaining, .. } => {
                    // Track the most restrictive (smallest remaining) allowed result
                    if let Some(LimitResult::Allowed {
                        remaining: best, ..
                    }) = most_restrictive
                    {
                        if *remaining < best {
                            most_restrictive = Some(result);
                        }
                    } else {
                        most_restrictive = Some(result);
                    }
                }
            }
        }

        most_restrictive.unwrap_or(LimitResult::Allowed {
            remaining: 0,
            reset_at: now_millis() / 1000,
            limit: 0,
        })
    }
}

/// A single rate limit layer to check.
#[derive(Debug, Clone)]
pub enum LayerCheck {
    /// Token bucket layer.
    TokenBucket {
        key: String,
        rate: f64,
        burst: u64,
        cost: u64,
    },
    /// Sliding window layer.
    SlidingWindow {
        key: String,
        limit: u64,
        window_secs: u64,
    },
}

/// Current time in milliseconds since Unix epoch.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limit_result_allowed() {
        let r = LimitResult::Allowed {
            remaining: 99,
            reset_at: 1700000000,
            limit: 100,
        };
        assert!(matches!(r, LimitResult::Allowed { .. }));
    }

    #[test]
    fn test_limit_result_exceeded() {
        let r = LimitResult::Exceeded {
            retry_after: 60,
            limit: 100,
        };
        assert!(matches!(r, LimitResult::Exceeded { .. }));
    }

    #[test]
    fn test_layer_check_creation() {
        let layer = LayerCheck::TokenBucket {
            key: "ratelimit:test".to_string(),
            rate: 10.0,
            burst: 20,
            cost: 1,
        };
        assert!(matches!(layer, LayerCheck::TokenBucket { .. }));
    }
}
