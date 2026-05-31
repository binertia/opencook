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
    pub scopes: Vec<String>,
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
    pub limit_value: rust_decimal::Decimal,
    pub warning_threshold: rust_decimal::Decimal,
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
    pub current_value: rust_decimal::Decimal,
    pub limit_value: rust_decimal::Decimal,
    pub metric: String,
    pub exceeded_at: Option<DateTime<Utc>>,
    pub warned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
