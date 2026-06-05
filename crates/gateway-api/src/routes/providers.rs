//! Provider management routes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::AuthContext;
use gateway_db::{
    models::{AuditAction, ProviderConfig as DbProviderConfig},
    repos::provider_config_repo::ProviderConfigRepo,
    ModelRegistry,
};
use gateway_providers::factory::{ProviderConfig as FactoryProviderConfig, ProviderKind};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    state::AppState,
    validation::{sanitize_display_text, validate_provider_kind},
};
use validator::Validate;

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProviderRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    #[validate(length(min = 1, max = 32, message = "Kind must be 1-32 characters"))]
    pub kind: String,
    pub api_key: Option<String>,
    #[validate(url(message = "Base URL must be a valid URL"))]
    pub base_url: Option<String>,
    pub models: Option<Vec<String>>,
    #[validate(range(min = 10, max = 86400, message = "Health check interval must be 10-86400 seconds"))]
    pub health_check_interval_seconds: Option<u64>,
    #[validate(range(min = 1, max = 300, message = "Health check timeout must be 1-300 seconds"))]
    pub health_check_timeout_seconds: Option<u64>,
    pub health_check_model: Option<String>,
    #[validate(range(min = 0, max = 1000, message = "Weight must be 0-1000"))]
    pub weight: Option<i32>,
    #[validate(range(min = 0, max = 1000, message = "Priority must be 0-1000"))]
    pub priority: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProviderRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<Option<String>>,
    pub models: Option<Vec<String>>,
    #[validate(range(min = 10, max = 86400, message = "Health check interval must be 10-86400 seconds"))]
    pub health_check_interval_seconds: Option<u64>,
    #[validate(range(min = 1, max = 300, message = "Health check timeout must be 1-300 seconds"))]
    pub health_check_timeout_seconds: Option<u64>,
    pub health_check_model: Option<String>,
    #[validate(range(min = 0, max = 1000, message = "Weight must be 0-1000"))]
    pub weight: Option<i32>,
    #[validate(range(min = 0, max = 1000, message = "Priority must be 0-1000"))]
    pub priority: Option<i32>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderModelResponse {
    pub id: String,
    pub name: String,
    pub context_window: Option<i32>,
    pub capabilities: Vec<String>,
    pub pricing: Option<serde_json::Value>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderHealthResponse {
    pub provider_id: String,
    pub status: String,
    pub latency_ms: u64,
    pub error_rate: f64,
    pub last_checked: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderDetailResponse {
    #[serde(flatten)]
    pub provider: ProviderResponse,
    pub models: Vec<ProviderModelResponse>,
    pub health: Option<ProviderHealthResponse>,
    pub routing_weight: i32,
    pub priority: i32,
}

#[derive(Debug, Serialize)]
pub struct ListProvidersResponse {
    pub object: String,
    pub data: Vec<ProviderResponse>,
}

#[derive(Debug, Serialize)]
pub struct HealthHistoryEntry {
    pub checked_at: String,
    pub status: String,
    pub latency_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthHistoryResponse {
    pub object: String,
    pub data: Vec<HealthHistoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub latency_ms: u64,
    pub message: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_to_response(db: &DbProviderConfig) -> ProviderResponse {
    ProviderResponse {
        id: db.id.to_string(),
        name: db.name.clone(),
        kind: db.kind.clone(),
        base_url: db.api_base.clone().unwrap_or_default(),
        status: db.status.clone(),
        created_at: db.created_at.to_rfc3339(),
        updated_at: db.updated_at.to_rfc3339(),
    }
}

fn encrypt_api_key(api_key: &str, master_key: &[u8; 32]) -> Option<Vec<u8>> {
    if api_key.is_empty() {
        return Some(Vec::new());
    }
    gateway_auth::crypto::encrypt(api_key, master_key).ok()
}

fn decrypt_api_key(api_key_enc: &[u8], master_key: &[u8; 32]) -> Option<String> {
    if api_key_enc.is_empty() {
        return Some(String::new());
    }
    gateway_auth::crypto::decrypt_with_keys(api_key_enc, &gateway_auth::ActiveKeyPair::new(*master_key)).ok()
}

fn parse_provider_kind(kind: &str) -> Option<ProviderKind> {
    match kind.to_lowercase().as_str() {
        "openai" => Some(ProviderKind::OpenAi),
        "anthropic" => Some(ProviderKind::Anthropic),
        "gemini" => Some(ProviderKind::Gemini),
        "ollama" => Some(ProviderKind::Ollama),
        "qwen" | "alibaba" | "dashscope" => Some(ProviderKind::Qwen),
        "kimi" | "moonshot" => Some(ProviderKind::Kimi),
        "tencent" | "hunyuan" => Some(ProviderKind::Tencent),
        "groq" => Some(ProviderKind::Groq),
        "mistral" => Some(ProviderKind::Mistral),
        "cohere" => Some(ProviderKind::Cohere),
        "azure" => Some(ProviderKind::Azure),
        _ => None,
    }
}

fn default_base_url(kind: &ProviderKind) -> String {
    match kind {
        ProviderKind::OpenAi => "https://api.openai.com".to_string(),
        ProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
        ProviderKind::Gemini => "https://generativelanguage.googleapis.com".to_string(),
        ProviderKind::Ollama => "http://localhost:11434".to_string(),
        ProviderKind::Qwen => "https://dashscope.aliyuncs.com/compatible-mode".to_string(),
        ProviderKind::Kimi => "https://api.moonshot.cn".to_string(),
        ProviderKind::Tencent => "https://hunyuan.tencentcloudapi.com".to_string(),
        ProviderKind::Groq => "https://api.groq.com/openai".to_string(),
        ProviderKind::Mistral => "https://api.mistral.ai".to_string(),
        ProviderKind::Cohere => "https://api.cohere.ai/compatibility".to_string(),
        ProviderKind::Azure => "https://your-resource.openai.azure.com".to_string(),
        ProviderKind::Custom => String::new(),
    }
}

fn build_config_json(req: &CreateProviderRequest) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    if let Some(v) = req.health_check_interval_seconds {
        config.insert("health_check_interval_seconds".to_string(), json!(v));
    }
    if let Some(v) = req.health_check_timeout_seconds {
        config.insert("health_check_timeout_seconds".to_string(), json!(v));
    }
    if let Some(ref v) = req.health_check_model {
        config.insert("health_check_model".to_string(), json!(v));
    }
    if let Some(v) = req.weight {
        config.insert("weight".to_string(), json!(v));
    }
    json!(config)
}

fn merge_config_json(
    existing: &serde_json::Value,
    req: &UpdateProviderRequest,
) -> serde_json::Value {
    let mut config = if let serde_json::Value::Object(m) = existing {
        m.clone()
    } else {
        serde_json::Map::new()
    };
    if let Some(v) = req.health_check_interval_seconds {
        config.insert("health_check_interval_seconds".to_string(), json!(v));
    }
    if let Some(v) = req.health_check_timeout_seconds {
        config.insert("health_check_timeout_seconds".to_string(), json!(v));
    }
    if let Some(ref v) = req.health_check_model {
        config.insert("health_check_model".to_string(), json!(v));
    }
    if let Some(v) = req.weight {
        config.insert("weight".to_string(), json!(v));
    }
    json!(config)
}

async fn fetch_provider_health(
    redis: &redis::aio::ConnectionManager,
    provider_id: Uuid,
) -> Option<ProviderHealthResponse> {
    let mut conn = redis.clone();
    let key = format!("health:{}", provider_id);
    let value: String = conn.get(&key).await.ok()?;
    let result: serde_json::Value = serde_json::from_str(&value).ok()?;

    Some(ProviderHealthResponse {
        provider_id: provider_id.to_string(),
        status: if result["healthy"].as_bool()? {
            "healthy"
        } else {
            "unhealthy"
        }
        .to_string(),
        latency_ms: result["latency_ms"].as_u64()?,
        error_rate: 0.0,
        last_checked: result["checked_at"].as_str()?.to_string(),
        message: result["error"].as_str().map(|s| s.to_string()),
    })
}

async fn fetch_health_history(
    redis: &redis::aio::ConnectionManager,
    provider_id: Uuid,
    hours: u64,
) -> Vec<HealthHistoryEntry> {
    let mut conn = redis.clone();
    let history_key = format!("health_history:{}", provider_id);
    let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours as i64))
        .timestamp() as f64;

    let values: Result<Vec<String>, _> = redis::cmd("ZRANGEBYSCORE")
        .arg(&history_key)
        .arg(cutoff)
        .arg("+inf")
        .query_async(&mut conn)
        .await;

    match values {
        Ok(vals) => vals
            .into_iter()
            .filter_map(|v| {
                let result: serde_json::Value = serde_json::from_str(&v).ok()?;
                Some(HealthHistoryEntry {
                    checked_at: result["checked_at"].as_str()?.to_string(),
                    status: if result["healthy"].as_bool()? {
                        "healthy"
                    } else {
                        "unhealthy"
                    }
                    .to_string(),
                    latency_ms: result["latency_ms"].as_u64()?,
                    error: result["error"].as_str().map(|s| s.to_string()),
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_providers(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<ListProvidersResponse>, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let providers = repo
        .list_by_org(auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(ListProvidersResponse {
        object: "list".to_string(),
        data: providers.iter().map(|p| db_to_response(p)).collect(),
    }))
}

pub async fn create_provider(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<CreateProviderRequest>,
) -> Result<Json<ProviderDetailResponse>, ApiError> {
    validate_provider_kind(&body.kind).map_err(|e| {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid_provider_kind", e.message.unwrap_or_default())
    })?;
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let name = sanitize_display_text(&body.name);
    let api_key_enc = body
        .api_key
        .as_deref()
        .and_then(|k| encrypt_api_key(k, &state.config.master_key))
        .unwrap_or_default();

    let config_json = build_config_json(&body);
    let priority = body.priority.unwrap_or(0);

    let provider = repo
        .create(
            auth.org_id,
            &name,
            &body.kind,
            body.base_url.as_deref(),
            &api_key_enc,
            json!({}),
            config_json,
            priority,
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    // Create provider models if specified
    let models = if let Some(model_ids) = body.models {
        let registry = ModelRegistry::new(state.db_pool.clone());
        let mut created_models = Vec::new();
        for model_id in model_ids {
            match registry
                .create_model(auth.org_id, provider.id, &model_id, &model_id)
                .await
            {
                Ok(m) => created_models.push(m),
                Err(e) => {
                    tracing::warn!(error = %e, model_id = %model_id, "Failed to create provider model");
                }
            }
        }
        created_models
    } else {
        vec![]
    };

    let model_responses = models
        .into_iter()
        .map(|m| ProviderModelResponse {
            id: m.id.to_string(),
            name: m.model_name,
            context_window: m.context_window,
            capabilities: vec![],
            pricing: None,
            status: m.status,
        })
        .collect();

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::ProviderCreated,
        "provider",
        Some(&provider.id.to_string()),
        None,
        Some(json!({
            "name": provider.name,
            "kind": provider.kind,
            "base_url": provider.api_base,
        })),
        "Provider created",
    )
    .await;

    Ok(Json(ProviderDetailResponse {
        provider: db_to_response(&provider),
        models: model_responses,
        health: None,
        routing_weight: body.weight.unwrap_or(1),
        priority,
    }))
}

pub async fn get_provider(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<ProviderDetailResponse>, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let provider = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    // Fetch models
    let registry = ModelRegistry::new(state.db_pool.clone());
    let db_models = registry
        .list_models(auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let models: Vec<ProviderModelResponse> = db_models
        .into_iter()
        .filter(|m| m.provider_config_id == provider_id)
        .map(|m| ProviderModelResponse {
            id: m.provider_config_id.to_string(), // Using provider_config_id as model id for now
            name: m.model_name,
            context_window: m.capabilities.max_context,
            capabilities: vec![],
            pricing: Some({
                let input_per_1m = m.pricing.input_cost_per_1k.into_inner() * rust_decimal::Decimal::from(1000);
                let output_per_1m = m.pricing.output_cost_per_1k.into_inner() * rust_decimal::Decimal::from(1000);
                json!({
                    "input_per_1m_tokens": input_per_1m.to_string().parse::<f64>().unwrap_or(0.0),
                    "output_per_1m_tokens": output_per_1m.to_string().parse::<f64>().unwrap_or(0.0),
                    "currency": "USD"
                })
            }),
            status: m.status,
        })
        .collect();

    let health = fetch_provider_health(&state.redis, provider_id).await;

    let weight = provider
        .config
        .get("weight")
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

    Ok(Json(ProviderDetailResponse {
        provider: db_to_response(&provider),
        models,
        health,
        routing_weight: weight,
        priority: provider.priority,
    }))
}

pub async fn update_provider(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(provider_id): Path<Uuid>,
    ValidatedJson(body): ValidatedJson<UpdateProviderRequest>,
) -> Result<Json<ProviderDetailResponse>, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());

    // Get existing provider to merge config
    let existing = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    let api_key_enc = body
        .api_key
        .as_deref()
        .and_then(|k| encrypt_api_key(k, &state.config.master_key));

    let config_json = Some(merge_config_json(&existing.config, &body));

    let base_url_ref: Option<Option<&str>> = body.base_url.as_ref().map(|b| b.as_deref());

    let provider = repo
        .update(
            provider_id,
            auth.org_id,
            body.name.as_deref(),
            base_url_ref,
            api_key_enc.as_deref(),
            Some(json!({})),
            config_json,
            body.priority,
            body.status.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    // Update models if specified
    if let Some(model_ids) = body.models {
        let registry = ModelRegistry::new(state.db_pool.clone());
        let _ = registry.delete_models_by_provider(auth.org_id, provider_id).await;
        for model_id in model_ids {
            let _ = registry
                .create_model(auth.org_id, provider_id, &model_id, &model_id)
                .await;
        }
    }

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::ProviderUpdated,
        "provider",
        Some(&provider.id.to_string()),
        Some(json!({
            "name": existing.name,
            "kind": existing.kind,
            "base_url": existing.api_base,
            "status": existing.status,
        })),
        Some(json!({
            "name": provider.name,
            "kind": provider.kind,
            "base_url": provider.api_base,
            "status": provider.status,
        })),
        "Provider updated",
    )
    .await;

    let health = fetch_provider_health(&state.redis, provider_id).await;
    let weight = provider
        .config
        .get("weight")
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

    Ok(Json(ProviderDetailResponse {
        provider: db_to_response(&provider),
        models: vec![], // Simplified: refetch on detail page
        health,
        routing_weight: weight,
        priority: provider.priority,
    }))
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let existing = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    repo.soft_delete(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    // Also delete associated models
    let registry = ModelRegistry::new(state.db_pool.clone());
    let _ = registry.delete_models_by_provider(auth.org_id, provider_id).await;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::ProviderDeleted,
        "provider",
        Some(&existing.id.to_string()),
        Some(json!({
            "name": existing.name,
            "kind": existing.kind,
            "base_url": existing.api_base,
        })),
        None,
        "Provider deleted",
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_provider_health(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<ProviderHealthResponse>, ApiError> {
    // Verify provider exists
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let _ = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    let health = fetch_provider_health(&state.redis, provider_id)
        .await
        .unwrap_or_else(|| ProviderHealthResponse {
            provider_id: provider_id.to_string(),
            status: "unknown".to_string(),
            latency_ms: 0,
            error_rate: 0.0,
            last_checked: chrono::Utc::now().to_rfc3339(),
            message: Some("No health check data available".to_string()),
        });

    Ok(Json(health))
}

pub async fn trigger_health_check(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<ProviderHealthResponse>, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let config = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    let kind = parse_provider_kind(&config.kind).ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_provider_kind",
            format!("Unknown provider kind: {}", config.kind),
        )
    })?;

    let base_url = config
        .api_base
        .clone()
        .unwrap_or_else(|| default_base_url(&kind));

    let api_key = decrypt_api_key(&config.api_key_enc, &state.config.master_key)
        .unwrap_or_default();

    let provider_config = FactoryProviderConfig {
        kind: kind.clone(),
        provider_id: config.id.to_string(),
        base_url,
        api_key,
        default_model: String::new(),
        timeout_ms: 10000,
    };

    let start = std::time::Instant::now();
    let result = match gateway_providers::factory::create_provider(provider_config) {
        Ok(provider) => {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                provider.health_check(),
            )
            .await
            {
                Ok(gateway_providers::traits::HealthStatus::Healthy) => {
                    ProviderHealthResponse {
                        provider_id: provider_id.to_string(),
                        status: "healthy".to_string(),
                        latency_ms: start.elapsed().as_millis() as u64,
                        error_rate: 0.0,
                        last_checked: chrono::Utc::now().to_rfc3339(),
                        message: None,
                    }
                }
                Ok(gateway_providers::traits::HealthStatus::Degraded(msg)) => {
                    ProviderHealthResponse {
                        provider_id: provider_id.to_string(),
                        status: "degraded".to_string(),
                        latency_ms: start.elapsed().as_millis() as u64,
                        error_rate: 0.0,
                        last_checked: chrono::Utc::now().to_rfc3339(),
                        message: Some(msg),
                    }
                }
                Ok(gateway_providers::traits::HealthStatus::Unhealthy(msg)) => {
                    ProviderHealthResponse {
                        provider_id: provider_id.to_string(),
                        status: "unhealthy".to_string(),
                        latency_ms: start.elapsed().as_millis() as u64,
                        error_rate: 0.0,
                        last_checked: chrono::Utc::now().to_rfc3339(),
                        message: Some(msg),
                    }
                }
                Err(_) => ProviderHealthResponse {
                    provider_id: provider_id.to_string(),
                    status: "unhealthy".to_string(),
                    latency_ms: start.elapsed().as_millis() as u64,
                    error_rate: 0.0,
                    last_checked: chrono::Utc::now().to_rfc3339(),
                    message: Some("Health check timed out".to_string()),
                },
            }
        }
        Err(e) => ProviderHealthResponse {
            provider_id: provider_id.to_string(),
            status: "unhealthy".to_string(),
            latency_ms: 0,
            error_rate: 0.0,
            last_checked: chrono::Utc::now().to_rfc3339(),
            message: Some(format!("Failed to create provider: {}", e)),
        },
    };

    Ok(Json(result))
}

pub async fn get_health_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<HealthHistoryResponse>, ApiError> {
    // Verify provider exists
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let _ = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    let history = fetch_health_history(&state.redis, provider_id, 24).await;

    Ok(Json(HealthHistoryResponse {
        object: "list".to_string(),
        data: history,
    }))
}

#[derive(Debug, Deserialize, Validate)]
pub struct TestConnectionRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    #[validate(length(min = 1, max = 32, message = "Kind must be 1-32 characters"))]
    pub kind: String,
    pub api_key: Option<String>,
    #[validate(url(message = "Base URL must be a valid URL"))]
    pub base_url: Option<String>,
}

pub async fn test_connection(
    State(_state): State<AppState>,
    ValidatedJson(body): ValidatedJson<TestConnectionRequest>,
) -> Result<Json<TestConnectionResponse>, ApiError> {
    let kind = parse_provider_kind(&body.kind).ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_provider_kind",
            format!("Unknown provider kind: {}", body.kind),
        )
    })?;

    let base_url = body
        .base_url
        .clone()
        .unwrap_or_else(|| default_base_url(&kind));

    let provider_config = FactoryProviderConfig {
        kind: kind.clone(),
        provider_id: "test".to_string(),
        base_url,
        api_key: body.api_key.unwrap_or_default(),
        default_model: String::new(),
        timeout_ms: 10000,
    };

    let start = std::time::Instant::now();
    let result = match gateway_providers::factory::create_provider(provider_config) {
        Ok(provider) => {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                provider.health_check(),
            )
            .await
            {
                Ok(gateway_providers::traits::HealthStatus::Healthy) => TestConnectionResponse {
                    success: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: None,
                },
                Ok(gateway_providers::traits::HealthStatus::Degraded(msg)) => TestConnectionResponse {
                    success: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some(msg),
                },
                Ok(gateway_providers::traits::HealthStatus::Unhealthy(msg)) => TestConnectionResponse {
                    success: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some(msg),
                },
                Err(_) => TestConnectionResponse {
                    success: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some("Health check timed out".to_string()),
                },
            }
        }
        Err(e) => TestConnectionResponse {
            success: false,
            latency_ms: 0,
            message: Some(format!("Failed to create provider: {}", e)),
        },
    };

    Ok(Json(result))
}

pub async fn test_existing_connection(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(provider_id): Path<Uuid>,
) -> Result<Json<TestConnectionResponse>, ApiError> {
    let repo = ProviderConfigRepo::new(state.db_pool.clone());
    let config = repo
        .get_by_id(provider_id, auth.org_id)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "provider_not_found", "Provider not found"))?;

    let kind = parse_provider_kind(&config.kind).ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_provider_kind",
            format!("Unknown provider kind: {}", config.kind),
        )
    })?;

    let base_url = config
        .api_base
        .clone()
        .unwrap_or_else(|| default_base_url(&kind));

    let api_key = decrypt_api_key(&config.api_key_enc, &state.config.master_key)
        .unwrap_or_default();

    let provider_config = FactoryProviderConfig {
        kind: kind.clone(),
        provider_id: config.id.to_string(),
        base_url,
        api_key,
        default_model: String::new(),
        timeout_ms: 10000,
    };

    let start = std::time::Instant::now();
    let result = match gateway_providers::factory::create_provider(provider_config) {
        Ok(provider) => {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                provider.health_check(),
            )
            .await
            {
                Ok(gateway_providers::traits::HealthStatus::Healthy) => TestConnectionResponse {
                    success: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: None,
                },
                Ok(gateway_providers::traits::HealthStatus::Degraded(msg)) => TestConnectionResponse {
                    success: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some(msg),
                },
                Ok(gateway_providers::traits::HealthStatus::Unhealthy(msg)) => TestConnectionResponse {
                    success: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some(msg),
                },
                Err(_) => TestConnectionResponse {
                    success: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: Some("Health check timed out".to_string()),
                },
            }
        }
        Err(e) => TestConnectionResponse {
            success: false,
            latency_ms: 0,
            message: Some(format!("Failed to create provider: {}", e)),
        },
    };

    Ok(Json(result))
}
