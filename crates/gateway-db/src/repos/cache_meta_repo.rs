//! Cache metadata repository.

use crate::error::DbError;
use crate::models::CacheMetadata;
use crate::pool::DbBackend;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

/// Repository for the `cache_metadata` table.
#[derive(Clone)]
pub struct CacheMetaRepo {
    pool: DbBackend,
}

impl CacheMetaRepo {
    /// Create a new cache metadata repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Access the underlying database pool.
    pub fn pool(&self) -> &DbBackend {
        &self.pool
    }

    /// Upsert cache metadata on insert (creates or updates hit_count on conflict).
    pub async fn upsert(
        &self,
        org_id: Uuid,
        cache_key_hash: &str,
        cache_key_preview: Option<&str>,
        model_id: &str,
        prompt_preview: Option<&str>,
        prompt_tokens: i32,
        storage_backend: &str,
        ttl_seconds: i32,
        expires_at: DateTime<Utc>,
        content_hash: Option<&str>,
    ) -> Result<CacheMetadata, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, CacheMetadata>(
                    r#"
                    INSERT INTO cache_metadata (
                        org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, content_hash
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, $10)
                    ON CONFLICT (org_id, cache_key_hash) WHERE deleted_at IS NULL
                    DO UPDATE SET
                        updated_at = now(),
                        expires_at = EXCLUDED.expires_at,
                        ttl_seconds = EXCLUDED.ttl_seconds,
                        content_hash = EXCLUDED.content_hash
                    RETURNING
                        id, org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, last_hit_at, content_hash,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .bind(cache_key_preview)
                .bind(model_id)
                .bind(prompt_preview)
                .bind(prompt_tokens)
                .bind(storage_backend)
                .bind(ttl_seconds)
                .bind(expires_at)
                .bind(content_hash)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                // SQLite doesn't support ON CONFLICT with partial indexes the same way;
                // attempt insert, and if it fails, update.
                let insert_result = sqlx::query_as::<_, CacheMetadata>(
                    r#"
                    INSERT INTO cache_metadata (
                        org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, content_hash
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, $10)
                    RETURNING
                        id, org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, last_hit_at, content_hash,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .bind(cache_key_preview)
                .bind(model_id)
                .bind(prompt_preview)
                .bind(prompt_tokens)
                .bind(storage_backend)
                .bind(ttl_seconds)
                .bind(expires_at)
                .bind(content_hash)
                .fetch_one(sqlite)
                .await;

                match insert_result {
                    Ok(row) => row,
                    Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => {
                        sqlx::query_as::<_, CacheMetadata>(
                            r#"
                            UPDATE cache_metadata
                            SET updated_at = datetime('now'),
                                expires_at = $3,
                                ttl_seconds = $4,
                                content_hash = $5
                            WHERE org_id = $1 AND cache_key_hash = $2 AND deleted_at IS NULL
                            RETURNING
                                id, org_id, cache_key_hash, cache_key_preview, model_id,
                                prompt_preview, prompt_tokens, storage_backend,
                                ttl_seconds, expires_at, hit_count, last_hit_at, content_hash,
                                created_at, updated_at, deleted_at
                            "#,
                        )
                        .bind(org_id)
                        .bind(cache_key_hash)
                        .bind(expires_at)
                        .bind(ttl_seconds)
                        .bind(content_hash)
                        .fetch_one(sqlite)
                        .await?
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        };

        Ok(row)
    }

    /// Increment hit count and update last_hit_at for a cache entry.
    pub async fn record_hit(
        &self,
        org_id: Uuid,
        cache_key_hash: &str,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE cache_metadata
                    SET hit_count = hit_count + 1,
                        last_hit_at = now(),
                        updated_at = now()
                    WHERE org_id = $1 AND cache_key_hash = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE cache_metadata
                    SET hit_count = hit_count + 1,
                        last_hit_at = datetime('now'),
                        updated_at = datetime('now')
                    WHERE org_id = $1 AND cache_key_hash = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .execute(sqlite)
                .await?;
            }
        };

        Ok(())
    }

    /// Get cache metadata by org + key hash.
    pub async fn get_by_hash(
        &self,
        org_id: Uuid,
        cache_key_hash: &str,
    ) -> Result<Option<CacheMetadata>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, CacheMetadata>(
                    r#"
                    SELECT
                        id, org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, last_hit_at, content_hash,
                        created_at, updated_at, deleted_at
                    FROM cache_metadata
                    WHERE org_id = $1 AND cache_key_hash = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, CacheMetadata>(
                    r#"
                    SELECT
                        id, org_id, cache_key_hash, cache_key_preview, model_id,
                        prompt_preview, prompt_tokens, storage_backend,
                        ttl_seconds, expires_at, hit_count, last_hit_at, content_hash,
                        created_at, updated_at, deleted_at
                    FROM cache_metadata
                    WHERE org_id = $1 AND cache_key_hash = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(cache_key_hash)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Get hit rate for an org over a time period.
    pub async fn get_hit_rate(
        &self,
        org_id: Uuid,
        start: DateTime<Utc>,
    ) -> Result<f64, DbError> {
        let (total_hits, total_entries) = match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query(
                    r#"
                    SELECT
                        COALESCE(SUM(hit_count), 0)::float8 AS total_hits,
                        COUNT(*)::float8 AS total_entries
                    FROM cache_metadata
                    WHERE org_id = $1 AND deleted_at IS NULL
                      AND created_at >= $2
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .fetch_one(pg)
                .await?;
                let total_hits: f64 = row.try_get("total_hits").unwrap_or(0.0);
                let total_entries: f64 = row.try_get("total_entries").unwrap_or(0.0);
                (total_hits, total_entries)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query(
                    r#"
                    SELECT
                        COALESCE(SUM(hit_count), 0) AS total_hits,
                        COUNT(*) AS total_entries
                    FROM cache_metadata
                    WHERE org_id = $1 AND deleted_at IS NULL
                      AND created_at >= $2
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .fetch_one(sqlite)
                .await?;
                let total_hits: f64 = row.try_get("total_hits").unwrap_or(0.0);
                let total_entries: f64 = row.try_get("total_entries").unwrap_or(0.0);
                (total_hits, total_entries)
            }
        };

        if total_entries == 0.0 {
            Ok(0.0)
        } else {
            // Hit rate = total_hits / (total_hits + total_entries)
            // where total_entries represents the original inserts (misses)
            let total_requests = total_hits + total_entries;
            Ok(total_hits / total_requests)
        }
    }

    /// Get top cached models by hit count.
    pub async fn get_top_models(
        &self,
        org_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ModelCacheStats>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, ModelCacheStats>(
                    r#"
                    SELECT
                        model_id,
                        COUNT(*) AS entry_count,
                        SUM(hit_count) AS total_hits,
                        AVG(hit_count)::float8 AS avg_hits
                    FROM cache_metadata
                    WHERE org_id = $1 AND deleted_at IS NULL
                    GROUP BY model_id
                    ORDER BY total_hits DESC
                    LIMIT $2
                    "#,
                )
                .bind(org_id)
                .bind(limit)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, ModelCacheStats>(
                    r#"
                    SELECT
                        model_id,
                        COUNT(*) AS entry_count,
                        SUM(hit_count) AS total_hits,
                        AVG(hit_count) AS avg_hits
                    FROM cache_metadata
                    WHERE org_id = $1 AND deleted_at IS NULL
                    GROUP BY model_id
                    ORDER BY total_hits DESC
                    LIMIT $2
                    "#,
                )
                .bind(org_id)
                .bind(limit)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }
}

/// Statistics for a single model's cache performance.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ModelCacheStats {
    pub model_id: String,
    pub entry_count: i64,
    pub total_hits: i64,
    pub avg_hits: f64,
}
