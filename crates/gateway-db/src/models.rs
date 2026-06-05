//! Database entity models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Organization (tenant root).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub settings: serde_json::Value,
    pub billing_email: Option<String>,
    pub plan_tier: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Organization membership (junction table).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserOrganization {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

/// Dashboard user.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub role: String,
    pub status: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub failed_login_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// API key for LLM API access.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub scopes: crate::types::JsonVec<String>,
    pub rate_limit_rps: i32,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Quota definition for an organization or API key.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Quota {
    pub id: Uuid,
    pub org_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub metric: String,      // 'requests' | 'tokens' | 'cost_usd'
    pub period: String,      // 'minute' | 'hour' | 'day' | 'month' | 'total'
    pub limit_value: crate::types::DbDecimal,
    pub warning_threshold: crate::types::DbDecimal,
    pub applies_to: String,  // 'all' | 'api_key' | 'model' | 'provider'
    pub scope_filter: serde_json::Value,
    pub action: String,      // 'block' | 'warn' | 'throttle'
    pub status: String,      // 'active' | 'inactive'
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Quota usage record for a specific period.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct QuotaUsage {
    pub id: Uuid,
    pub org_id: Uuid,
    pub quota_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub current_value: crate::types::DbDecimal,
    pub limit_value: crate::types::DbDecimal,
    pub metric: String,
    pub exceeded_at: Option<DateTime<Utc>>,
    pub warned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Provider model entry in the registry.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: Uuid,
    pub org_id: Uuid,
    pub provider_config_id: Uuid,
    pub model_id: String,
    pub model_name: String,
    pub aliases: crate::types::JsonVec<String>,
    pub input_cost_per_1k: crate::types::DbDecimal,
    pub output_cost_per_1k: crate::types::DbDecimal,
    pub context_window: Option<i32>,
    pub max_tokens: Option<i32>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub status: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Provider configuration.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub kind: String,
    pub api_base: Option<String>,
    pub api_key_enc: Vec<u8>,
    pub default_headers: serde_json::Value,
    pub config: serde_json::Value,
    pub priority: i32,
    pub status: String,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error_msg: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Model registry entry (joined view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub model_id: String,
    pub model_name: String,
    pub provider_config_id: Uuid,
    pub provider_name: String,
    pub provider_kind: String,
    pub aliases: crate::types::JsonVec<String>,
    pub pricing: PricingInfo,
    pub capabilities: Capabilities,
    pub status: String,
}

/// Pricing information for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingInfo {
    pub input_cost_per_1k: crate::types::DbDecimal,
    pub output_cost_per_1k: crate::types::DbDecimal,
}

/// Model capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub streaming: bool,
    pub tools: bool,
    pub vision: bool,
    pub json_mode: bool,
    pub max_context: Option<i32>,
    pub max_tokens: Option<i32>,
}

/// Routing rule for provider selection.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub strategy: String, // 'single' | 'fallback' | 'weighted' | 'conditional'
    pub priority: i32,
    pub match_model: Option<String>,
    pub match_tags: crate::types::JsonVec<String>,
    pub conditions: serde_json::Value,
    pub targets: serde_json::Value,
    pub timeout_ms: i32,
    pub retries: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// A routing target (provider + model + weight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub provider_config_id: Uuid,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>, // 'openai' | 'anthropic' | 'gemini' | 'ollama'
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
}

/// Request log entry (partitioned by created_at).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub org_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub provider_config_id: Option<Uuid>,
    pub provider_model_id: Option<Uuid>,
    pub routing_rule_id: Option<Uuid>,

    pub trace_id: String,
    pub parent_trace_id: Option<String>,

    pub method: String,
    pub path: String,
    pub model_requested: Option<String>,
    pub model_routed: Option<String>,

    pub request_headers: serde_json::Value,
    pub request_body: Option<String>,
    pub request_body_truncated: bool,

    pub requested_at: DateTime<Utc>,
    pub gateway_received_at: DateTime<Utc>,
    pub provider_sent_at: Option<DateTime<Utc>>,
    pub provider_responded_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    pub latency_gateway_ms: Option<i32>,
    pub latency_provider_ms: Option<i32>,
    pub latency_total_ms: Option<i32>,

    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,

    pub input_cost: crate::types::DbDecimal,
    pub output_cost: crate::types::DbDecimal,
    pub total_cost: crate::types::DbDecimal,

    pub status: String,
    pub status_code: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,

    pub cache_hit: bool,
    pub cache_key_hash: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Aggregated usage record (partitioned by period_start).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub provider_config_id: Option<Uuid>,
    pub provider_model_id: Option<Uuid>,

    pub period: String,
    pub period_start: DateTime<Utc>,

    pub request_count: i32,
    pub request_success: i32,
    pub request_error: i32,

    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,

    pub input_cost: crate::types::DbDecimal,
    pub output_cost: crate::types::DbDecimal,
    pub total_cost: crate::types::DbDecimal,

    pub latency_ms_p50: Option<i32>,
    pub latency_ms_p90: Option<i32>,
    pub latency_ms_p99: Option<i32>,
    pub latency_ms_avg: Option<i32>,

    pub cache_hits: i32,
    pub cache_misses: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Event types that can trigger webhook deliveries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    RequestCompleted,
    RequestFailed,
    QuotaWarning,
    QuotaExceeded,
    ProviderError,
    ProviderRecovered,
}

impl WebhookEvent {
    /// Return the string representation used in the database and payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            WebhookEvent::RequestCompleted => "request.completed",
            WebhookEvent::RequestFailed => "request.failed",
            WebhookEvent::QuotaWarning => "quota.warning",
            WebhookEvent::QuotaExceeded => "quota.exceeded",
            WebhookEvent::ProviderError => "provider.error",
            WebhookEvent::ProviderRecovered => "provider.recovered",
        }
    }
}

impl std::fmt::Display for WebhookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Webhook configuration for event notifications.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Webhook {
    pub id: Uuid,
    pub org_id: Uuid,

    pub name: String,
    pub url: String,
    pub secret_enc: Option<Vec<u8>>,

    pub events: crate::types::JsonVec<String>,

    pub custom_headers: serde_json::Value,

    pub max_retries: i32,
    pub retry_interval_seconds: i32,
    pub timeout_seconds: i32,

    pub status: String,

    pub last_delivered_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Webhook delivery record.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub org_id: Uuid,
    pub webhook_id: Uuid,

    pub event_type: String,
    pub payload: serde_json::Value,

    pub attempt_number: i32,

    pub request_headers: serde_json::Value,
    pub request_body: Option<String>,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub response_headers: serde_json::Value,

    pub status: String,
    pub error_message: Option<String>,

    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Security-relevant actions tracked in the immutable audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Login,
    Logout,
    ApiKeyCreated,
    ApiKeyRevoked,
    ProviderCreated,
    ProviderUpdated,
    ProviderDeleted,
    QuotaExceeded,
    QuotaWarning,
    WebhookCreated,
    WebhookDeleted,
    RoutingRuleCreated,
    RoutingRuleUpdated,
    SettingsUpdated,
    BillingUpdated,
    UserRoleChanged,
    SecurityViolation,
}

impl AuditAction {
    /// String representation used in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Create => "create",
            AuditAction::Update => "update",
            AuditAction::Delete => "delete",
            AuditAction::Login => "login",
            AuditAction::Logout => "logout",
            AuditAction::ApiKeyCreated => "api_key.created",
            AuditAction::ApiKeyRevoked => "api_key.revoked",
            AuditAction::ProviderCreated => "provider.created",
            AuditAction::ProviderUpdated => "provider.updated",
            AuditAction::ProviderDeleted => "provider.deleted",
            AuditAction::QuotaExceeded => "quota.exceeded",
            AuditAction::QuotaWarning => "quota.warning",
            AuditAction::WebhookCreated => "webhook.created",
            AuditAction::WebhookDeleted => "webhook.deleted",
            AuditAction::RoutingRuleCreated => "routing_rule.created",
            AuditAction::RoutingRuleUpdated => "routing_rule.updated",
            AuditAction::SettingsUpdated => "settings.updated",
            AuditAction::BillingUpdated => "billing.updated",
            AuditAction::UserRoleChanged => "user.role_changed",
            AuditAction::SecurityViolation => "security.violation",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Single audit log entry — immutable, append-only.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,

    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,

    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub summary: String,

    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>,

    pub created_at: DateTime<Utc>,
}
