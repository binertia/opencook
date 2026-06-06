//! Webhook event publisher and delivery worker.
//!
//! Events are queued via an async channel and processed by a background
//! worker that delivers signed HTTP POSTs with exponential backoff retries.

use gateway_auth::crypto::hmac_sha256_hex;
use gateway_db::{
    pool::DbBackend,
    repos::{webhook_delivery_repo::WebhookDeliveryRepo, webhook_repo::WebhookRepo},
    Webhook, WebhookEvent,
};
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Internal queue capacity.
const QUEUE_CAPACITY: usize = 1000;
/// Max concurrent deliveries per organization.
#[allow(dead_code)]
const MAX_CONCURRENT_PER_ORG: usize = 5;
/// Truncate response body preview at this many bytes.
const RESPONSE_PREVIEW_LEN: usize = 200;
/// Truncate request payload at this many bytes for the delivery log.
const PAYLOAD_LOG_LEN: usize = 1024;

/// A webhook event to be delivered.
#[derive(Debug, Clone)]
pub struct PendingWebhook {
    pub org_id: Uuid,
    pub event: WebhookEvent,
    pub data: serde_json::Value,
}

/// Publisher handle — cloneable, lightweight.
#[derive(Debug, Clone)]
pub struct WebhookPublisher {
    tx: mpsc::Sender<PendingWebhook>,
}

impl WebhookPublisher {
    /// Spawn a new publisher with a background worker.
    ///
    /// `master_key` is used to decrypt webhook signing secrets.
    pub fn new(db_pool: DbBackend, master_key: [u8; 32]) -> Self {
        let (tx, rx) = mpsc::channel::<PendingWebhook>(QUEUE_CAPACITY);
        let worker = DeliveryWorker::new(db_pool, master_key, rx);
        tokio::spawn(worker.run());
        Self { tx }
    }

    /// Queue a webhook event for delivery.
    ///
    /// Returns immediately; delivery happens asynchronously.
    pub async fn publish(&self, org_id: Uuid, event: WebhookEvent, data: serde_json::Value) {
        let pending = PendingWebhook {
            org_id,
            event,
            data,
        };
        if let Err(e) = self.tx.send(pending).await {
            warn!(error = %e, "Webhook queue full — dropping event");
        }
    }
}

// ── Background Worker ────────────────────────────────────────────────

struct DeliveryWorker {
    db_pool: DbBackend,
    master_key: [u8; 32],
    rx: mpsc::Receiver<PendingWebhook>,
    http: reqwest::Client,
}

impl DeliveryWorker {
    fn new(db_pool: DbBackend, master_key: [u8; 32], rx: mpsc::Receiver<PendingWebhook>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("AI-Gateway-Webhook/1.0")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            db_pool,
            master_key,
            rx,
            http,
        }
    }

    async fn run(mut self) {
        info!("Webhook delivery worker started");

        while let Some(pending) = self.rx.recv().await {
            let db_pool = self.db_pool.clone();
            let master_key = self.master_key;
            let http = self.http.clone();
            tokio::spawn(async move {
                DeliveryWorker::process_event_with_resources(db_pool, master_key, http, pending)
                    .await;
            });
        }

        info!("Webhook delivery worker shutting down");
    }

    async fn process_event_with_resources(
        db_pool: DbBackend,
        master_key: [u8; 32],
        http: reqwest::Client,
        pending: PendingWebhook,
    ) {
        let webhook_repo = WebhookRepo::new(db_pool.clone());
        let delivery_repo = WebhookDeliveryRepo::new(db_pool.clone());

        let webhooks = match webhook_repo.list_by_org(pending.org_id).await {
            Ok(w) => w,
            Err(e) => {
                error!(org_id = %pending.org_id, error = %e, "Failed to list webhooks");
                return;
            }
        };

        let event_str = pending.event.as_str();

        for webhook in webhooks {
            // Skip inactive webhooks
            if webhook.status != "active" {
                debug!(webhook_id = %webhook.id, "Skipping inactive webhook");
                continue;
            }

            // Skip webhooks not subscribed to this event
            if !webhook.events.0.iter().any(|e| e == event_str) {
                continue;
            }

            let payload = build_payload(event_str, &pending.data);
            let payload_json = serde_json::to_string(&payload).unwrap_or_default();

            // Create delivery record
            let scheduled_at = chrono::Utc::now();
            let delivery = match delivery_repo
                .create_delivery(
                    pending.org_id,
                    webhook.id,
                    event_str,
                    &payload,
                    1,
                    scheduled_at,
                )
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    error!(webhook_id = %webhook.id, error = %e, "Failed to create delivery record");
                    continue;
                }
            };

            // Attempt delivery with retries
            let result = Self::deliver_with_retries(
                &http,
                master_key,
                &webhook,
                &payload_json,
                &delivery_repo,
                &webhook_repo,
                delivery.id,
            )
            .await;

            match result {
                Ok(()) => {
                    info!(webhook_id = %webhook.id, delivery_id = %delivery.id, "Webhook delivered");
                }
                Err(e) => {
                    warn!(webhook_id = %webhook.id, delivery_id = %delivery.id, error = %e, "Webhook delivery failed after retries");
                }
            }
        }
    }

    /// Deliver a webhook with exponential backoff retries.
    async fn deliver_with_retries(
        http: &reqwest::Client,
        master_key: [u8; 32],
        webhook: &Webhook,
        payload_json: &str,
        delivery_repo: &WebhookDeliveryRepo,
        webhook_repo: &WebhookRepo,
        delivery_id: Uuid,
    ) -> Result<(), String> {
        let secret = match &webhook.secret_enc {
            Some(enc) => match gateway_auth::crypto::decrypt_with_keys(
                enc,
                &gateway_auth::ActiveKeyPair::new(master_key),
            ) {
                Ok(s) => s,
                Err(e) => {
                    return Err(format!("Failed to decrypt secret: {}", e));
                }
            },
            None => {
                return Err("Webhook has no signing secret".to_string());
            }
        };

        let max_retries = webhook.max_retries.max(1) as u32;
        let base_delay = webhook.retry_interval_seconds.max(1);

        for attempt in 1..=max_retries {
            let result = Self::attempt_delivery(
                http,
                webhook,
                payload_json,
                &secret,
                delivery_repo,
                delivery_id,
            )
            .await;

            match result {
                Ok(()) => {
                    // Record success on webhook
                    let _ = webhook_repo
                        .record_delivery_result(webhook.org_id, webhook.id, true)
                        .await;
                    return Ok(());
                }
                Err(e) => {
                    let is_last = attempt == max_retries;
                    warn!(
                        webhook_id = %webhook.id,
                        attempt,
                        max_retries,
                        error = %e,
                        last = is_last,
                        "Delivery attempt failed"
                    );

                    if is_last {
                        // Record failure on webhook
                        let _ = webhook_repo
                            .record_delivery_result(webhook.org_id, webhook.id, false)
                            .await;
                        return Err(format!("All {} attempts failed. Last: {}", max_retries, e));
                    }

                    // Exponential backoff: base_delay * 5^(attempt-1)
                    let delay_secs = base_delay.saturating_mul(5_i32.pow(attempt - 1));
                    let delay = Duration::from_secs(delay_secs as u64);
                    debug!(webhook_id = %webhook.id, attempt, delay_secs, "Retrying after backoff");
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err("Retry loop exited unexpectedly".to_string())
    }

    /// Single delivery attempt.
    async fn attempt_delivery(
        http: &reqwest::Client,
        webhook: &Webhook,
        payload_json: &str,
        secret: &str,
        delivery_repo: &WebhookDeliveryRepo,
        delivery_id: Uuid,
    ) -> Result<(), String> {
        let signature = hmac_sha256_hex(secret, payload_json.as_bytes())
            .map_err(|e| format!("Signature generation failed: {}", e))?;

        let mut request = http
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", &signature)
            .body(payload_json.to_string());

        // Add custom headers
        if let Some(headers) = webhook.custom_headers.as_object() {
            for (key, value) in headers {
                if let Some(val_str) = value.as_str() {
                    request = request.header(key, val_str);
                }
            }
        }

        let start = std::time::Instant::now();
        let response = request.send().await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let request_headers = json!({
            "Content-Type": "application/json",
            "X-Webhook-Signature": signature,
        });

        let payload_preview = if payload_json.len() > PAYLOAD_LOG_LEN {
            format!("{}... [truncated]", &payload_json[..PAYLOAD_LOG_LEN])
        } else {
            payload_json.to_string()
        };

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let body_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<unreadable body>".to_string());

                let response_preview = if body_text.len() > RESPONSE_PREVIEW_LEN {
                    format!("{}...", &body_text[..RESPONSE_PREVIEW_LEN])
                } else {
                    body_text.clone()
                };

                debug!(
                    webhook_id = %webhook.id,
                    status,
                    elapsed_ms,
                    "Webhook delivery attempt completed"
                );

                // Record attempt
                let status_str = if (200..300).contains(&status) {
                    "delivered"
                } else {
                    "failed"
                };
                let error_msg = if (200..300).contains(&status) {
                    None
                } else {
                    Some(format!("HTTP {}", status))
                };

                let _ = delivery_repo
                    .update_delivery_status(
                        delivery_id,
                        status_str,
                        &request_headers,
                        Some(&payload_preview),
                        Some(status),
                        Some(&response_preview),
                        &json!({}),
                        error_msg.as_deref(),
                    )
                    .await;

                if (200..300).contains(&status) {
                    Ok(())
                } else {
                    Err(format!("HTTP {}", status))
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                warn!(
                    webhook_id = %webhook.id,
                    error = %error_msg,
                    elapsed_ms,
                    "Webhook delivery attempt failed"
                );

                let _ = delivery_repo
                    .update_delivery_status(
                        delivery_id,
                        "failed",
                        &request_headers,
                        Some(&payload_preview),
                        None,
                        None,
                        &json!({}),
                        Some(&error_msg),
                    )
                    .await;

                Err(error_msg)
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn build_payload(event: &str, data: &serde_json::Value) -> serde_json::Value {
    json!({
        "event": event,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "data": data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_display() {
        assert_eq!(
            WebhookEvent::RequestCompleted.to_string(),
            "request.completed"
        );
        assert_eq!(WebhookEvent::RequestFailed.to_string(), "request.failed");
        assert_eq!(WebhookEvent::QuotaWarning.to_string(), "quota.warning");
        assert_eq!(WebhookEvent::QuotaExceeded.to_string(), "quota.exceeded");
        assert_eq!(WebhookEvent::ProviderError.to_string(), "provider.error");
        assert_eq!(
            WebhookEvent::ProviderRecovered.to_string(),
            "provider.recovered"
        );
    }

    #[test]
    fn test_build_payload_structure() {
        let data = json!({"id": "req-123", "model": "gpt-4"});
        let payload = build_payload("request.completed", &data);

        assert_eq!(payload["event"], "request.completed");
        assert_eq!(payload["data"]["id"], "req-123");
        assert!(payload["timestamp"].as_str().is_some());
    }

    #[test]
    fn test_pending_webhook_clone() {
        let p = PendingWebhook {
            org_id: Uuid::nil(),
            event: WebhookEvent::QuotaExceeded,
            data: json!({}),
        };
        let cloned = p.clone();
        assert_eq!(cloned.event, WebhookEvent::QuotaExceeded);
    }
}
