//! L1 in-process cache backed by `moka`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use moka::future::Cache;

use crate::types::CachedResponse;

/// Statistics for the L1 cache.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: u64,
}

/// L1 in-process cache — sub-millisecond lookups, LRU + TTL eviction.
#[derive(Clone)]
pub struct L1Cache {
    inner: Cache<String, CachedResponse>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl L1Cache {
    /// Create a new L1 cache with the given capacity and TTL.
    pub fn new(max_capacity: u64, ttl_seconds: u64) -> Self {
        let inner = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();

        Self {
            inner,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create with default settings (10K entries, 60s TTL).
    pub fn default() -> Self {
        Self::new(10_000, 60)
    }

    /// Get a cached entry by key.
    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        match self.inner.get(key).await {
            Some(value) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(value)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Insert a cached entry.
    pub async fn insert(&self, key: String, value: CachedResponse) {
        self.inner.insert(key, value).await;
    }

    /// Invalidate a single entry.
    pub async fn invalidate(&self, key: &str) {
        self.inner.invalidate(key).await;
    }

    /// Invalidate all entries.
    pub async fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }

    /// Return current cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            size: self.inner.entry_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dummy_response() -> CachedResponse {
        CachedResponse {
            body: r#"{"choices":[{"message":{"content":"Hello"}}]}"#.to_string(),
            provider: "openai".to_string(),
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            cached_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = L1Cache::default();
        let key = "test-key".to_string();
        let resp = dummy_response();

        cache.insert(key.clone(), resp.clone()).await;
        let got = cache.get(&key).await;

        assert!(got.is_some());
        assert_eq!(got.unwrap().body, resp.body);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 0);
    }

    #[tokio::test]
    async fn test_miss() {
        let cache = L1Cache::default();
        let got = cache.get("missing").await;

        assert!(got.is_none());
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 1);
    }

    #[tokio::test]
    async fn test_invalidate() {
        let cache = L1Cache::default();
        let key = "test-key".to_string();
        cache.insert(key.clone(), dummy_response()).await;

        cache.invalidate(&key).await;
        let got = cache.get(&key).await;
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let cache = L1Cache::new(100, 1); // 1 second TTL
        let key = "test-key".to_string();
        cache.insert(key.clone(), dummy_response()).await;

        // Should exist immediately
        assert!(cache.get(&key).await.is_some());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_capacity_eviction() {
        let cache = L1Cache::new(3, 3600); // capacity 3

        for i in 0..5 {
            let mut resp = dummy_response();
            resp.body = format!("response-{i}");
            cache.insert(format!("key-{i}"), resp).await;
        }

        // Some of the earliest entries should have been evicted
        let stats = cache.stats();
        assert!(stats.size <= 3, "size {} should be <= 3", stats.size);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let cache = L1Cache::default();
        let mut handles = vec![];

        for i in 0..100 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let key = format!("key-{i}");
                cache.insert(key.clone(), dummy_response()).await;
                cache.get(&key).await
            }));
        }

        for h in handles {
            assert!(h.await.unwrap().is_some());
        }
    }
}
