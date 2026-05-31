//! Model listing endpoints.

use axum::{extract::State, http::HeaderMap, Json};
use gateway_db::ModelRegistry;
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

/// Default org ID for SOLO mode.
const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

/// List available models (OpenAI-compatible).
pub async fn list_models(
    State(state): State<AppState>,
    _headers: HeaderMap,
) -> Json<ModelListResponse> {
    let registry = ModelRegistry::new(state.db_pool);
    let org_id = uuid::Uuid::parse_str(DEFAULT_ORG_ID).expect("valid uuid");

    match registry.list_models(org_id).await {
        Ok(entries) => {
            let data: Vec<ModelInfo> = entries
                .into_iter()
                .map(|e| {
                    let mut capabilities = vec!["chat".to_string()];
                    if e.capabilities.vision {
                        capabilities.push("vision".to_string());
                    }
                    if e.capabilities.tools {
                        capabilities.push("tools".to_string());
                    }
                    if e.capabilities.json_mode {
                        capabilities.push("json_mode".to_string());
                    }

                    ModelInfo {
                        id: e.model_id,
                        object: "model".to_string(),
                        created: chrono::Utc::now().timestamp(),
                        owned_by: e.provider_name.clone(),
                        gateway: Some(ModelGatewayMeta {
                            provider_id: e.provider_name,
                            capabilities,
                        }),
                    }
                })
                .collect();

            Json(ModelListResponse {
                object: "list".to_string(),
                data,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to query model registry, returning static fallback");
            Json(ModelListResponse {
                object: "list".to_string(),
                data: static_fallback_models(),
            })
        }
    }
}

fn static_fallback_models() -> Vec<ModelInfo> {
    vec![
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
        ModelInfo {
            id: "claude-3-5-sonnet-20241022".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gemini-1.5-flash".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "google".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "gemini".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "llama3.2".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "ollama".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "ollama".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
    ]
}
