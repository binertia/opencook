//! Audit log routes — read-only, RBAC-protected.
//!
//! Endpoints:
//!   GET /api/v1/organizations/:org_id/audit-log
//!   GET /api/v1/organizations/:org_id/audit-log/:entry_id

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::{
    rbac::{check_permission, Permission, Role},
    AuthContext,
};
use gateway_db::{
    models::AuditAction,
    repos::audit_repo::{AuditListFilter, AuditRepo},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    audit::record as audit_record, audit::AuditRequestContext, error::ApiError, state::AppState,
};

/// Require the caller to have `audit:read` and be accessing their own org.
fn require_audit_access(auth: &AuthContext, org_id: Uuid) -> Result<(), Box<ApiError>> {
    let role = auth
        .role
        .as_deref()
        .and_then(Role::from_str)
        .unwrap_or(Role::Viewer);

    if !check_permission(role, Permission::AuditRead) {
        return Err(Box::new(ApiError::new(
            StatusCode::FORBIDDEN,
            "insufficient_permissions",
            "Audit log access requires owner or admin role".to_string(),
        )));
    }

    if auth.org_id != org_id {
        return Err(Box::new(ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_org_access",
            "Cannot access audit log for another organization".to_string(),
        )));
    }

    Ok(())
}

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuditListQuery {
    pub user_id: Option<String>,
    pub api_key_id: Option<String>,
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

fn default_offset() -> i64 {
    0
}

#[derive(Debug, Serialize)]
pub struct AuditEntryItem {
    pub id: String,
    pub org_id: String,
    pub user_id: Option<String>,
    pub api_key_id: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub summary: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AuditListResponse {
    pub object: String,
    pub data: Vec<AuditEntryItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct AuditEntryResponse {
    pub object: String,
    pub data: AuditEntryItem,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_audit_entries(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<AuditListQuery>,
) -> Result<Json<AuditListResponse>, ApiError> {
    require_audit_access(&auth, org_id)?;

    let repo = AuditRepo::new(state.db_pool.clone());

    let filter = build_filter(&query)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_filter", e.to_string()))?;

    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.max(0);

    let result = repo
        .list(org_id, filter, limit, offset)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    Ok(Json(AuditListResponse {
        object: "list".to_string(),
        data: result.entries.iter().map(db_to_item).collect(),
        total: result.total,
        limit,
        offset,
    }))
}

pub async fn get_audit_entry(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((org_id, entry_id)): Path<(Uuid, String)>,
) -> Result<Json<AuditEntryResponse>, ApiError> {
    require_audit_access(&auth, org_id)?;

    let repo = AuditRepo::new(state.db_pool.clone());

    let id = Uuid::parse_str(&entry_id).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_entry_id",
            "Invalid audit entry ID",
        )
    })?;

    let entry = repo
        .get_by_id(org_id, id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "audit_entry_not_found",
                "Audit entry not found",
            )
        })?;

    Ok(Json(AuditEntryResponse {
        object: "audit_entry".to_string(),
        data: db_to_item(&entry),
    }))
}

/// Record a security-violation audit entry when cross-org access is attempted.
pub async fn record_security_violation(
    state: &AppState,
    auth: &AuthContext,
    ctx: &AuditRequestContext,
    target_org_id: Uuid,
    summary: &str,
) {
    let detail = serde_json::json!({
        "target_org_id": target_org_id.to_string(),
        "actor_org_id": auth.org_id.to_string(),
        "actor_user_id": auth.user_id.map(|u| u.to_string()),
    });
    audit_record(
        state,
        auth,
        ctx,
        AuditAction::SecurityViolation,
        "organization",
        Some(&target_org_id.to_string()),
        None,
        Some(detail),
        summary,
    )
    .await;
}

// ── Helpers ──────────────────────────────────────────────────────────

fn build_filter(query: &AuditListQuery) -> Result<AuditListFilter, anyhow::Error> {
    let mut filter = AuditListFilter::default();

    if let Some(s) = &query.user_id {
        filter.user_id = Some(Uuid::parse_str(s)?);
    }
    if let Some(s) = &query.api_key_id {
        filter.api_key_id = Some(Uuid::parse_str(s)?);
    }
    if let Some(s) = &query.action {
        let parts: Vec<&str> = s
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .collect();
        let mut actions = Vec::new();
        for part in parts {
            actions.push(parse_action(part)?);
        }
        filter.actions = actions;
    }
    if let Some(s) = &query.entity_type {
        filter.entity_type = Some(s.clone());
    }
    if let Some(s) = &query.entity_id {
        filter.entity_id = Some(s.clone());
    }
    if let Some(s) = &query.start {
        filter.start = Some(s.parse::<chrono::DateTime<chrono::Utc>>()?);
    }
    if let Some(s) = &query.end {
        filter.end = Some(s.parse::<chrono::DateTime<chrono::Utc>>()?);
    }

    Ok(filter)
}

fn parse_action(s: &str) -> Result<AuditAction, anyhow::Error> {
    match s {
        "create" => Ok(AuditAction::Create),
        "update" => Ok(AuditAction::Update),
        "delete" => Ok(AuditAction::Delete),
        "login" => Ok(AuditAction::Login),
        "logout" => Ok(AuditAction::Logout),
        "api_key.created" => Ok(AuditAction::ApiKeyCreated),
        "api_key.revoked" => Ok(AuditAction::ApiKeyRevoked),
        "provider.created" => Ok(AuditAction::ProviderCreated),
        "provider.updated" => Ok(AuditAction::ProviderUpdated),
        "provider.deleted" => Ok(AuditAction::ProviderDeleted),
        "quota.exceeded" => Ok(AuditAction::QuotaExceeded),
        "quota.warning" => Ok(AuditAction::QuotaWarning),
        "webhook.created" => Ok(AuditAction::WebhookCreated),
        "webhook.deleted" => Ok(AuditAction::WebhookDeleted),
        "routing_rule.created" => Ok(AuditAction::RoutingRuleCreated),
        "routing_rule.updated" => Ok(AuditAction::RoutingRuleUpdated),
        "settings.updated" => Ok(AuditAction::SettingsUpdated),
        "billing.updated" => Ok(AuditAction::BillingUpdated),
        "user.role_changed" => Ok(AuditAction::UserRoleChanged),
        "security.violation" => Ok(AuditAction::SecurityViolation),
        _ => Err(anyhow::anyhow!("unknown audit action: {}", s)),
    }
}

fn db_to_item(entry: &gateway_db::AuditEntry) -> AuditEntryItem {
    AuditEntryItem {
        id: entry.id.to_string(),
        org_id: entry.org_id.to_string(),
        user_id: entry.user_id.map(|u| u.to_string()),
        api_key_id: entry.api_key_id.map(|u| u.to_string()),
        action: entry.action.clone(),
        entity_type: entry.entity_type.clone(),
        entity_id: entry.entity_id.clone(),
        old_values: entry.old_values.clone(),
        new_values: entry.new_values.clone(),
        summary: entry.summary.clone(),
        ip_address: entry.ip_address.clone(),
        user_agent: entry.user_agent.clone(),
        request_id: entry.request_id.map(|u| u.to_string()),
        created_at: entry.created_at.to_rfc3339(),
    }
}
