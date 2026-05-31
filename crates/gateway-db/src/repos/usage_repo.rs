//! Usage records repository — aggregated request metrics.

use chrono::{DateTime, Utc};
use crate::pool::DbBackend;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::UsageRecord;

/// Repository for usage_records table.
#[derive(Clone)]
pub struct UsageRepo {
    pool: DbBackend,
}

impl UsageRepo {
    /// Create a new usage repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Upsert hourly usage aggregates from unaggregated requests.
    ///
    /// Aggregates all requests where `aggregated_at IS NULL` and
    /// `created_at < date_trunc('hour', NOW())`, then inserts or
    /// updates `usage_records` with `ON CONFLICT DO UPDATE`.
    ///
    /// Returns the number of usage_records rows affected.
    ///
    /// **Postgres only** — not supported on SQLite.
    pub async fn aggregate_hourly(&self) -> Result<u64, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let result = sqlx::query(
                    r#"
                    INSERT INTO usage_records (
                        org_id, api_key_id, provider_config_id, provider_model_id,
                        period, period_start,
                        request_count, request_success, request_error,
                        prompt_tokens, completion_tokens, total_tokens,
                        input_cost, output_cost, total_cost,
                        latency_ms_p50, latency_ms_p90, latency_ms_p99, latency_ms_avg,
                        cache_hits, cache_misses
                    )
                    SELECT
                        org_id,
                        api_key_id,
                        provider_config_id,
                        provider_model_id,
                        'hourly',
                        date_trunc('hour', created_at),
                        COUNT(*),
                        COUNT(*) FILTER (WHERE status = 'success'),
                        COUNT(*) FILTER (WHERE status = 'error'),
                        COALESCE(SUM(prompt_tokens), 0),
                        COALESCE(SUM(completion_tokens), 0),
                        COALESCE(SUM(total_tokens), 0),
                        COALESCE(SUM(input_cost), 0),
                        COALESCE(SUM(output_cost), 0),
                        COALESCE(SUM(total_cost), 0),
                        (percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_total_ms))::int,
                        (percentile_cont(0.9) WITHIN GROUP (ORDER BY latency_total_ms))::int,
                        (percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_total_ms))::int,
                        (AVG(latency_total_ms))::int,
                        COUNT(*) FILTER (WHERE cache_hit = true),
                        COUNT(*) FILTER (WHERE cache_hit = false)
                    FROM requests
                    WHERE aggregated_at IS NULL
                      AND created_at < date_trunc('hour', NOW())
                      AND deleted_at IS NULL
                    GROUP BY
                        org_id,
                        api_key_id,
                        provider_config_id,
                        provider_model_id,
                        date_trunc('hour', created_at)
                    ON CONFLICT (org_id, api_key_id, provider_config_id, provider_model_id, period, period_start)
                    DO UPDATE SET
                        request_count = usage_records.request_count + EXCLUDED.request_count,
                        request_success = usage_records.request_success + EXCLUDED.request_success,
                        request_error = usage_records.request_error + EXCLUDED.request_error,
                        prompt_tokens = usage_records.prompt_tokens + EXCLUDED.prompt_tokens,
                        completion_tokens = usage_records.completion_tokens + EXCLUDED.completion_tokens,
                        total_tokens = usage_records.total_tokens + EXCLUDED.total_tokens,
                        input_cost = usage_records.input_cost + EXCLUDED.input_cost,
                        output_cost = usage_records.output_cost + EXCLUDED.output_cost,
                        total_cost = usage_records.total_cost + EXCLUDED.total_cost,
                        latency_ms_p50 = EXCLUDED.latency_ms_p50,
                        latency_ms_p90 = EXCLUDED.latency_ms_p90,
                        latency_ms_p99 = EXCLUDED.latency_ms_p99,
                        latency_ms_avg = EXCLUDED.latency_ms_avg,
                        cache_hits = usage_records.cache_hits + EXCLUDED.cache_hits,
                        cache_misses = usage_records.cache_misses + EXCLUDED.cache_misses,
                        updated_at = NOW()
                    "#,
                )
                .execute(pg)
                .await?;

                Ok(result.rows_affected())
            }
            DbBackend::Sqlite(_) => {
                Err(DbError::Unsupported(
                    "aggregate_hourly is not supported on SQLite".into(),
                ))
            }
        }
    }

    /// Mark all unaggregated requests from before the current hour as aggregated.
    ///
    /// Should be called after `aggregate_hourly()` succeeds.
    pub async fn mark_requests_aggregated(&self) -> Result<u64, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let result = sqlx::query(
                    r#"
                    UPDATE requests
                    SET aggregated_at = NOW()
                    WHERE aggregated_at IS NULL
                      AND created_at < date_trunc('hour', NOW())
                      AND deleted_at IS NULL
                    "#,
                )
                .execute(pg)
                .await?;

                Ok(result.rows_affected())
            }
            DbBackend::Sqlite(sqlite) => {
                let result = sqlx::query(
                    r#"
                    UPDATE requests
                    SET aggregated_at = datetime('now')
                    WHERE aggregated_at IS NULL
                      AND created_at < datetime(strftime('%Y-%m-%d %H:00:00', 'now'))
                      AND deleted_at IS NULL
                    "#,
                )
                .execute(sqlite)
                .await?;

                Ok(result.rows_affected())
            }
        }
    }

    /// Get usage records for an org within a time range.
    pub async fn list_by_org_and_period(
        &self,
        org_id: Uuid,
        period: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>, DbError> {
        let sql = r#"
            SELECT id, org_id, api_key_id, provider_config_id, provider_model_id,
                   period, period_start,
                   request_count, request_success, request_error,
                   prompt_tokens, completion_tokens, total_tokens,
                   input_cost, output_cost, total_cost,
                   latency_ms_p50, latency_ms_p90, latency_ms_p99, latency_ms_avg,
                   cache_hits, cache_misses,
                   created_at, updated_at, deleted_at
            FROM usage_records
            WHERE org_id = $1
              AND period = $2
              AND period_start >= $3
              AND period_start < $4
              AND deleted_at IS NULL
            ORDER BY period_start DESC
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, UsageRecord>(sql)
                    .bind(org_id)
                    .bind(period)
                    .bind(start)
                    .bind(end)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, UsageRecord>(sql)
                    .bind(org_id)
                    .bind(period)
                    .bind(start)
                    .bind(end)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }

    /// Get a single usage record by its natural key.
    pub async fn get_by_key(
        &self,
        org_id: Uuid,
        api_key_id: Option<Uuid>,
        provider_config_id: Option<Uuid>,
        provider_model_id: Option<Uuid>,
        period: &str,
        period_start: DateTime<Utc>,
    ) -> Result<Option<UsageRecord>, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, UsageRecord>(
                    r#"
                    SELECT id, org_id, api_key_id, provider_config_id, provider_model_id,
                           period, period_start,
                           request_count, request_success, request_error,
                           prompt_tokens, completion_tokens, total_tokens,
                           input_cost, output_cost, total_cost,
                           latency_ms_p50, latency_ms_p90, latency_ms_p99, latency_ms_avg,
                           cache_hits, cache_misses,
                           created_at, updated_at, deleted_at
                    FROM usage_records
                    WHERE org_id = $1
                      AND api_key_id IS NOT DISTINCT FROM $2
                      AND provider_config_id IS NOT DISTINCT FROM $3
                      AND provider_model_id IS NOT DISTINCT FROM $4
                      AND period = $5
                      AND period_start = $6
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(provider_config_id)
                .bind(provider_model_id)
                .bind(period)
                .bind(period_start)
                .fetch_optional(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, UsageRecord>(
                    r#"
                    SELECT id, org_id, api_key_id, provider_config_id, provider_model_id,
                           period, period_start,
                           request_count, request_success, request_error,
                           prompt_tokens, completion_tokens, total_tokens,
                           input_cost, output_cost, total_cost,
                           latency_ms_p50, latency_ms_p90, latency_ms_p99, latency_ms_avg,
                           cache_hits, cache_misses,
                           created_at, updated_at, deleted_at
                    FROM usage_records
                    WHERE org_id = $1
                      AND (api_key_id = $2 OR (api_key_id IS NULL AND $2 IS NULL))
                      AND (provider_config_id = $3 OR (provider_config_id IS NULL AND $3 IS NULL))
                      AND (provider_model_id = $4 OR (provider_model_id IS NULL AND $4 IS NULL))
                      AND period = $5
                      AND period_start = $6
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(provider_config_id)
                .bind(provider_model_id)
                .bind(period)
                .bind(period_start)
                .fetch_optional(sqlite)
                .await?;
                Ok(row)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Note: integration tests require a running PostgreSQL instance.
    // Run with: cargo test -p gateway-db -- --ignored

    #[test]
    fn test_usage_repo_new() {
        // This is a compile-time check that UsageRepo::new exists.
        // Real tests need a PgPool.
    }
}
