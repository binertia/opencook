//! Quota usage tracking repository.

use crate::pool::DbBackend;
use crate::types::DbDecimal;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::DbError;
use crate::models::QuotaUsage;

/// Repository for quota usage records.
#[derive(Clone)]
pub struct QuotaUsageRepo {
    pool: DbBackend,
}

impl QuotaUsageRepo {
    /// Create a new quota usage repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Get or create a quota usage record for the current period.
    ///
    /// First attempts SELECT; if not found, INSERTs a new record.
    /// This two-step approach is required because PostgreSQL treats NULLs
    /// as distinct in unique constraints, so ON CONFLICT may not match
    /// when api_key_id is NULL.
    pub async fn get_or_create(
        &self,
        org_id: Uuid,
        quota_id: Uuid,
        api_key_id: Option<Uuid>,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        limit_value: impl Into<DbDecimal>,
        metric: &str,
    ) -> Result<QuotaUsage, DbError> {
        let limit_value = limit_value.into();
        // Try to find existing record first
        let existing = match &self.pool {
            DbBackend::Postgres(pg) => {
                if let Some(key_id) = api_key_id {
                    sqlx::query_as::<_, QuotaUsage>(
                        r#"
                        SELECT id, org_id, quota_id, api_key_id,
                               period_start, period_end,
                               current_value, limit_value, metric::text,
                               exceeded_at, warned_at,
                               created_at, updated_at, deleted_at
                        FROM quota_usage
                        WHERE org_id = $1
                          AND quota_id = $2
                          AND api_key_id = $3
                          AND period_start = $4
                          AND deleted_at IS NULL
                        "#,
                    )
                    .bind(org_id)
                    .bind(quota_id)
                    .bind(key_id)
                    .bind(period_start)
                    .fetch_optional(pg)
                    .await?
                } else {
                    sqlx::query_as::<_, QuotaUsage>(
                        r#"
                        SELECT id, org_id, quota_id, api_key_id,
                               period_start, period_end,
                               current_value, limit_value, metric::text,
                               exceeded_at, warned_at,
                               created_at, updated_at, deleted_at
                        FROM quota_usage
                        WHERE org_id = $1
                          AND quota_id = $2
                          AND api_key_id IS NULL
                          AND period_start = $3
                          AND deleted_at IS NULL
                        "#,
                    )
                    .bind(org_id)
                    .bind(quota_id)
                    .bind(period_start)
                    .fetch_optional(pg)
                    .await?
                }
            }
            DbBackend::Sqlite(sqlite) => {
                if let Some(key_id) = api_key_id {
                    sqlx::query_as::<_, QuotaUsage>(
                        r#"
                        SELECT id, org_id, quota_id, api_key_id,
                               period_start, period_end,
                               current_value, limit_value, metric,
                               exceeded_at, warned_at,
                               created_at, updated_at, deleted_at
                        FROM quota_usage
                        WHERE org_id = $1
                          AND quota_id = $2
                          AND api_key_id = $3
                          AND period_start = $4
                          AND deleted_at IS NULL
                        "#,
                    )
                    .bind(org_id)
                    .bind(quota_id)
                    .bind(key_id)
                    .bind(period_start)
                    .fetch_optional(sqlite)
                    .await?
                } else {
                    sqlx::query_as::<_, QuotaUsage>(
                        r#"
                        SELECT id, org_id, quota_id, api_key_id,
                               period_start, period_end,
                               current_value, limit_value, metric,
                               exceeded_at, warned_at,
                               created_at, updated_at, deleted_at
                        FROM quota_usage
                        WHERE org_id = $1
                          AND quota_id = $2
                          AND api_key_id IS NULL
                          AND period_start = $3
                          AND deleted_at IS NULL
                        "#,
                    )
                    .bind(org_id)
                    .bind(quota_id)
                    .bind(period_start)
                    .fetch_optional(sqlite)
                    .await?
                }
            }
        };

        if let Some(row) = existing {
            return Ok(row);
        }

        // Insert new record
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, QuotaUsage>(
                    r#"
                    INSERT INTO quota_usage (
                        org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric
                    )
                    VALUES ($1, $2, $3, $4, $5, 0, $6, $7::text::quota_metric)
                    RETURNING
                        id, org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric::text,
                        exceeded_at, warned_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .bind(period_end)
                .bind(limit_value)
                .bind(metric)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, QuotaUsage>(
                    r#"
                    INSERT INTO quota_usage (
                        org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric
                    )
                    VALUES ($1, $2, $3, $4, $5, 0, $6, $7)
                    RETURNING
                        id, org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric,
                        exceeded_at, warned_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .bind(period_end)
                .bind(limit_value)
                .bind(metric)
                .fetch_one(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Atomically increment quota usage by the given amount.
    ///
    /// Returns the updated record.
    pub async fn increment(
        &self,
        org_id: Uuid,
        quota_id: Uuid,
        api_key_id: Option<Uuid>,
        period_start: DateTime<Utc>,
        amount: impl Into<DbDecimal>,
    ) -> Result<QuotaUsage, DbError> {
        let amount = amount.into();
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, QuotaUsage>(
                    r#"
                    UPDATE quota_usage
                    SET current_value = current_value + $5,
                        updated_at = NOW()
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    RETURNING
                        id, org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric,
                        exceeded_at, warned_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .bind(amount)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, QuotaUsage>(
                    r#"
                    UPDATE quota_usage
                    SET current_value = current_value + $5,
                        updated_at = datetime('now')
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    RETURNING
                        id, org_id, quota_id, api_key_id,
                        period_start, period_end,
                        current_value, limit_value, metric,
                        exceeded_at, warned_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .bind(amount)
                .fetch_one(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Mark a quota usage record as exceeded.
    pub async fn mark_exceeded(
        &self,
        org_id: Uuid,
        quota_id: Uuid,
        api_key_id: Option<Uuid>,
        period_start: DateTime<Utc>,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE quota_usage
                    SET exceeded_at = NOW(),
                        updated_at = NOW()
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE quota_usage
                    SET exceeded_at = datetime('now'),
                        updated_at = datetime('now')
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .execute(sqlite)
                .await?;
            }
        };

        Ok(())
    }

    /// Mark a quota usage record as warned.
    pub async fn mark_warned(
        &self,
        org_id: Uuid,
        quota_id: Uuid,
        api_key_id: Option<Uuid>,
        period_start: DateTime<Utc>,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE quota_usage
                    SET warned_at = NOW(),
                        updated_at = NOW()
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE quota_usage
                    SET warned_at = datetime('now'),
                        updated_at = datetime('now')
                    WHERE org_id = $1
                      AND quota_id = $2
                      AND api_key_id IS NOT DISTINCT FROM $3
                      AND period_start = $4
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(quota_id)
                .bind(api_key_id)
                .bind(period_start)
                .execute(sqlite)
                .await?;
            }
        };

        Ok(())
    }
}
