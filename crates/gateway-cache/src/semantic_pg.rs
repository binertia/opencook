//! Semantic cache backed by pgvector + Redis.
//!
//! Stores embeddings in PostgreSQL with pgvector HNSW indexing for fast
//! cosine-similarity lookup. The actual response bodies are stored in Redis
//! keyed by `response_hash` to keep the vector table lean.

use std::time::Duration;

use pgvector::Vector;
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use crate::semantic::EmbeddingClient;
use crate::types::CachedResponse;

/// PostgreSQL + pgvector backed semantic cache.
#[derive(Clone)]
pub struct PgvectorSemanticCache {
    pool: PgPool,
    redis: ConnectionManager,
    embedding: EmbeddingClient,
    similarity_threshold: f32,
}

impl PgvectorSemanticCache {
    pub fn new(
        pool: PgPool,
        redis: ConnectionManager,
        embedding: EmbeddingClient,
        similarity_threshold: f32,
    ) -> Self {
        Self {
            pool,
            redis,
            embedding,
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
        }
    }

    /// Find a semantically similar cached response using HNSW index.
    #[instrument(skip(self, embedding), fields(org_id = %org_id, model = %model))]
    pub async fn find_similar(
        &self,
        org_id: Uuid,
        model: &str,
        embedding: &[f32],
    ) -> Result<Option<CachedResponse>, sqlx::Error> {
        let vector = Vector::from(embedding.to_vec());

        // Use HNSW index for approximate nearest neighbor search with cosine distance.
        // The `<=>` operator computes cosine distance (1 - cosine_similarity).
        let threshold_distance = 1.0 - self.similarity_threshold;

        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT response_hash
            FROM query_embeddings
            WHERE org_id = $1
              AND model = $2
              AND expires_at > NOW()
              AND embedding <=> $3 <= $4
            ORDER BY embedding <=> $3
            LIMIT 1
            "#,
        )
        .bind(org_id)
        .bind(model)
        .bind(&vector)
        .bind(threshold_distance)
        .fetch_optional(&self.pool)
        .await?;

        let response_hash = match row {
            Some((hash,)) => hash,
            None => return Ok(None),
        };

        // Fetch the actual response from Redis.
        match self.fetch_response(&response_hash).await {
            Ok(Some(resp)) => {
                // Update last_hit_at for LRU tracking.
                if let Err(e) = self.touch_last_hit(org_id, model, &response_hash).await {
                    warn!(error = %e, "Failed to update last_hit_at");
                }
                Ok(Some(resp))
            }
            Ok(None) => {
                debug!(hash = %response_hash, "Response hash in pgvector but missing from Redis");
                Ok(None)
            }
            Err(e) => {
                warn!(error = %e, "Redis fetch failed for semantic cache");
                Ok(None)
            }
        }
    }

    /// Store an embedding + response hash in pgvector and the response body in Redis.
    #[instrument(skip(self, embedding), fields(org_id = %org_id, model = %model))]
    pub async fn store_embedding(
        &self,
        org_id: Uuid,
        model: &str,
        embedding: &[f32],
        response_hash: &str,
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vector = Vector::from(embedding.to_vec());
        let expires_at = chrono::Utc::now() + chrono::Duration::from_std(ttl).unwrap_or_default();

        // Insert into pgvector.
        sqlx::query(
            r#"
            INSERT INTO query_embeddings (org_id, model, embedding, response_hash, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (org_id, model, response_hash) DO UPDATE
            SET expires_at = EXCLUDED.expires_at,
                last_hit_at = NOW()
            "#,
        )
        .bind(org_id)
        .bind(model)
        .bind(&vector)
        .bind(response_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            warn!(error = %e, "Failed to insert semantic cache embedding");
            Box::new(e) as Box<dyn std::error::Error + Send + Sync>
        })?;

        // Store response in Redis.
        let redis_key = format!("semantic:response:{response_hash}");
        let json = serde_json::to_string(response).map_err(|e| {
            warn!(error = %e, "Failed to serialize cached response");
            Box::new(e) as Box<dyn std::error::Error + Send + Sync>
        })?;

        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(ttl.as_secs() as i64)
            .arg(json)
            .query_async::<_, ()>(&mut self.redis.clone())
            .await
            .map_err(|e| {
                warn!(error = %e, "Failed to store semantic cache response in Redis");
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })?;

        debug!(hash = %response_hash, ttl = ttl.as_secs(), "Semantic cache stored");
        Ok(())
    }

    /// Convenience: embed text and find similar in one call.
    pub async fn find_similar_text(
        &self,
        org_id: Uuid,
        model: &str,
        text: &str,
    ) -> Result<Option<CachedResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let embedding = self.embedding.embed(text).await.map_err(|e| {
            warn!(error = %e, "Embedding generation failed");
            Box::new(std::io::Error::other(e)) as Box<dyn std::error::Error + Send + Sync>
        })?;

        let result = self.find_similar(org_id, model, &embedding).await?;
        Ok(result)
    }

    /// Convenience: embed text and store embedding + response in one call.
    pub async fn store_text(
        &self,
        org_id: Uuid,
        model: &str,
        text: &str,
        response_hash: &str,
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embedding = self.embedding.embed(text).await.map_err(|e| {
            warn!(error = %e, "Embedding generation failed during store");
            Box::new(std::io::Error::other(e)) as Box<dyn std::error::Error + Send + Sync>
        })?;

        self.store_embedding(org_id, model, &embedding, response_hash, response, ttl)
            .await
    }

    /// Delete expired entries. Returns number of rows deleted.
    pub async fn delete_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM query_embeddings
            WHERE expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Delete oldest entries for an org+model when max entries exceeded (LRU).
    pub async fn evict_oldest(
        &self,
        org_id: Uuid,
        model: &str,
        max_entries: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM query_embeddings
            WHERE ctid IN (
                SELECT ctid FROM query_embeddings
                WHERE org_id = $1 AND model = $2
                ORDER BY COALESCE(last_hit_at, created_at) ASC
                LIMIT GREATEST(0, (SELECT COUNT(*) FROM query_embeddings WHERE org_id = $1 AND model = $2) - $3)
            )
            "#,
        )
        .bind(org_id)
        .bind(model)
        .bind(max_entries)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get statistics for the semantic cache.
    pub async fn stats(&self, org_id: Uuid) -> Result<SemanticCacheStats, sqlx::Error> {
        let row: (i64, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                MAX(created_at)
            FROM query_embeddings
            WHERE org_id = $1
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(SemanticCacheStats {
            total_entries: row.0,
            newest_entry: row.1,
        })
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    async fn fetch_response(
        &self,
        response_hash: &str,
    ) -> Result<Option<CachedResponse>, redis::RedisError> {
        let redis_key = format!("semantic:response:{response_hash}");
        let json: Option<String> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async::<_, Option<String>>(&mut self.redis.clone())
            .await?;

        match json {
            Some(j) => match serde_json::from_str(&j) {
                Ok(resp) => Ok(Some(resp)),
                Err(e) => {
                    warn!(error = %e, "Failed to deserialize cached response");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    async fn touch_last_hit(
        &self,
        org_id: Uuid,
        model: &str,
        response_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE query_embeddings
            SET last_hit_at = NOW()
            WHERE org_id = $1 AND model = $2 AND response_hash = $3
            "#,
        )
        .bind(org_id)
        .bind(model)
        .bind(response_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Statistics for the semantic cache.
#[derive(Debug, Clone)]
pub struct SemanticCacheStats {
    pub total_entries: i64,
    pub newest_entry: Option<chrono::DateTime<chrono::Utc>>,
}

/// Spawn a background maintenance task for the semantic cache.
/// Runs every `interval` and deletes expired entries.
pub fn spawn_maintenance(
    cache: PgvectorSemanticCache,
    interval: std::time::Duration,
    _max_entries_per_org_model: i64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;

            // Delete expired entries
            match cache.delete_expired().await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(deleted = deleted, "Semantic cache expired entries cleaned");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Semantic cache cleanup failed");
                }
            }

            // LRU eviction per org+model if needed (simplified: just check total)
            // In production this would iterate over orgs/models; for now we rely
            // on the per-org+model eviction being triggered by insert paths.
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_cache_stats_default() {
        let stats = SemanticCacheStats {
            total_entries: 0,
            newest_entry: None,
        };
        assert_eq!(stats.total_entries, 0);
        assert!(stats.newest_entry.is_none());
    }
}
