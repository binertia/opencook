//! Model listing endpoints.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use gateway_auth::AuthContext;
use gateway_db::ModelRegistry;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{error::ApiError, extractors::ValidatedJson, state::AppState};

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

/// Get a single model by ID (OpenAI-compatible).
pub async fn get_model(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelInfo>, ApiError> {
    let registry = ModelRegistry::new(state.db_pool);

    match registry.list_models(auth.org_id).await {
        Ok(entries) => {
            for e in entries {
                if e.model_id == model_id {
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

                    return Ok(Json(ModelInfo {
                        id: e.model_id,
                        object: "model".to_string(),
                        created: chrono::Utc::now().timestamp(),
                        owned_by: e.provider_name.clone(),
                        gateway: Some(ModelGatewayMeta {
                            provider_id: e.provider_name,
                            capabilities,
                        }),
                    }));
                }
            }
            // Check static fallback
            for m in static_fallback_models() {
                if m.id == model_id {
                    return Ok(Json(m));
                }
            }
            Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "model_not_found",
                "Model not found",
            ))
        }
        Err(_) => {
            for m in static_fallback_models() {
                if m.id == model_id {
                    return Ok(Json(m));
                }
            }
            Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "model_not_found",
                "Model not found",
            ))
        }
    }
}

fn validate_non_negative_decimal(value: &Decimal) -> Result<(), validator::ValidationError> {
    if *value < Decimal::ZERO {
        let mut err = validator::ValidationError::new("negative_value");
        err.message = Some("Value must be non-negative".into());
        return Err(err);
    }
    Ok(())
}

/// Pricing update request body.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePricingRequest {
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub input_cost_per_1k: Decimal,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub output_cost_per_1k: Decimal,
}

/// Update pricing for a specific model on a provider.
///
/// Requires admin privileges (Owner or Admin).
pub async fn update_pricing(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((provider_id, model_id)): Path<(Uuid, String)>,
    ValidatedJson(body): ValidatedJson<UpdatePricingRequest>,
) -> Result<StatusCode, ApiError> {
    // Only owners and admins can update pricing
    let role = auth.role.as_deref().unwrap_or("viewer");
    if !matches!(role, "owner" | "admin") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            "Only organization owners and admins can update model pricing",
        ));
    }

    let registry = ModelRegistry::new(state.db_pool);
    match registry
        .update_pricing(
            auth.org_id,
            provider_id,
            &model_id,
            body.input_cost_per_1k,
            body.output_cost_per_1k,
        )
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(gateway_db::DbError::NotFound(_)) => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "model_not_found",
            "Model not found for this provider",
        )),
        Err(gateway_db::DbError::Unsupported(msg)) => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_pricing",
            &msg,
        )),
        Err(e) => {
            tracing::error!(error = %e, "Failed to update pricing");
            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "pricing_update_failed",
                "Failed to update model pricing",
            ))
        }
    }
}

fn static_fallback_models() -> Vec<ModelInfo> {
    vec![
        // OpenAI
        ModelInfo {
            id: "gpt-5.5".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gpt-5.5-mini".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gpt-5".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
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
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gpt-4.5-preview".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "o1".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "reasoning".to_string()],
            }),
        },
        ModelInfo {
            id: "o3-mini".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["chat".to_string(), "reasoning".to_string()],
            }),
        },
        ModelInfo {
            id: "text-embedding-3-small".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "openai".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "openai".to_string(),
                capabilities: vec!["embeddings".to_string()],
            }),
        },
        // Anthropic
        ModelInfo {
            id: "claude-4.8-sonnet".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "claude-4.5-sonnet".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "claude-3-7-sonnet-20250219".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
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
            id: "claude-3-5-haiku-20241022".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "claude-3-opus-20240229".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "anthropic".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "anthropic".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        // Google
        ModelInfo {
            id: "gemini-2.5-flash".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "google".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "gemini".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gemini-2.5-pro".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "google".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "gemini".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "gemini-2.0-flash".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "google".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "gemini".to_string(),
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
        // Local / Ollama
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
        ModelInfo {
            id: "llama3.3".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "ollama".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "ollama".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "qwen2.5".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "ollama".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "ollama".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "deepseek-r1".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "ollama".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "ollama".to_string(),
                capabilities: vec!["chat".to_string(), "reasoning".to_string()],
            }),
        },
        // Chinese providers
        ModelInfo {
            id: "qwen-max".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "qwen".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "qwen".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
        ModelInfo {
            id: "qwen-plus".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "qwen".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "qwen".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "moonshot-v1-8k".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "kimi".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "kimi".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "hunyuan-lite".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "tencent".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "tencent".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        // Additional providers
        ModelInfo {
            id: "llama-3.1-70b-versatile".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "groq".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "groq".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "mistral-large-latest".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "mistral".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "mistral".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "command-r".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "cohere".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "cohere".to_string(),
                capabilities: vec!["chat".to_string()],
            }),
        },
        ModelInfo {
            id: "gpt-4o".to_string(),
            object: "model".to_string(),
            created: 1715000000,
            owned_by: "azure".to_string(),
            gateway: Some(ModelGatewayMeta {
                provider_id: "azure".to_string(),
                capabilities: vec!["chat".to_string(), "vision".to_string()],
            }),
        },
    ]
}
