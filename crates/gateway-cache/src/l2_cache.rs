//! L2 Redis-backed cache.

use std::time::Duration;

use redis::aio::ConnectionManager;
use tracing::{debug, error, warn};

use crate::types::CachedResponse;

/// L2 cache backed by Redis.
#[derive(Clone)]
pub struct L2Cache {
    redis: ConnectionManager,
}

impl L2Cache {
    /// Create a new L2 cache from a Redis connection manager.
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Get a cached entry by key.
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        match redis::cmd("GET")
            .arg(key)
            .query_async::<_, Option<String>>(&mut self.redis.clone())
            .await
        {
            Ok(Some(json)) => match serde_json::from_str::<CachedResponse>(&json) {
                Ok(value) => {
                    debug!(key = %key, "L2 cache hit");
                    Some(value)
                }
                Err(e) => {
                    warn!(key = %key, error = %e, "L2 cache deserialization failed");
                    None
                }
            },
            Ok(None) => {
                debug!(key = %key, "L2 cache miss");
                None
            }
            Err(e) => {
                warn!(key = %key, error = %e, "L2 cache Redis error (pass-through)");
                None
            }
        }
    }

    /// Insert a cached entry with TTL.
    pub async fn insert(&self, key: String, value: CachedResponse, ttl: Duration) {
        let json = match serde_json::to_string(&value) {
            Ok(j) => j,
            Err(e) => {
                error!(key = %key, error = %e, "L2 cache serialization failed");
                return;
            }
        };

        match redis::cmd("SETEX")
            .arg(&key)
            .arg(ttl.as_secs() as i64)
            .arg(json)
            .query_async::<_, ()>(&mut self.redis.clone())
            .await
        {
            Ok(()) => {
                debug!(key = %key, ttl = ttl.as_secs(), "L2 cache insert");
            }
            Err(e) => {
                warn!(key = %key, error = %e, "L2 cache insert failed (non-fatal)");
            }
        }
    }

    /// Invalidate a single entry.
    pub async fn invalidate(&self, key: &str) {
        match redis::cmd("DEL")
            .arg(key)
            .query_async::<_, ()>(&mut self.redis.clone())
            .await
        {
            Ok(()) => {
                debug!(key = %key, "L2 cache invalidated");
            }
            Err(e) => {
                warn!(key = %key, error = %e, "L2 cache invalidation failed");
            }
        }
    }

    /// Invalidate entries matching a Redis key pattern (uses SCAN + DEL).
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.redis.clone();
        let mut cursor = 0u64;
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;

            if !keys.is_empty() {
                redis::cmd("DEL")
                    .arg(&keys)
                    .query_async::<_, ()>(&mut conn)
                    .await?;
                debug!(pattern = %pattern, count = keys.len(), "L2 cache pattern invalidation");
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }
        Ok(())
    }
}
