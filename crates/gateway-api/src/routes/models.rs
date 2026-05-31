//! Model listing endpoints.

use axum::{extract::State, http::HeaderMap, Extension, Json};
use gateway_auth::AuthContext;
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

/// List available models (OpenAI-compatible).
///
/// In production, queries the `provider_models` table via `ModelRegistry`.
/// Falls back to a static list if the DB is unreachable.
pub async fn list_models(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    _headers: HeaderMap,
) -> Json<ModelListResponse> {
    let registry = ModelRegistry::new(state.db_pool);

    match registry.list_models(auth.org_id).await {
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
            // Static fallback for development / DB unavailable
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
