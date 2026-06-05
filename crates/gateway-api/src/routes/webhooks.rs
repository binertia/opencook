//! Webhook CRUD and delivery management routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::AuthContext;
use gateway_db::{
    models::AuditAction,
    repos::{
        webhook_delivery_repo::WebhookDeliveryRepo,
        webhook_repo::WebhookRepo,
    },
    Webhook,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    state::AppState,
    validation::sanitize_display_text,
};
use validator::Validate;

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WebhookItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub custom_headers: serde_json::Value,
    pub max_retries: i32,
    pub retry_interval_seconds: i32,
    pub timeout_seconds: i32,
    pub status: String,
    pub last_delivered_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub consecutive_failures: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookListResponse {
    pub data: Vec<WebhookItem>,
}

#[derive(Debug, Serialize)]
pub struct WebhookDetailResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub custom_headers: serde_json::Value,
    pub max_retries: i32,
    pub retry_interval_seconds: i32,
    pub timeout_seconds: i32,
    pub status: String,
    pub last_delivered_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub consecutive_failures: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateWebhookResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub max_retries: i32,
    pub retry_interval_seconds: i32,
    pub timeout_seconds: i32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWebhookRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    #[validate(url(message = "URL must be a valid URL"))]
    pub url: String,
    #[validate(length(min = 1, message = "At least one event is required"))]
    pub events: Vec<String>,
    #[serde(default = "default_custom_headers")]
    pub custom_headers: serde_json::Value,
    #[validate(range(min = 0, max = 20, message = "Max retries must be 0-20"))]
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
    #[validate(range(min = 1, max = 3600, message = "Retry interval must be 1-3600 seconds"))]
    #[serde(default = "default_retry_interval")]
    pub retry_interval_seconds: i32,
    #[validate(range(min = 1, max = 300, message = "Timeout must be 1-300 seconds"))]
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateWebhookRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: Option<String>,
    #[validate(url(message = "URL must be a valid URL"))]
    pub url: Option<String>,
    pub events: Option<Vec<String>>,
    pub custom_headers: Option<serde_json::Value>,
    #[validate(range(min = 0, max = 20, message = "Max retries must be 0-20"))]
    pub max_retries: Option<i32>,
    #[validate(range(min = 1, max = 3600, message = "Retry interval must be 1-3600 seconds"))]
    pub retry_interval_seconds: Option<i32>,
    #[validate(range(min = 1, max = 300, message = "Timeout must be 1-300 seconds"))]
    pub timeout_seconds: Option<i32>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookDeliveryItem {
    pub id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub attempt_number: i32,
    pub status: String,
    pub response_status: Option<i32>,
    pub error_message: Option<String>,
    pub scheduled_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub request_headers: serde_json::Value,
    pub request_body: Option<String>,
    pub response_headers: serde_json::Value,
    pub response_body: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookDeliveryListResponse {
    pub data: Vec<WebhookDeliveryItem>,
}

fn default_max_retries() -> i32 {
    3
}

fn default_retry_interval() -> i32 {
    60
}

fn default_timeout() -> i32 {
    30
}

fn default_custom_headers() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_webhooks(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<WebhookListResponse>, ApiError> {
    let repo = WebhookRepo::new(state.db_pool.clone());

    let webhooks = repo
        .list_by_org(auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(WebhookListResponse {
        data: webhooks.iter().map(db_to_item).collect(),
    }))
}

pub async fn create_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<CreateWebhookRequest>,
) -> Result<Json<CreateWebhookResponse>, ApiError> {
    let repo = WebhookRepo::new(state.db_pool.clone());

    // Generate a random signing secret
    let secret = gateway_auth::crypto::generate_webhook_secret();

    // Encrypt the secret with the master key
    let secret_enc = gateway_auth::crypto::encrypt(&secret, &state.config.master_key)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "encryption_error", e.to_string()))?;

    let name = sanitize_display_text(&body.name);
    let url = body.url.clone();
    let webhook = repo
        .create(
            auth.org_id,
            &name,
            &url,
            Some(&secret_enc),
            &body.events,
            &body.custom_headers,
            body.max_retries,
            body.retry_interval_seconds,
            body.timeout_seconds,
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::WebhookCreated,
        "webhook",
        Some(&webhook.id.to_string()),
        None,
        Some(json!({
            "name": webhook.name,
            "url": webhook.url,
            "events": body.events,
        })),
        "Webhook created",
    )
    .await;

    Ok(Json(CreateWebhookResponse {
        id: webhook.id.to_string(),
        name: webhook.name,
        url: webhook.url,
        secret,
        events: body.events,
        max_retries: webhook.max_retries,
        retry_interval_seconds: webhook.retry_interval_seconds,
        timeout_seconds: webhook.timeout_seconds,
        status: webhook.status,
        created_at: webhook.created_at.to_rfc3339(),
    }))
}

pub async fn get_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(webhook_id): Path<String>,
) -> Result<Json<WebhookDetailResponse>, ApiError> {
    let repo = WebhookRepo::new(state.db_pool.clone());

    let webhook_id = Uuid::parse_str(&webhook_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid webhook ID"))?;

    let webhook = repo
        .get_by_id(auth.org_id, webhook_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "Webhook not found"))?;

    Ok(Json(db_to_detail(webhook)))
}

pub async fn update_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(webhook_id): Path<String>,
    ValidatedJson(body): ValidatedJson<UpdateWebhookRequest>,
) -> Result<Json<WebhookDetailResponse>, ApiError> {
    let repo = WebhookRepo::new(state.db_pool.clone());

    let webhook_id = Uuid::parse_str(&webhook_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid webhook ID"))?;

    let existing = repo
        .get_by_id(auth.org_id, webhook_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "Webhook not found"))?;

    let name = body.name.as_deref().map(sanitize_display_text);
    let status = body.status.as_deref().map(sanitize_display_text);
    let webhook = repo
        .update(
            auth.org_id,
            webhook_id,
            name.as_deref(),
            body.url.as_deref(),
            None, // secret_enc not updated via this endpoint
            body.events.as_deref(),
            body.custom_headers.as_ref(),
            body.max_retries,
            body.retry_interval_seconds,
            body.timeout_seconds,
            status.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::Update,
        "webhook",
        Some(&webhook.id.to_string()),
        Some(json!({
            "name": existing.name,
            "url": existing.url,
            "events": existing.events,
            "status": existing.status,
        })),
        Some(json!({
            "name": webhook.name,
            "url": webhook.url,
            "events": webhook.events,
            "status": webhook.status,
        })),
        "Webhook updated",
    )
    .await;

    Ok(Json(db_to_detail(webhook)))
}

pub async fn delete_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(webhook_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repo = WebhookRepo::new(state.db_pool.clone());

    let webhook_id = Uuid::parse_str(&webhook_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid webhook ID"))?;

    let existing = repo
        .get_by_id(auth.org_id, webhook_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "Webhook not found"))?;

    repo
        .delete(auth.org_id, webhook_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::WebhookDeleted,
        "webhook",
        Some(&existing.id.to_string()),
        Some(json!({
            "name": existing.name,
            "url": existing.url,
            "events": existing.events,
            "status": existing.status,
        })),
        None,
        "Webhook deleted",
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_webhook_deliveries(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthContext>,
    Path(webhook_id): Path<String>,
) -> Result<Json<WebhookDeliveryListResponse>, ApiError> {
    let webhook_id = Uuid::parse_str(&webhook_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid webhook ID"))?;

    let delivery_repo = WebhookDeliveryRepo::new(state.db_pool.clone());

    let deliveries = delivery_repo
        .list_by_webhook(webhook_id, 50)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(WebhookDeliveryListResponse {
        data: deliveries.iter().map(db_to_delivery_item).collect(),
    }))
}

pub async fn retry_webhook_delivery(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((webhook_id, delivery_id)): Path<(String, String)>,
) -> Result<Json<WebhookDeliveryItem>, ApiError> {
    let webhook_id = Uuid::parse_str(&webhook_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid webhook ID"))?;
    let delivery_id = Uuid::parse_str(&delivery_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", "Invalid delivery ID"))?;

    let webhook_repo = WebhookRepo::new(state.db_pool.clone());
    let delivery_repo = WebhookDeliveryRepo::new(state.db_pool.clone());

    let webhook = webhook_repo
        .get_by_id(auth.org_id, webhook_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "Webhook not found"))?;

    let delivery = delivery_repo
        .get_by_id(delivery_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "Delivery not found"))?;

    // Decrypt secret
    let secret = match &webhook.secret_enc {
        Some(enc) => state.config.decrypt_master(enc)
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "decryption_error", e.to_string()))?,
        None => return Err(ApiError::new(StatusCode::BAD_REQUEST, "no_secret", "Webhook has no signing secret")),
    };

    // Build payload
    let payload_json = serde_json::to_string(&delivery.payload)
        .unwrap_or_default();

    let signature = gateway_auth::crypto::hmac_sha256_hex(&secret, payload_json.as_bytes())
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "signature_error", e.to_string()))?;

    // Send request
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(webhook.timeout_seconds as u64))
        .build()
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "http_client_error", e.to_string()))?;

    let mut request = client
        .post(&webhook.url)
        .header("Content-Type", "application/json")
        .header("X-Webhook-Signature", &signature)
        .body(payload_json.clone());

    if let Some(headers) = webhook.custom_headers.as_object() {
        for (key, value) in headers {
            if let Some(val_str) = value.as_str() {
                request = request.header(key, val_str);
            }
        }
    }

    let start = std::time::Instant::now();
    let response = request.send().await;
    let elapsed_ms = start.elapsed().as_millis() as i32;

    let request_headers = serde_json::json!({
        "Content-Type": "application/json",
        "X-Webhook-Signature": signature,
    });

    let (status_str, response_status, error_msg) = match response {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let body_text = resp.text().await.unwrap_or_else(|_| "<unreadable>".to_string());
            let preview = if body_text.len() > 200 { format!("{}...", &body_text[..200]) } else { body_text };

            if (200..300).contains(&status) {
                ("delivered", Some(status), None)
            } else {
                ("failed", Some(status), Some(format!("HTTP {}", status)))
            }
        }
        Err(e) => {
            ("failed", None, Some(e.to_string()))
        }
    };

    // Update delivery record
    let updated = delivery_repo
        .update_delivery_status(
            delivery_id,
            status_str,
            &request_headers,
            Some(&payload_json),
            response_status,
            None,
            &serde_json::json!({}),
            error_msg.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    // Update webhook result tracking
    let _ = webhook_repo
        .record_delivery_result(auth.org_id, webhook_id, status_str == "delivered")
        .await;

    Ok(Json(db_to_delivery_item(&updated)))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_to_item(webhook: &Webhook) -> WebhookItem {
    WebhookItem {
        id: webhook.id.to_string(),
        name: webhook.name.clone(),
        url: webhook.url.clone(),
        events: webhook.events.0.clone(),
        custom_headers: webhook.custom_headers.clone(),
        max_retries: webhook.max_retries,
        retry_interval_seconds: webhook.retry_interval_seconds,
        timeout_seconds: webhook.timeout_seconds,
        status: webhook.status.clone(),
        last_delivered_at: webhook.last_delivered_at.map(|t| t.to_rfc3339()),
        last_failure_at: webhook.last_failure_at.map(|t| t.to_rfc3339()),
        consecutive_failures: webhook.consecutive_failures,
        created_at: webhook.created_at.to_rfc3339(),
    }
}

fn db_to_detail(webhook: Webhook) -> WebhookDetailResponse {
    WebhookDetailResponse {
        id: webhook.id.to_string(),
        name: webhook.name,
        url: webhook.url,
        events: webhook.events.0,
        custom_headers: webhook.custom_headers,
        max_retries: webhook.max_retries,
        retry_interval_seconds: webhook.retry_interval_seconds,
        timeout_seconds: webhook.timeout_seconds,
        status: webhook.status,
        last_delivered_at: webhook.last_delivered_at.map(|t| t.to_rfc3339()),
        last_failure_at: webhook.last_failure_at.map(|t| t.to_rfc3339()),
        consecutive_failures: webhook.consecutive_failures,
        created_at: webhook.created_at.to_rfc3339(),
    }
}

fn db_to_delivery_item(delivery: &gateway_db::WebhookDelivery) -> WebhookDeliveryItem {
    WebhookDeliveryItem {
        id: delivery.id.to_string(),
        event_type: delivery.event_type.clone(),
        payload: delivery.payload.clone(),
        attempt_number: delivery.attempt_number,
        status: delivery.status.clone(),
        response_status: delivery.response_status,
        error_message: delivery.error_message.clone(),
        scheduled_at: delivery.scheduled_at.to_rfc3339(),
        started_at: delivery.started_at.map(|t| t.to_rfc3339()),
        completed_at: delivery.completed_at.map(|t| t.to_rfc3339()),
        request_headers: delivery.request_headers.clone(),
        request_body: delivery.request_body.clone(),
        response_headers: delivery.response_headers.clone(),
        response_body: delivery.response_body.clone(),
        created_at: delivery.created_at.to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_webhook_request_defaults() {
        let json = r#"{"name":"Test","url":"https://example.com","events":["request.completed"]}"#;
        let req: CreateWebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Test");
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.events, vec!["request.completed"]);
        assert_eq!(req.max_retries, 3); // default
        assert_eq!(req.retry_interval_seconds, 60); // default
        assert_eq!(req.timeout_seconds, 30); // default
        assert_eq!(req.custom_headers, serde_json::Value::Object(Default::default()));
    }

    #[test]
    fn test_create_webhook_request_custom_values() {
        let json = r#"{
            "name":"Test","url":"https://example.com","events":["request.completed"],
            "max_retries":5,"retry_interval_seconds":300,"timeout_seconds":60,
            "custom_headers":{"X-Custom":"value"}
        }"#;
        let req: CreateWebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_retries, 5);
        assert_eq!(req.retry_interval_seconds, 300);
        assert_eq!(req.timeout_seconds, 60);
        assert_eq!(req.custom_headers, serde_json::json!({"X-Custom": "value"}));
    }

    #[test]
    fn test_create_webhook_request_empty_events() {
        let json = r#"{"name":"Test","url":"https://example.com","events":[]}"#;
        let req: CreateWebhookRequest = serde_json::from_str(json).unwrap();
        assert!(req.events.is_empty());
    }

    #[test]
    fn test_update_webhook_request_partial() {
        let json = r#"{"name":"Updated Name","status":"inactive"}"#;
        let req: UpdateWebhookRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Updated Name".to_string()));
        assert_eq!(req.status, Some("inactive".to_string()));
        assert!(req.url.is_none());
        assert!(req.events.is_none());
    }

    #[test]
    fn test_webhook_item_serialization() {
        let item = WebhookItem {
            id: "uuid-123".to_string(),
            name: "Test Hook".to_string(),
            url: "https://example.com".to_string(),
            events: vec!["request.completed".to_string()],
            custom_headers: serde_json::json!({}),
            max_retries: 3,
            retry_interval_seconds: 60,
            timeout_seconds: 30,
            status: "active".to_string(),
            last_delivered_at: None,
            last_failure_at: None,
            consecutive_failures: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("Test Hook"));
        assert!(json.contains("active"));
    }

    #[test]
    fn test_webhook_delivery_item_serialization() {
        let item = WebhookDeliveryItem {
            id: "uuid-456".to_string(),
            event_type: "request.failed".to_string(),
            payload: serde_json::json!({"error": "timeout"}),
            attempt_number: 2,
            status: "failed".to_string(),
            response_status: Some(500),
            error_message: Some("Timeout".to_string()),
            scheduled_at: "2024-01-01T00:00:00Z".to_string(),
            started_at: Some("2024-01-01T00:00:01Z".to_string()),
            completed_at: Some("2024-01-01T00:00:05Z".to_string()),
            request_headers: serde_json::json!({"Content-Type": "application/json"}),
            request_body: Some(r#"{"event":"request.failed"}"#.to_string()),
            response_headers: serde_json::json!({"Content-Type": "text/plain"}),
            response_body: Some("Internal Server Error".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("request.failed"));
        assert!(json.contains("Timeout"));
        assert!(json.contains("request_body"));
        assert!(json.contains("response_body"));
    }

    #[test]
    fn test_db_to_item_maps_correctly() {
        let webhook = gateway_db::Webhook {
            id: uuid::Uuid::nil(),
            org_id: uuid::Uuid::nil(),
            name: "Test".to_string(),
            url: "https://example.com".to_string(),
            secret_enc: None,
            events: gateway_db::types::JsonVec(vec!["e1".to_string()]),
            custom_headers: serde_json::json!({}),
            max_retries: 3,
            retry_interval_seconds: 60,
            timeout_seconds: 30,
            status: "active".to_string(),
            last_delivered_at: None,
            last_failure_at: None,
            consecutive_failures: 0,
            created_at: chrono::DateTime::UNIX_EPOCH,
            updated_at: chrono::DateTime::UNIX_EPOCH,
            deleted_at: None,
        };

        let item = db_to_item(&webhook);
        assert_eq!(item.name, "Test");
        assert_eq!(item.events, vec!["e1"]);
        assert_eq!(item.status, "active");
    }

    #[test]
    fn test_db_to_item_with_timestamps() {
        let now = chrono::Utc::now();
        let webhook = gateway_db::Webhook {
            id: uuid::Uuid::nil(),
            org_id: uuid::Uuid::nil(),
            name: "Test".to_string(),
            url: "https://example.com".to_string(),
            secret_enc: None,
            events: gateway_db::types::JsonVec(vec![]),
            custom_headers: serde_json::json!({}),
            max_retries: 3,
            retry_interval_seconds: 60,
            timeout_seconds: 30,
            status: "failing".to_string(),
            last_delivered_at: Some(now),
            last_failure_at: Some(now),
            consecutive_failures: 5,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let item = db_to_item(&webhook);
        assert_eq!(item.status, "failing");
        assert_eq!(item.consecutive_failures, 5);
        assert!(item.last_delivered_at.is_some());
        assert!(item.last_failure_at.is_some());
    }

    #[test]
    fn test_db_to_delivery_item_maps_correctly() {
        let delivery = gateway_db::WebhookDelivery {
            id: uuid::Uuid::nil(),
            org_id: uuid::Uuid::nil(),
            webhook_id: uuid::Uuid::nil(),
            event_type: "test.event".to_string(),
            payload: serde_json::json!({}),
            attempt_number: 1,
            request_headers: serde_json::json!({}),
            request_body: None,
            response_status: Some(200),
            response_body: None,
            response_headers: serde_json::json!({}),
            status: "delivered".to_string(),
            error_message: None,
            scheduled_at: chrono::DateTime::UNIX_EPOCH,
            started_at: None,
            completed_at: None,
            created_at: chrono::DateTime::UNIX_EPOCH,
            updated_at: chrono::DateTime::UNIX_EPOCH,
            deleted_at: None,
        };

        let item = db_to_delivery_item(&delivery);
        assert_eq!(item.event_type, "test.event");
        assert_eq!(item.status, "delivered");
        assert_eq!(item.response_status, Some(200));
    }
}
