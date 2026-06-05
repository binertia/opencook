//! Solo-mode webhook stubs — returns empty lists so the webhook pages
//! don't crash when running in SOLO mode.

use axum::{extract::Path, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct WebhookListResponse {
    pub data: Vec<WebhookItem>,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct WebhookDeliveryListResponse {
    pub data: Vec<WebhookDeliveryItem>,
}

#[derive(Serialize)]
pub struct WebhookDeliveryItem {
    pub id: String,
    pub event_type: String,
    pub attempt_number: i32,
    pub status: String,
    pub response_status: Option<i32>,
    pub error_message: Option<String>,
    pub scheduled_at: String,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Serialize)]
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

pub async fn list_webhooks() -> Json<WebhookListResponse> {
    Json(WebhookListResponse { data: vec![] })
}

pub async fn create_webhook(Json(req): Json<CreateWebhookRequest>) -> Json<CreateWebhookResponse> {
    Json(CreateWebhookResponse {
        id: "solo-wh-1".to_string(),
        name: req.name,
        url: req.url,
        secret: "sk_wh_solo_demo_secret_do_not_use".to_string(),
        events: req.events,
        max_retries: 3,
        retry_interval_seconds: 60,
        timeout_seconds: 30,
        status: "active".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub async fn get_webhook(Path(_id): Path<String>) -> Json<WebhookItem> {
    Json(WebhookItem {
        id: "solo-wh-1".to_string(),
        name: "Demo Webhook".to_string(),
        url: "https://example.com/webhook".to_string(),
        events: vec!["request.completed".to_string()],
        custom_headers: serde_json::Value::Object(Default::default()),
        max_retries: 3,
        retry_interval_seconds: 60,
        timeout_seconds: 30,
        status: "active".to_string(),
        last_delivered_at: None,
        last_failure_at: None,
        consecutive_failures: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub async fn update_webhook(Path(_id): Path<String>) -> Json<WebhookItem> {
    get_webhook(Path("solo-wh-1".to_string())).await
}

pub async fn delete_webhook(Path(_id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"success": true}))
}

pub async fn list_deliveries(Path(_id): Path<String>) -> Json<WebhookDeliveryListResponse> {
    Json(WebhookDeliveryListResponse { data: vec![] })
}

pub async fn retry_delivery(Path((_webhook_id, _delivery_id)): Path<(String, String)>) -> Json<WebhookDeliveryItem> {
    Json(WebhookDeliveryItem {
        id: "solo-dl-1".to_string(),
        event_type: "request.completed".to_string(),
        attempt_number: 1,
        status: "pending".to_string(),
        response_status: None,
        error_message: None,
        scheduled_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}
