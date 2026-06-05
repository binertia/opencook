//! Webhook repository — CRUD operations for webhook configurations.

use crate::error::DbError;
use crate::models::Webhook;
use crate::pool::DbBackend;
use tracing::{debug, warn};
use uuid::Uuid;

/// Repository for webhook configurations.
#[derive(Clone)]
pub struct WebhookRepo {
    pool: DbBackend,
}

impl WebhookRepo {
    /// Create a new webhook repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Create a new webhook.
    pub async fn create(
        &self,
        org_id: Uuid,
        name: &str,
        url: &str,
        secret_enc: Option<&[u8]>,
        events: &[String],
        custom_headers: &serde_json::Value,
        max_retries: i32,
        retry_interval_seconds: i32,
        timeout_seconds: i32,
    ) -> Result<Webhook, DbError> {
        let events_json = serde_json::to_value(events).unwrap_or_default();
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    INSERT INTO webhooks (
                        org_id, name, url, secret_enc,
                        events, custom_headers, max_retries,
                        retry_interval_seconds, timeout_seconds, status
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active')
                    RETURNING
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(url)
                .bind(secret_enc)
                .bind(&events_json)
                .bind(custom_headers)
                .bind(max_retries)
                .bind(retry_interval_seconds)
                .bind(timeout_seconds)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    INSERT INTO webhooks (
                        org_id, name, url, secret_enc,
                        events, custom_headers, max_retries,
                        retry_interval_seconds, timeout_seconds, status
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active')
                    RETURNING
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(url)
                .bind(secret_enc)
                .bind(&events_json)
                .bind(custom_headers)
                .bind(max_retries)
                .bind(retry_interval_seconds)
                .bind(timeout_seconds)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(org_id = %org_id, webhook_id = %row.id, "Created webhook");
        Ok(row)
    }

    /// List all webhooks for an organization.
    pub async fn list_by_org(&self, org_id: Uuid) -> Result<Vec<Webhook>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    SELECT
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    FROM webhooks
                    WHERE org_id = $1 AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    "#,
                )
                .bind(org_id)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    SELECT
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    FROM webhooks
                    WHERE org_id = $1 AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    "#,
                )
                .bind(org_id)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }

    /// Get a single webhook by ID.
    pub async fn get_by_id(
        &self,
        org_id: Uuid,
        webhook_id: Uuid,
    ) -> Result<Option<Webhook>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    SELECT
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    FROM webhooks
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    SELECT
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    FROM webhooks
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Update a webhook (partial update supported via Option fields).
    pub async fn update(
        &self,
        org_id: Uuid,
        webhook_id: Uuid,
        name: Option<&str>,
        url: Option<&str>,
        secret_enc: Option<Option<&[u8]>>,
        events: Option<&[String]>,
        custom_headers: Option<&serde_json::Value>,
        max_retries: Option<i32>,
        retry_interval_seconds: Option<i32>,
        timeout_seconds: Option<i32>,
        status: Option<&str>,
    ) -> Result<Webhook, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    UPDATE webhooks
                    SET name = COALESCE($3, name),
                        url = COALESCE($4, url),
                        secret_enc = COALESCE($5, secret_enc),
                        events = COALESCE($6, events),
                        custom_headers = COALESCE($7, custom_headers),
                        max_retries = COALESCE($8, max_retries),
                        retry_interval_seconds = COALESCE($9, retry_interval_seconds),
                        timeout_seconds = COALESCE($10, timeout_seconds),
                        status = COALESCE($11, status),
                        updated_at = now()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .bind(name)
                .bind(url)
                .bind(secret_enc)
                .bind(events.map(|e| serde_json::to_value(e).unwrap_or_default()))
                .bind(custom_headers)
                .bind(max_retries)
                .bind(retry_interval_seconds)
                .bind(timeout_seconds)
                .bind(status)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Webhook>(
                    r#"
                    UPDATE webhooks
                    SET name = COALESCE($3, name),
                        url = COALESCE($4, url),
                        secret_enc = COALESCE($5, secret_enc),
                        events = COALESCE($6, events),
                        custom_headers = COALESCE($7, custom_headers),
                        max_retries = COALESCE($8, max_retries),
                        retry_interval_seconds = COALESCE($9, retry_interval_seconds),
                        timeout_seconds = COALESCE($10, timeout_seconds),
                        status = COALESCE($11, status),
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING
                        id, org_id, name, url, secret_enc,
                        events as "events: _",
                        custom_headers,
                        max_retries, retry_interval_seconds, timeout_seconds,
                        status,
                        last_delivered_at, last_failure_at, consecutive_failures,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .bind(name)
                .bind(url)
                .bind(secret_enc)
                .bind(events.map(|e| serde_json::to_value(e).unwrap_or_default()))
                .bind(custom_headers)
                .bind(max_retries)
                .bind(retry_interval_seconds)
                .bind(timeout_seconds)
                .bind(status)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(org_id = %org_id, webhook_id = %webhook_id, "Updated webhook");
        Ok(row)
    }

    /// Soft delete a webhook.
    pub async fn delete(&self, org_id: Uuid, webhook_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let result = sqlx::query(
                    r#"
                    UPDATE webhooks
                    SET deleted_at = now(), updated_at = now()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .execute(pg)
                .await?;

                if result.rows_affected() == 0 {
                    warn!(org_id = %org_id, webhook_id = %webhook_id, "Webhook not found for deletion");
                    return Err(DbError::NotFound(format!(
                        "Webhook {} not found",
                        webhook_id
                    )));
                }
            }
            DbBackend::Sqlite(sqlite) => {
                let result = sqlx::query(
                    r#"
                    UPDATE webhooks
                    SET deleted_at = datetime('now'), updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(webhook_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;

                if result.rows_affected() == 0 {
                    warn!(org_id = %org_id, webhook_id = %webhook_id, "Webhook not found for deletion");
                    return Err(DbError::NotFound(format!(
                        "Webhook {} not found",
                        webhook_id
                    )));
                }
            }
        };

        debug!(org_id = %org_id, webhook_id = %webhook_id, "Deleted webhook");
        Ok(())
    }

    /// Update delivery status after a delivery attempt.
    pub async fn record_delivery_result(
        &self,
        org_id: Uuid,
        webhook_id: Uuid,
        success: bool,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                if success {
                    sqlx::query(
                        r#"
                        UPDATE webhooks
                        SET last_delivered_at = now(),
                            consecutive_failures = 0,
                            updated_at = now()
                        WHERE id = $1 AND org_id = $2
                        "#,
                    )
                    .bind(webhook_id)
                    .bind(org_id)
                    .execute(pg)
                    .await?;
                } else {
                    sqlx::query(
                        r#"
                        UPDATE webhooks
                        SET last_failure_at = now(),
                            consecutive_failures = consecutive_failures + 1,
                            updated_at = now()
                        WHERE id = $1 AND org_id = $2
                        "#,
                    )
                    .bind(webhook_id)
                    .bind(org_id)
                    .execute(pg)
                    .await?;
                }
            }
            DbBackend::Sqlite(sqlite) => {
                if success {
                    sqlx::query(
                        r#"
                        UPDATE webhooks
                        SET last_delivered_at = datetime('now'),
                            consecutive_failures = 0,
                            updated_at = datetime('now')
                        WHERE id = $1 AND org_id = $2
                        "#,
                    )
                    .bind(webhook_id)
                    .bind(org_id)
                    .execute(sqlite)
                    .await?;
                } else {
                    sqlx::query(
                        r#"
                        UPDATE webhooks
                        SET last_failure_at = datetime('now'),
                            consecutive_failures = consecutive_failures + 1,
                            updated_at = datetime('now')
                        WHERE id = $1 AND org_id = $2
                        "#,
                    )
                    .bind(webhook_id)
                    .bind(org_id)
                    .execute(sqlite)
                    .await?;
                }
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_struct_defaults() {
        let webhook = Webhook {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "Test Webhook".to_string(),
            url: "https://example.com/webhook".to_string(),
            secret_enc: None,
            events: crate::types::JsonVec(vec!["request.completed".to_string()]),
            custom_headers: serde_json::json!({"X-Custom": "header"}),
            max_retries: 3,
            retry_interval_seconds: 60,
            timeout_seconds: 30,
            status: "active".to_string(),
            last_delivered_at: None,
            last_failure_at: None,
            consecutive_failures: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(webhook.name, "Test Webhook");
        assert_eq!(webhook.status, "active");
        assert_eq!(webhook.max_retries, 3);
        assert!(webhook.secret_enc.is_none());
    }

    #[test]
    fn test_webhook_struct_with_secret() {
        let secret = b"encrypted_secret_data".to_vec();
        let webhook = Webhook {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "Secure Webhook".to_string(),
            url: "https://hooks.example.com/endpoint".to_string(),
            secret_enc: Some(secret.clone()),
            events: crate::types::JsonVec(vec![
                "request.completed".to_string(),
                "request.failed".to_string(),
            ]),
            custom_headers: serde_json::json!({}),
            max_retries: 5,
            retry_interval_seconds: 300,
            timeout_seconds: 60,
            status: "inactive".to_string(),
            last_delivered_at: None,
            last_failure_at: None,
            consecutive_failures: 2,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(webhook.secret_enc, Some(secret));
        assert_eq!(webhook.events.0.len(), 2);
        assert_eq!(webhook.consecutive_failures, 2);
    }

    #[test]
    fn test_webhook_struct_serde_roundtrip() {
        let webhook = Webhook {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "Serde Test".to_string(),
            url: "https://example.com/hook".to_string(),
            secret_enc: None,
            events: crate::types::JsonVec(vec!["quota.exceeded".to_string()]),
            custom_headers: serde_json::json!({"Authorization": "Bearer token"}),
            max_retries: 3,
            retry_interval_seconds: 60,
            timeout_seconds: 30,
            status: "active".to_string(),
            last_delivered_at: None,
            last_failure_at: None,
            consecutive_failures: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let json = serde_json::to_string(&webhook).expect("serialize");
        let decoded: Webhook = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.name, webhook.name);
        assert_eq!(decoded.url, webhook.url);
        assert_eq!(decoded.status, webhook.status);
    }
}
