//! Webhook delivery repository — tracks delivery attempts and outcomes.

use crate::error::DbError;
use crate::models::WebhookDelivery;
use crate::pool::DbBackend;
use tracing::debug;
use uuid::Uuid;

/// Repository for webhook delivery records.
#[derive(Clone)]
pub struct WebhookDeliveryRepo {
    pool: DbBackend,
}

impl WebhookDeliveryRepo {
    /// Create a new delivery repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Record a new delivery attempt.
    pub async fn create_delivery(
        &self,
        org_id: Uuid,
        webhook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        attempt_number: i32,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<WebhookDelivery, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    INSERT INTO webhook_deliveries (
                        org_id, webhook_id, event_type, payload,
                        attempt_number, status, scheduled_at
                    )
                    VALUES ($1, $2, $3, $4, $5, 'pending', $6)
                    RETURNING
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(webhook_id)
                .bind(event_type)
                .bind(payload)
                .bind(attempt_number)
                .bind(scheduled_at)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    INSERT INTO webhook_deliveries (
                        org_id, webhook_id, event_type, payload,
                        attempt_number, status, scheduled_at
                    )
                    VALUES ($1, $2, $3, $4, $5, 'pending', $6)
                    RETURNING
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(webhook_id)
                .bind(event_type)
                .bind(payload)
                .bind(attempt_number)
                .bind(scheduled_at)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(org_id = %org_id, webhook_id = %webhook_id, delivery_id = %row.id, "Created webhook delivery");
        Ok(row)
    }

    /// Update delivery status after an attempt.
    pub async fn update_delivery_status(
        &self,
        delivery_id: Uuid,
        status: &str,
        request_headers: &serde_json::Value,
        request_body: Option<&str>,
        response_status: Option<i32>,
        response_body: Option<&str>,
        response_headers: &serde_json::Value,
        error_message: Option<&str>,
    ) -> Result<WebhookDelivery, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    UPDATE webhook_deliveries
                    SET status = $2,
                        request_headers = $3,
                        request_body = $4,
                        response_status = $5,
                        response_body = $6,
                        response_headers = $7,
                        error_message = $8,
                        completed_at = now(),
                        updated_at = now()
                    WHERE id = $1
                    RETURNING
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(delivery_id)
                .bind(status)
                .bind(request_headers)
                .bind(request_body)
                .bind(response_status)
                .bind(response_body)
                .bind(response_headers)
                .bind(error_message)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    UPDATE webhook_deliveries
                    SET status = $2,
                        request_headers = $3,
                        request_body = $4,
                        response_status = $5,
                        response_body = $6,
                        response_headers = $7,
                        error_message = $8,
                        completed_at = datetime('now'),
                        updated_at = datetime('now')
                    WHERE id = $1
                    RETURNING
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(delivery_id)
                .bind(status)
                .bind(request_headers)
                .bind(request_body)
                .bind(response_status)
                .bind(response_body)
                .bind(response_headers)
                .bind(error_message)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(delivery_id = %delivery_id, status = %status, "Updated webhook delivery status");
        Ok(row)
    }

    /// List deliveries for a webhook.
    pub async fn list_by_webhook(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDelivery>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    SELECT
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    FROM webhook_deliveries
                    WHERE webhook_id = $1 AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    LIMIT $2
                    "#,
                )
                .bind(webhook_id)
                .bind(limit)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    SELECT
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    FROM webhook_deliveries
                    WHERE webhook_id = $1 AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    LIMIT $2
                    "#,
                )
                .bind(webhook_id)
                .bind(limit)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }

    /// Get a single delivery by ID.
    pub async fn get_by_id(&self, delivery_id: Uuid) -> Result<Option<WebhookDelivery>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    SELECT
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    FROM webhook_deliveries
                    WHERE id = $1 AND deleted_at IS NULL
                    "#,
                )
                .bind(delivery_id)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, WebhookDelivery>(
                    r#"
                    SELECT
                        id, org_id, webhook_id, event_type, payload,
                        attempt_number,
                        request_headers, request_body, response_status,
                        response_body, response_headers,
                        status, error_message,
                        scheduled_at, started_at, completed_at,
                        created_at, updated_at, deleted_at
                    FROM webhook_deliveries
                    WHERE id = $1 AND deleted_at IS NULL
                    "#,
                )
                .bind(delivery_id)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_delivery_struct_defaults() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event_type: "request.completed".to_string(),
            payload: serde_json::json!({"id": "req-123"}),
            attempt_number: 1,
            request_headers: serde_json::json!({"Content-Type": "application/json"}),
            request_body: Some(r#"{"event":"request.completed"}"#.to_string()),
            response_status: Some(200),
            response_body: Some("ok".to_string()),
            response_headers: serde_json::json!({}),
            status: "delivered".to_string(),
            error_message: None,
            scheduled_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            completed_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(delivery.event_type, "request.completed");
        assert_eq!(delivery.status, "delivered");
        assert_eq!(delivery.attempt_number, 1);
        assert!(delivery.error_message.is_none());
    }

    #[test]
    fn test_webhook_delivery_failed_state() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event_type: "request.failed".to_string(),
            payload: serde_json::json!({"error": "timeout"}),
            attempt_number: 3,
            request_headers: serde_json::json!({}),
            request_body: None,
            response_status: None,
            response_body: None,
            response_headers: serde_json::json!({}),
            status: "failed".to_string(),
            error_message: Some("Connection timeout".to_string()),
            scheduled_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            completed_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(delivery.status, "failed");
        assert_eq!(delivery.attempt_number, 3);
        assert_eq!(
            delivery.error_message,
            Some("Connection timeout".to_string())
        );
    }

    #[test]
    fn test_webhook_delivery_serde_roundtrip() {
        let delivery = WebhookDelivery {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            webhook_id: Uuid::new_v4(),
            event_type: "quota.warning".to_string(),
            payload: serde_json::json!({"threshold": 0.8}),
            attempt_number: 1,
            request_headers: serde_json::json!({}),
            request_body: None,
            response_status: Some(200),
            response_body: None,
            response_headers: serde_json::json!({}),
            status: "delivered".to_string(),
            error_message: None,
            scheduled_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let json = serde_json::to_string(&delivery).expect("serialize");
        let decoded: WebhookDelivery = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.event_type, delivery.event_type);
        assert_eq!(decoded.status, delivery.status);
        assert_eq!(decoded.attempt_number, delivery.attempt_number);
    }
}
