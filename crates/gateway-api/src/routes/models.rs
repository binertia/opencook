//! Model listing endpoints.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<ModelGatewayMeta>,
}

#[derive(Serialize)]
pub struct ModelGatewayMeta {
    pub provider_id: String,
    pub capabilities: Vec<String>,
}

/// List available models (OpenAI-compatible).
pub async fn list_models(State(_state): State<AppState>) -> Json<ModelListResponse> {
    // Stub: return a static list. In production, query provider_models table.
    Json(ModelListResponse {
        object: "list".to_string(),
        data: vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                object: "model".to_string(),
                created: 1715000000,
                owned_by: "openai".to_string(),
                gateway: Some(ModelGatewayMeta {
                    provider_id: "openai".to_string(),
                    capabilities: vec!["chat".to_string(), "vision".to_string()],
                }),
            },
            ModelInfo {
                id: "gpt-4o-mini".to_string(),
                object: "model".to_string(),
                created: 1715000000,
                owned_by: "openai".to_string(),
                gateway: Some(ModelGatewayMeta {
                    provider_id: "openai".to_string(),
                    capabilities: vec!["chat".to_string()],
                }),
            },
        ],
    })
}
