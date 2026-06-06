//! Append-only audit log repository.

use crate::error::DbError;
use crate::models::{AuditAction, AuditEntry};
use crate::pool::DbBackend;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

/// Filter parameters for listing audit log entries.
#[derive(Debug, Clone, Default)]
pub struct AuditListFilter {
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub actions: Vec<AuditAction>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// Pagination cursor for audit log listing.
#[derive(Debug, Clone)]
pub struct AuditListResult {
    pub entries: Vec<AuditEntry>,
    pub total: i64,
}

/// Repository for the `audit_log` table.
#[derive(Clone)]
pub struct AuditRepo {
    pool: DbBackend,
}

impl AuditRepo {
    /// Create a new audit repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Record a new audit entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        &self,
        org_id: Uuid,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
        action: AuditAction,
        entity_type: &str,
        entity_id: Option<&str>,
        old_values: Option<serde_json::Value>,
        new_values: Option<serde_json::Value>,
        summary: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        request_id: Option<Uuid>,
    ) -> Result<AuditEntry, DbError> {
        let action_str = action.as_str();
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, AuditEntry>(
                    r#"
                    INSERT INTO audit_log (
                        org_id, user_id, api_key_id,
                        action, entity_type, entity_id,
                        old_values, new_values, summary,
                        ip_address, user_agent, request_id,
                        created_at
                    )
                    VALUES (
                        $1, $2, $3,
                        $4::audit_action, $5, $6,
                        $7, $8, $9,
                        $10, $11, $12,
                        NOW()
                    )
                    RETURNING
                        id, org_id, user_id, api_key_id,
                        action::text, entity_type, entity_id,
                        old_values, new_values, summary,
                        ip_address, user_agent, request_id,
                        created_at
                    "#,
                )
                .bind(org_id)
                .bind(user_id)
                .bind(api_key_id)
                .bind(action_str)
                .bind(entity_type)
                .bind(entity_id)
                .bind(old_values)
                .bind(new_values)
                .bind(summary)
                .bind(ip_address)
                .bind(user_agent)
                .bind(request_id)
                .fetch_one(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, AuditEntry>(
                    r#"
                    INSERT INTO audit_log (
                        org_id, user_id, api_key_id,
                        action, entity_type, entity_id,
                        old_values, new_values, summary,
                        ip_address, user_agent, request_id,
                        created_at
                    )
                    VALUES (
                        $1, $2, $3,
                        $4, $5, $6,
                        $7, $8, $9,
                        $10, $11, $12,
                        datetime('now')
                    )
                    RETURNING
                        id, org_id, user_id, api_key_id,
                        action, entity_type, entity_id,
                        old_values, new_values, summary,
                        ip_address, user_agent, request_id,
                        created_at
                    "#,
                )
                .bind(org_id)
                .bind(user_id)
                .bind(api_key_id)
                .bind(action_str)
                .bind(entity_type)
                .bind(entity_id)
                .bind(old_values)
                .bind(new_values)
                .bind(summary)
                .bind(ip_address)
                .bind(user_agent)
                .bind(request_id)
                .fetch_one(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// Get a single audit entry by ID (org-scoped).
    pub async fn get_by_id(&self, org_id: Uuid, id: Uuid) -> Result<Option<AuditEntry>, DbError> {
        let sql_pg = r#"
            SELECT
                id, org_id, user_id, api_key_id,
                action::text, entity_type, entity_id,
                old_values, new_values, summary,
                ip_address, user_agent, request_id,
                created_at
            FROM audit_log
            WHERE id = $1 AND org_id = $2
            "#;
        let sql_sqlite = r#"
            SELECT
                id, org_id, user_id, api_key_id,
                action, entity_type, entity_id,
                old_values, new_values, summary,
                ip_address, user_agent, request_id,
                created_at
            FROM audit_log
            WHERE id = $1 AND org_id = $2
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, AuditEntry>(sql_pg)
                    .bind(id)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, AuditEntry>(sql_sqlite)
                    .bind(id)
                    .bind(org_id)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// List audit entries for an org with optional filtering.
    pub async fn list(
        &self,
        org_id: Uuid,
        filter: AuditListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<AuditListResult, DbError> {
        let mut conditions = vec!["org_id = $1".to_string()];
        let mut arg_idx = 2;

        if filter.user_id.is_some() {
            conditions.push(format!("user_id = ${}", arg_idx));
            arg_idx += 1;
        }
        if filter.api_key_id.is_some() {
            conditions.push(format!("api_key_id = ${}", arg_idx));
            arg_idx += 1;
        }
        if !filter.actions.is_empty() {
            let action_placeholders: Vec<String> = filter
                .actions
                .iter()
                .map(|_| {
                    let p = format!("${}", arg_idx);
                    arg_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("action IN ({})", action_placeholders.join(", ")));
        }
        if filter.entity_type.is_some() {
            conditions.push(format!("entity_type = ${}", arg_idx));
            arg_idx += 1;
        }
        if filter.entity_id.is_some() {
            conditions.push(format!("entity_id = ${}", arg_idx));
            arg_idx += 1;
        }
        if filter.start.is_some() {
            conditions.push(format!("created_at >= ${}", arg_idx));
            arg_idx += 1;
        }
        if filter.end.is_some() {
            conditions.push(format!("created_at < ${}", arg_idx));
            arg_idx += 1;
        }

        let where_clause = conditions.join(" AND ");
        let entries_sql = format!(
            "SELECT \
                id, org_id, user_id, api_key_id, \
                action, entity_type, entity_id, \
                old_values, new_values, summary, \
                ip_address, user_agent, request_id, \
                created_at \
            FROM audit_log \
            WHERE {} \
            ORDER BY created_at DESC \
            LIMIT ${} OFFSET ${}",
            where_clause,
            arg_idx,
            arg_idx + 1
        );
        let count_sql = format!(
            "SELECT COUNT(*) as total FROM audit_log WHERE {}",
            where_clause
        );

        let entries = match &self.pool {
            DbBackend::Postgres(pg) => {
                let entries_sql_pg = entries_sql.replace("action,", "action::text,");
                let mut q = sqlx::query_as::<_, AuditEntry>(&entries_sql_pg).bind(org_id);
                if let Some(uid) = filter.user_id {
                    q = q.bind(uid);
                }
                if let Some(akid) = filter.api_key_id {
                    q = q.bind(akid);
                }
                for a in &filter.actions {
                    q = q.bind(a.as_str());
                }
                if let Some(et) = &filter.entity_type {
                    q = q.bind(et);
                }
                if let Some(eid) = &filter.entity_id {
                    q = q.bind(eid);
                }
                if let Some(s) = filter.start {
                    q = q.bind(s);
                }
                if let Some(e) = filter.end {
                    q = q.bind(e);
                }
                q.bind(limit).bind(offset).fetch_all(pg).await?
            }
            DbBackend::Sqlite(sqlite) => {
                let mut q = sqlx::query_as::<_, AuditEntry>(&entries_sql).bind(org_id);
                if let Some(uid) = filter.user_id {
                    q = q.bind(uid);
                }
                if let Some(akid) = filter.api_key_id {
                    q = q.bind(akid);
                }
                for a in &filter.actions {
                    q = q.bind(a.as_str());
                }
                if let Some(et) = &filter.entity_type {
                    q = q.bind(et);
                }
                if let Some(eid) = &filter.entity_id {
                    q = q.bind(eid);
                }
                if let Some(s) = filter.start {
                    q = q.bind(s);
                }
                if let Some(e) = filter.end {
                    q = q.bind(e);
                }
                q.bind(limit).bind(offset).fetch_all(sqlite).await?
            }
        };

        let total: i64 = match &self.pool {
            DbBackend::Postgres(pg) => {
                let mut q = sqlx::query(&count_sql).bind(org_id);
                if let Some(uid) = filter.user_id {
                    q = q.bind(uid);
                }
                if let Some(akid) = filter.api_key_id {
                    q = q.bind(akid);
                }
                for a in &filter.actions {
                    q = q.bind(a.as_str());
                }
                if let Some(et) = &filter.entity_type {
                    q = q.bind(et);
                }
                if let Some(eid) = &filter.entity_id {
                    q = q.bind(eid);
                }
                if let Some(s) = filter.start {
                    q = q.bind(s);
                }
                if let Some(e) = filter.end {
                    q = q.bind(e);
                }
                let row = q.fetch_one(pg).await?;
                row.try_get("total").unwrap_or(0)
            }
            DbBackend::Sqlite(sqlite) => {
                let mut q = sqlx::query(&count_sql).bind(org_id);
                if let Some(uid) = filter.user_id {
                    q = q.bind(uid);
                }
                if let Some(akid) = filter.api_key_id {
                    q = q.bind(akid);
                }
                for a in &filter.actions {
                    q = q.bind(a.as_str());
                }
                if let Some(et) = &filter.entity_type {
                    q = q.bind(et);
                }
                if let Some(eid) = &filter.entity_id {
                    q = q.bind(eid);
                }
                if let Some(s) = filter.start {
                    q = q.bind(s);
                }
                if let Some(e) = filter.end {
                    q = q.bind(e);
                }
                let row = q.fetch_one(sqlite).await?;
                row.try_get("total").unwrap_or(0)
            }
        };

        Ok(AuditListResult { entries, total })
    }
}
