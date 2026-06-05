//! Routing rule admin routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{AuthContext, rbac::{check_permission, Permission, Role}};
use gateway_db::{
    models::{AuditAction, RoutingRule, Target},
    repos::{
        provider_config_repo::ProviderConfigRepo,
        routing_repo::RoutingRepo,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    state::AppState,
    validation::sanitize_display_text,
};

fn require_permission(auth: &AuthContext, permission: Permission) -> Result<(), ApiError> {
    let role = auth
        .role
        .as_deref()
        .and_then(Role::from_str)
        .unwrap_or(Role::Viewer);

    if !check_permission(role, permission) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            format!("Role '{:?}' does not have permission '{:?}'", role, permission),
        ));
    }
    Ok(())
}

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRuleRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: String,
    #[validate(length(max = 512, message = "Description must be at most 512 characters"))]
    pub description: Option<String>,
    #[validate(custom(function = "validate_strategy"))]
    pub strategy: String,
    #[validate(range(min = 1, max = 1000, message = "Priority must be 1-1000"))]
    pub priority: i32,
    #[validate(length(max = 128, message = "Match model must be at most 128 characters"))]
    pub match_model: Option<String>,
    pub match_tags: Option<Vec<String>>,
    pub conditions: Option<serde_json::Value>,
    pub targets: serde_json::Value,
    #[validate(range(min = 1000, max = 300000, message = "Timeout must be 1000-300000 ms"))]
    pub timeout_ms: Option<i32>,
    #[validate(range(min = 0, max = 5, message = "Retries must be 0-5"))]
    pub retries: Option<i32>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRuleRequest {
    #[validate(length(min = 1, max = 128, message = "Name must be 1-128 characters"))]
    pub name: Option<String>,
    #[validate(length(max = 512, message = "Description must be at most 512 characters"))]
    pub description: Option<Option<String>>,
    #[validate(custom(function = "validate_strategy_opt"))]
    pub strategy: Option<String>,
    #[validate(range(min = 1, max = 1000, message = "Priority must be 1-1000"))]
    pub priority: Option<i32>,
    #[validate(length(max = 128, message = "Match model must be at most 128 characters"))]
    pub match_model: Option<Option<String>>,
    pub match_tags: Option<Option<Vec<String>>>,
    pub conditions: Option<serde_json::Value>,
    pub targets: Option<serde_json::Value>,
    #[validate(range(min = 1000, max = 300000, message = "Timeout must be 1000-300000 ms"))]
    pub timeout_ms: Option<i32>,
    #[validate(range(min = 0, max = 5, message = "Retries must be 0-5"))]
    pub retries: Option<i32>,
    #[validate(length(min = 1, max = 32, message = "Status must be 1-32 characters"))]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListRulesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

fn default_limit() -> i64 { 50 }
fn default_offset() -> i64 { 0 }

#[derive(Debug, Serialize)]
pub struct RuleResponse {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub description: Option<String>,
    pub strategy: String,
    pub priority: i32,
    pub match_model: Option<String>,
    pub match_tags: Vec<String>,
    pub conditions: serde_json::Value,
    pub targets: serde_json::Value,
    pub timeout_ms: i32,
    pub retries: i32,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListRulesResponse {
    pub data: Vec<RuleResponse>,
    pub total: i64,
}

// ── Validators ───────────────────────────────────────────────────────

fn validate_strategy(s: &str) -> Result<(), validator::ValidationError> {
    match s {
        "single" | "fallback" | "weighted" | "conditional" => Ok(()),
        _ => {
            let mut err = validator::ValidationError::new("invalid_strategy");
            err.message = Some("Strategy must be one of: single, fallback, weighted, conditional".into());
            Err(err)
        }
    }
}

fn validate_strategy_opt(s: &str) -> Result<(), validator::ValidationError> {
    validate_strategy(s)
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_to_response(rule: &RoutingRule) -> RuleResponse {
    let match_tags: Vec<String> = rule.match_tags.0.clone();
    RuleResponse {
        id: rule.id.to_string(),
        org_id: rule.org_id.to_string(),
        name: rule.name.clone(),
        description: rule.description.clone(),
        strategy: rule.strategy.clone(),
        priority: rule.priority,
        match_model: rule.match_model.clone(),
        match_tags,
        conditions: rule.conditions.clone(),
        targets: rule.targets.clone(),
        timeout_ms: rule.timeout_ms,
        retries: rule.retries,
        status: rule.status.clone(),
        created_at: rule.created_at.to_rfc3339(),
        updated_at: rule.updated_at.to_rfc3339(),
    }
}

/// Validate that targets contains valid provider references.
async fn validate_targets(
    state: &AppState,
    org_id: Uuid,
    targets: &serde_json::Value,
) -> Result<(), ApiError> {
    let targets_arr: Vec<Target> = serde_json::from_value(targets.clone())
        .map_err(|e| ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_targets",
            format!("Targets must be a valid JSON array of target objects: {}", e),
        ))?;

    if targets_arr.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_targets",
            "At least one target provider is required",
        ));
    }

    let provider_repo = ProviderConfigRepo::new(state.db_pool.clone());
    let active_providers = provider_repo.list_active_by_org(org_id).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    let active_ids: std::collections::HashSet<Uuid> = active_providers.into_iter().map(|p| p.id).collect();

    for (idx, target) in targets_arr.iter().enumerate() {
        if !active_ids.contains(&target.provider_config_id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_target_provider",
                format!(
                    "Target at index {} references provider {} which does not exist or is not active",
                    idx,
                    target.provider_config_id
                ),
            ));
        }
    }

    Ok(())
}

/// Validate that conditions is a valid JSON object.
fn validate_conditions(conditions: &serde_json::Value) -> Result<(), ApiError> {
    if !conditions.is_object() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_conditions",
            "Conditions must be a JSON object",
        ));
    }
    Ok(())
}

/// Invalidate routing config cache and publish change event.
async fn invalidate_routing_cache(state: &AppState, org_id: Uuid) {
    let cache_key = format!("config:routing:{}", org_id);
    let mut conn = state.redis.clone();

    let _: Result<(), _> = redis::cmd("DEL")
        .arg(&cache_key)
        .query_async(&mut conn)
        .await;

    let _: Result<(), _> = redis::cmd("PUBLISH")
        .arg("routing:changed")
        .arg(org_id.to_string())
        .query_async(&mut conn)
        .await;

    tracing::debug!(org_id = %org_id, "Invalidated routing cache");
}

/// Shallow-merge two JSON objects; if either is not an object, returns the new value.
fn merge_json_objects(base: &serde_json::Value, update: &serde_json::Value) -> serde_json::Value {
    match (base.as_object(), update.as_object()) {
        (Some(base_obj), Some(update_obj)) => {
            let mut merged = base_obj.clone();
            for (k, v) in update_obj {
                merged.insert(k.clone(), v.clone());
            }
            serde_json::Value::Object(merged)
        }
        _ => update.clone(),
    }
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_rules(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<ListRulesQuery>,
) -> Result<Json<ListRulesResponse>, ApiError> {
    require_permission(&auth, Permission::RoutingRead)?;

    let repo = RoutingRepo::new(state.db_pool.clone());
    let rules = repo.list_rules(auth.org_id).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    let total = rules.len() as i64;
    let data: Vec<RuleResponse> = rules
        .into_iter()
        .skip(query.offset as usize)
        .take(query.limit as usize)
        .map(|r| db_to_response(&r))
        .collect();

    Ok(Json(ListRulesResponse { data, total }))
}

pub async fn create_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<CreateRuleRequest>,
) -> Result<Json<RuleResponse>, ApiError> {
    require_permission(&auth, Permission::RoutingWrite)?;

    let repo = RoutingRepo::new(state.db_pool.clone());

    // Name uniqueness per org
    if let Some(existing) = repo.get_by_name(auth.org_id, &body.name).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })? {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "duplicate_rule_name",
            format!("Routing rule '{}' already exists", existing.name),
        ));
    }

    let name = sanitize_display_text(&body.name);
    let description = body.description.as_deref().map(sanitize_display_text);
    let strategy = sanitize_display_text(&body.strategy);
    let match_model = body.match_model.as_deref().map(sanitize_display_text);
    let match_tags = body.match_tags.unwrap_or_default();
    let conditions = body.conditions.unwrap_or_else(|| json!({}));
    let timeout_ms = body.timeout_ms.unwrap_or(30000);
    let retries = body.retries.unwrap_or(1);
    let status = body.status.as_deref().unwrap_or("active");

    validate_conditions(&conditions)?;
    validate_targets(&state, auth.org_id, &body.targets).await?;

    let rule = RoutingRule {
        id: Uuid::new_v4(),
        org_id: auth.org_id,
        name,
        description,
        strategy,
        priority: body.priority,
        match_model,
        match_tags: gateway_db::types::JsonVec(match_tags),
        conditions,
        targets: body.targets,
        timeout_ms,
        retries,
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        deleted_at: None,
    };

    let created = repo.create_rule(auth.org_id, &rule).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    invalidate_routing_cache(&state, auth.org_id).await;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::RoutingRuleCreated,
        "routing_rule",
        Some(&created.id.to_string()),
        None,
        Some(json!({
            "name": created.name,
            "strategy": created.strategy,
            "priority": created.priority,
            "status": created.status,
        })),
        "Routing rule created",
    )
    .await;

    Ok(Json(db_to_response(&created)))
}

pub async fn get_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<RuleResponse>, ApiError> {
    require_permission(&auth, Permission::RoutingRead)?;

    let repo = RoutingRepo::new(state.db_pool.clone());
    let rule = repo.get_by_id(auth.org_id, rule_id).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    match rule {
        Some(r) => Ok(Json(db_to_response(&r))),
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "routing_rule_not_found",
            format!("Routing rule {} not found", rule_id),
        )),
    }
}

pub async fn update_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(rule_id): Path<Uuid>,
    ValidatedJson(body): ValidatedJson<UpdateRuleRequest>,
) -> Result<Json<RuleResponse>, ApiError> {
    require_permission(&auth, Permission::RoutingWrite)?;

    let repo = RoutingRepo::new(state.db_pool.clone());
    let existing = repo.get_by_id(auth.org_id, rule_id).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?.ok_or_else(|| ApiError::new(
        StatusCode::NOT_FOUND,
        "routing_rule_not_found",
        format!("Routing rule {} not found", rule_id),
    ))?;

    // Name uniqueness if changing name
    if let Some(ref new_name) = body.name {
        let sanitized = sanitize_display_text(new_name);
        if sanitized != existing.name {
            if let Some(other) = repo.get_by_name(auth.org_id, &sanitized).await.map_err(|e| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
            })? {
                if other.id != rule_id {
                    return Err(ApiError::new(
                        StatusCode::CONFLICT,
                        "duplicate_rule_name",
                        format!("Routing rule '{}' already exists", other.name),
                    ));
                }
            }
        }
    }

    // Validate targets if provided
    if let Some(ref targets) = body.targets {
        validate_targets(&state, auth.org_id, targets).await?;
    }

    // Validate conditions if provided
    if let Some(ref conditions) = body.conditions {
        validate_conditions(conditions)?;
    }

    let old_values = json!({
        "name": existing.name,
        "strategy": existing.strategy,
        "priority": existing.priority,
        "status": existing.status,
        "conditions": existing.conditions,
        "targets": existing.targets,
    });

    let updated_rule = RoutingRule {
        id: rule_id,
        org_id: auth.org_id,
        name: body.name.map(|n| sanitize_display_text(&n)).unwrap_or(existing.name),
        description: match body.description {
            Some(Some(d)) => Some(sanitize_display_text(&d)),
            Some(None) => None,
            None => existing.description,
        },
        strategy: body.strategy.map(|s| sanitize_display_text(&s)).unwrap_or(existing.strategy),
        priority: body.priority.unwrap_or(existing.priority),
        match_model: match body.match_model {
            Some(Some(m)) => Some(sanitize_display_text(&m)),
            Some(None) => None,
            None => existing.match_model,
        },
        match_tags: match body.match_tags {
            Some(Some(tags)) => gateway_db::types::JsonVec(tags),
            Some(None) => gateway_db::types::JsonVec(vec![]),
            None => existing.match_tags,
        },
        conditions: match body.conditions {
            Some(c) => merge_json_objects(&existing.conditions, &c),
            None => existing.conditions,
        },
        targets: body.targets.unwrap_or(existing.targets),
        timeout_ms: body.timeout_ms.unwrap_or(existing.timeout_ms),
        retries: body.retries.unwrap_or(existing.retries),
        status: body.status.unwrap_or(existing.status),
        created_at: existing.created_at,
        updated_at: chrono::Utc::now(),
        deleted_at: None,
    };

    let updated = repo.update_rule(auth.org_id, rule_id, &updated_rule).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    invalidate_routing_cache(&state, auth.org_id).await;

    audit::record(
        &state,
        &auth,
        &ctx,
        AuditAction::RoutingRuleUpdated,
        "routing_rule",
        Some(&updated.id.to_string()),
        Some(old_values),
        Some(json!({
            "name": updated.name,
            "strategy": updated.strategy,
            "priority": updated.priority,
            "status": updated.status,
            "conditions": updated.conditions,
            "targets": updated.targets,
        })),
        "Routing rule updated",
    )
    .await;

    Ok(Json(db_to_response(&updated)))
}

pub async fn delete_rule(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    Path(rule_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission(&auth, Permission::RoutingDelete)?;

    let repo = RoutingRepo::new(state.db_pool.clone());
    let existing = repo.get_by_id(auth.org_id, rule_id).await.map_err(|e| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
    })?;

    match existing {
        Some(rule) => {
            repo.delete_rule(auth.org_id, rule_id).await.map_err(|e| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string())
            })?;

            invalidate_routing_cache(&state, auth.org_id).await;

            audit::record(
                &state,
                &auth,
                &ctx,
                AuditAction::RoutingRuleDeleted,
                "routing_rule",
                Some(&rule_id.to_string()),
                Some(json!({
                    "name": rule.name,
                    "strategy": rule.strategy,
                    "priority": rule.priority,
                    "status": rule.status,
                })),
                None,
                "Routing rule deleted",
            )
            .await;

            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "routing_rule_not_found",
            format!("Routing rule {} not found", rule_id),
        )),
    }
}
