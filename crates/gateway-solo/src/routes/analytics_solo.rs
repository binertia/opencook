//! Solo-mode analytics stubs.

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct KeyUsageResponse {
    pub data: Vec<KeyUsageItem>,
}

#[derive(Serialize)]
pub struct KeyUsageItem {
    pub key_id: String,
    pub key_name: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
}

pub async fn get_key_usage() -> Json<KeyUsageResponse> {
    Json(KeyUsageResponse {
        data: vec![KeyUsageItem {
            key_id: "solo-key".to_string(),
            key_name: "Default Solo Key".to_string(),
            request_count: 0,
            total_tokens: 0,
            total_cost: 0.0,
        }],
    })
}
