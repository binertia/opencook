//! Two-tier cache — L1 (moka) + L2 (Redis) with promotion.

use std::time::Duration;

use redis::aio::ConnectionManager;
use tracing::{debug, warn};

use crate::l1_cache::{CacheStats, L1Cache};
use crate::l2_cache::L2Cache;
use crate::types::CachedResponse;

/// Unified two-tier cache.
///
/// Lookup order: L1 → L2 → None.
/// On L2 hit: entry is promoted to L1.
/// Writes: L2 first, then L1.
#[derive(Clone)]
pub struct TwoTierCache {
    l1: L1Cache,
    l2: L2Cache,
}

impl TwoTierCache {
    /// Create a new two-tier cache from a Redis connection manager.
    pub fn new(redis: ConnectionManager) -> Self {
        Self {
            l1: L1Cache::default(),
            l2: L2Cache::new(redis),
        }
    }

    /// Create with custom L1 settings.
    pub fn with_l1(redis: ConnectionManager, l1_capacity: u64, l1_ttl_seconds: u64) -> Self {
        Self {
            l1: L1Cache::new(l1_capacity, l1_ttl_seconds),
            l2: L2Cache::new(redis),
        }
    }

    /// Get a cached entry. Tries L1 first, then L2 (promoting on hit).
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        // L1 first
        if let Some(value) = self.l1.get(key).await {
            debug!(key = %key, "Two-tier cache L1 hit");
            return Some(value);
        }

        // L2 second
        match self.l2.get(key).await {
            Some(value) => {
                debug!(key = %key, "Two-tier cache L2 hit → promoting to L1");
                // Promote to L1 asynchronously (fire-and-forget)
                let l1 = self.l1.clone();
                let key = key.to_string();
                let value_clone = value.clone();
                tokio::spawn(async move {
                    l1.insert(key, value_clone).await;
                });
                Some(value)
            }
            None => {
                debug!(key = %key, "Two-tier cache miss");
                None
            }
        }
    }

    /// Insert into both tiers. L2 first, then L1.
    pub async fn insert(&self, key: String, value: CachedResponse, ttl: Duration) {
        // Write L2 first (source of truth)
        self.l2.insert(key.clone(), value.clone(), ttl).await;
        // Then L1 (may evict, but L2 has it)
        self.l1.insert(key, value).await;
    }

    /// Invalidate from both tiers.
    pub async fn invalidate(&self, key: &str) {
        self.l1.invalidate(key).await;
        self.l2.invalidate(key).await;
    }

    /// Invalidate entries matching a pattern from L2 (and corresponding L1 entries).
    pub async fn invalidate_pattern(&self, pattern: &str) {
        // L2 pattern invalidation
        if let Err(e) = self.l2.invalidate_pattern(pattern).await {
            warn!(pattern = %pattern, error = %e, "L2 pattern invalidation failed");
        }
        // L1 doesn't support pattern invalidation, so we invalidate all
        // (conservative but correct for the gateway use-case)
        self.l1.invalidate_all().await;
    }

    /// Return L1 cache statistics.
    pub fn l1_stats(&self) -> CacheStats {
        self.l1.stats()
    }
}
