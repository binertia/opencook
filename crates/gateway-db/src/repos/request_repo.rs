//! Request log repository.

use crate::pool::DbBackend;
use crate::types::DbDecimal;
use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::Request;

/// Repository for request logs.
#[derive(Clone)]
pub struct RequestRepo {
    pool: DbBackend,
}

impl RequestRepo {
    /// Create a new request repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Insert a new request record (initial state: pending).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        org_id: Uuid,
        api_key_id: Option<Uuid>,
        trace_id: &str,
        method: &str,
        path: &str,
        model_requested: Option<&str>,
        request_headers: serde_json::Value,
        request_body: Option<&str>,
    ) -> Result<Request, DbError> {
        let body_truncated = request_body.map(|b| b.len() > 100_000).unwrap_or(false);
        let body =
            request_body.map(|b| if b.len() > 100_000 { &b[..100_000] } else { b }.to_string());

        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, Request>(
                    r#"
                    INSERT INTO requests (
                        org_id, api_key_id, trace_id, method, path,
                        model_requested, request_headers, request_body, request_body_truncated,
                        status, gateway_received_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', now())
                    RETURNING
                        id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                        routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                        model_routed, request_headers, request_body, request_body_truncated,
                        requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                        completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                        prompt_tokens, completion_tokens, total_tokens,
                        input_cost, output_cost, total_cost,
                        status::text, status_code, error_code, error_message, metadata,
                        cache_hit, cache_key_hash,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(trace_id)
                .bind(method)
                .bind(path)
                .bind(model_requested)
                .bind(request_headers)
                .bind(body)
                .bind(body_truncated)
                .fetch_one(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, Request>(
                    r#"
                    INSERT INTO requests (
                        org_id, api_key_id, trace_id, method, path,
                        model_requested, request_headers, request_body, request_body_truncated,
                        status, gateway_received_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', datetime('now'))
                    RETURNING
                        id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                        routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                        model_routed, request_headers, request_body, request_body_truncated,
                        requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                        completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                        prompt_tokens, completion_tokens, total_tokens,
                        input_cost, output_cost, total_cost,
                        status, status_code, error_code, error_message, metadata,
                        cache_hit, cache_key_hash,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(trace_id)
                .bind(method)
                .bind(path)
                .bind(model_requested)
                .bind(request_headers)
                .bind(body)
                .bind(body_truncated)
                .fetch_one(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// Update a request record with response data.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_response(
        &self,
        request_id: Uuid,
        org_id: Uuid,
        model_routed: Option<&str>,
        prompt_tokens: i32,
        completion_tokens: i32,
        total_tokens: i32,
        input_cost: impl Into<DbDecimal>,
        output_cost: impl Into<DbDecimal>,
        total_cost: impl Into<DbDecimal>,
        status: &str,
        status_code: Option<i32>,
        error_code: Option<&str>,
        error_message: Option<&str>,
        latency_gateway_ms: i32,
        latency_total_ms: i32,
        cache_hit: bool,
    ) -> Result<(), DbError> {
        let input_cost = input_cost.into();
        let output_cost = output_cost.into();
        let total_cost = total_cost.into();
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET model_routed = $3,
                        prompt_tokens = $4,
                        completion_tokens = $5,
                        total_tokens = $6,
                        input_cost = $7,
                        output_cost = $8,
                        total_cost = $9,
                        status = $10::request_status,
                        status_code = $11,
                        error_code = $12,
                        error_message = $13,
                        latency_gateway_ms = $14,
                        latency_total_ms = $15,
                        completed_at = now(),
                        cache_hit = $16,
                        updated_at = now()
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .bind(model_routed)
                .bind(prompt_tokens)
                .bind(completion_tokens)
                .bind(total_tokens)
                .bind(input_cost)
                .bind(output_cost)
                .bind(total_cost)
                .bind(status)
                .bind(status_code)
                .bind(error_code)
                .bind(error_message)
                .bind(latency_gateway_ms)
                .bind(latency_total_ms)
                .bind(cache_hit)
                .execute(pg)
                .await?;
                Ok(())
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET model_routed = $3,
                        prompt_tokens = $4,
                        completion_tokens = $5,
                        total_tokens = $6,
                        input_cost = $7,
                        output_cost = $8,
                        total_cost = $9,
                        status = $10,
                        status_code = $11,
                        error_code = $12,
                        error_message = $13,
                        latency_gateway_ms = $14,
                        latency_total_ms = $15,
                        completed_at = datetime('now'),
                        cache_hit = $16,
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .bind(model_routed)
                .bind(prompt_tokens)
                .bind(completion_tokens)
                .bind(total_tokens)
                .bind(input_cost)
                .bind(output_cost)
                .bind(total_cost)
                .bind(status)
                .bind(status_code)
                .bind(error_code)
                .bind(error_message)
                .bind(latency_gateway_ms)
                .bind(latency_total_ms)
                .bind(cache_hit)
                .execute(sqlite)
                .await?;
                Ok(())
            }
        }
    }

    /// Mark request as provider-sent (before awaiting response).
    pub async fn mark_provider_sent(&self, request_id: Uuid, org_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET provider_sent_at = now(),
                        status = 'processing'::request_status,
                        updated_at = now()
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .execute(pg)
                .await?;
                Ok(())
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET provider_sent_at = datetime('now'),
                        status = 'processing',
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
                Ok(())
            }
        }
    }

    /// Mark request as provider-responded.
    pub async fn mark_provider_responded(
        &self,
        request_id: Uuid,
        org_id: Uuid,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET provider_responded_at = now(),
                        updated_at = now()
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .execute(pg)
                .await?;
                Ok(())
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE requests
                    SET provider_responded_at = datetime('now'),
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
                Ok(())
            }
        }
    }

    /// Get a request by ID (with org isolation).
    pub async fn get_by_id(
        &self,
        org_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<Request>, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, Request>(
                    r#"
                    SELECT
                        id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                        routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                        model_routed, request_headers, request_body, request_body_truncated,
                        requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                        completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                        prompt_tokens, completion_tokens, total_tokens,
                        input_cost, output_cost, total_cost,
                        status::text, status_code, error_code, error_message, metadata,
                        cache_hit, cache_key_hash,
                        created_at, updated_at, deleted_at
                    FROM requests
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .fetch_optional(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, Request>(
                    r#"
                    SELECT
                        id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                        routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                        model_routed, request_headers, request_body, request_body_truncated,
                        requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                        completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                        prompt_tokens, completion_tokens, total_tokens,
                        input_cost, output_cost, total_cost,
                        status, status_code, error_code, error_message, metadata,
                        cache_hit, cache_key_hash,
                        created_at, updated_at, deleted_at
                    FROM requests
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(request_id)
                .bind(org_id)
                .fetch_optional(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// Aggregate request stats for a time range.
    pub async fn aggregate_stats(
        &self,
        org_id: Uuid,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<RequestStats, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, RequestStats>(
                    r#"
                    SELECT
                        COALESCE(COUNT(*), 0) as total_requests,
                        COALESCE(SUM(total_cost), 0)::double precision as total_cost,
                        COALESCE(AVG(latency_total_ms), 0)::double precision as avg_latency_ms,
                        COALESCE(SUM(CASE WHEN cache_hit = true THEN 1 ELSE 0 END), 0) as cache_hits,
                        COALESCE(SUM(CASE WHEN cache_hit = false THEN 1 ELSE 0 END), 0) as cache_misses
                    FROM requests
                    WHERE org_id = $1
                      AND gateway_received_at >= $2
                      AND gateway_received_at < $3
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .bind(end)
                .fetch_one(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query(
                    r#"
                    SELECT
                        COALESCE(COUNT(*), 0) as total_requests,
                        COALESCE(SUM(total_cost), 0) as total_cost,
                        COALESCE(AVG(latency_total_ms), 0) as avg_latency_ms,
                        COALESCE(SUM(CASE WHEN cache_hit = true THEN 1 ELSE 0 END), 0) as cache_hits,
                        COALESCE(SUM(CASE WHEN cache_hit = false THEN 1 ELSE 0 END), 0) as cache_misses
                    FROM requests
                    WHERE org_id = $1
                      AND gateway_received_at >= $2
                      AND gateway_received_at < $3
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .bind(end)
                .fetch_one(sqlite)
                .await?;
                Ok(RequestStats {
                    total_requests: row.try_get("total_requests").unwrap_or(0),
                    total_cost: row.try_get::<f64, _>("total_cost").unwrap_or(0.0),
                    avg_latency_ms: row.try_get::<f64, _>("avg_latency_ms").unwrap_or(0.0),
                    cache_hits: row.try_get("cache_hits").unwrap_or(0),
                    cache_misses: row.try_get("cache_misses").unwrap_or(0),
                })
            }
        }
    }

    /// List recent requests for an org.
    pub async fn list_recent(&self, org_id: Uuid, limit: i64) -> Result<Vec<Request>, DbError> {
        let sql_pg = r#"
            SELECT
                id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                model_routed, request_headers, request_body, request_body_truncated,
                requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                prompt_tokens, completion_tokens, total_tokens,
                input_cost, output_cost, total_cost,
                status::text, status_code, error_code, error_message, metadata,
                cache_hit, cache_key_hash,
                created_at, updated_at, deleted_at
            FROM requests
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY gateway_received_at DESC
            LIMIT $2
            "#;
        let sql_sqlite = r#"
            SELECT
                id, org_id, api_key_id, user_id, provider_config_id, provider_model_id,
                routing_rule_id, trace_id, parent_trace_id, method, path, model_requested,
                model_routed, request_headers, request_body, request_body_truncated,
                requested_at, gateway_received_at, provider_sent_at, provider_responded_at,
                completed_at, latency_gateway_ms, latency_provider_ms, latency_total_ms,
                prompt_tokens, completion_tokens, total_tokens,
                input_cost, output_cost, total_cost,
                status, status_code, error_code, error_message, metadata,
                cache_hit, cache_key_hash,
                created_at, updated_at, deleted_at
            FROM requests
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY gateway_received_at DESC
            LIMIT $2
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, Request>(sql_pg)
                    .bind(org_id)
                    .bind(limit)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, Request>(sql_sqlite)
                    .bind(org_id)
                    .bind(limit)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }
}

/// Aggregated request stats.
#[derive(Debug, sqlx::FromRow)]
pub struct RequestStats {
    pub total_requests: i64,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
    pub cache_hits: i64,
    pub cache_misses: i64,
}
