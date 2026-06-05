//! Routing rule repository.

use crate::pool::DbBackend;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::DbError;
use crate::models::RoutingRule;

/// Repository for routing rules.
#[derive(Clone)]
pub struct RoutingRepo {
    pool: DbBackend,
}

impl RoutingRepo {
    /// Create a new routing rule repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Create a new routing rule.
    pub async fn create_rule(&self, org_id: Uuid, rule: &RoutingRule) -> Result<RoutingRule, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    INSERT INTO routing_rules (
                        org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status
                    )
                    VALUES ($1, $2, $3, $4::routing_strategy, $5, $6, $7, $8, $9, $10, $11, $12)
                    RETURNING
                        id, org_id, name, description, strategy::text, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status::text,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(&rule.name)
                .bind(&rule.description)
                .bind(&rule.strategy)
                .bind(rule.priority)
                .bind(&rule.match_model)
                .bind(&rule.match_tags)
                .bind(&rule.conditions)
                .bind(&rule.targets)
                .bind(rule.timeout_ms)
                .bind(rule.retries)
                .bind(&rule.status)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    INSERT INTO routing_rules (
                        org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    RETURNING
                        id, org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(&rule.name)
                .bind(&rule.description)
                .bind(&rule.strategy)
                .bind(rule.priority)
                .bind(&rule.match_model)
                .bind(&rule.match_tags)
                .bind(&rule.conditions)
                .bind(&rule.targets)
                .bind(rule.timeout_ms)
                .bind(rule.retries)
                .bind(&rule.status)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(org_id = %org_id, rule_id = %row.id, "Created routing rule");
        Ok(row)
    }

    /// List all rules for an organization.
    pub async fn list_rules(&self, org_id: Uuid) -> Result<Vec<RoutingRule>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy::text, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status::text,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE org_id = $1 AND deleted_at IS NULL
                    ORDER BY priority, created_at
                    "#,
                )
                .bind(org_id)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE org_id = $1 AND deleted_at IS NULL
                    ORDER BY priority, created_at
                    "#,
                )
                .bind(org_id)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }

    /// Get active rules matching a model, sorted by priority.
    /// `match_model IS NULL` means wildcard (matches any model).
    pub async fn get_active_rules(
        &self,
        org_id: Uuid,
        model: Option<&str>,
    ) -> Result<Vec<RoutingRule>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                if let Some(model_id) = model {
                    sqlx::query_as::<_, RoutingRule>(
                        r#"
                        SELECT
                            id, org_id, name, description, strategy::text, priority,
                            match_model, match_tags, conditions, targets,
                            timeout_ms, retries, status::text,
                            created_at, updated_at, deleted_at
                        FROM routing_rules
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND (match_model IS NULL OR match_model = $2)
                        ORDER BY priority, created_at
                        "#,
                    )
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_all(pg)
                    .await?
                } else {
                    sqlx::query_as::<_, RoutingRule>(
                        r#"
                        SELECT
                            id, org_id, name, description, strategy::text, priority,
                            match_model, match_tags, conditions, targets,
                            timeout_ms, retries, status::text,
                            created_at, updated_at, deleted_at
                        FROM routing_rules
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                        ORDER BY priority, created_at
                        "#,
                    )
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?
                }
            }
            DbBackend::Sqlite(sqlite) => {
                if let Some(model_id) = model {
                    sqlx::query_as::<_, RoutingRule>(
                        r#"
                        SELECT
                            id, org_id, name, description, strategy, priority,
                            match_model, match_tags, conditions, targets,
                            timeout_ms, retries, status,
                            created_at, updated_at, deleted_at
                        FROM routing_rules
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND (match_model IS NULL OR match_model = $2)
                        ORDER BY priority, created_at
                        "#,
                    )
                    .bind(org_id)
                    .bind(model_id)
                    .fetch_all(sqlite)
                    .await?
                } else {
                    sqlx::query_as::<_, RoutingRule>(
                        r#"
                        SELECT
                            id, org_id, name, description, strategy, priority,
                            match_model, match_tags, conditions, targets,
                            timeout_ms, retries, status,
                            created_at, updated_at, deleted_at
                        FROM routing_rules
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                        ORDER BY priority, created_at
                        "#,
                    )
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?
                }
            }
        };

        debug!(org_id = %org_id, model = ?model, count = rows.len(), "Fetched active routing rules");
        Ok(rows)
    }

    /// Get a single rule by name within an organization.
    pub async fn get_by_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> Result<Option<RoutingRule>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy::text, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status::text,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE org_id = $1 AND name = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE org_id = $1 AND name = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Get a single rule by ID.
    pub async fn get_by_id(
        &self,
        org_id: Uuid,
        rule_id: Uuid,
    ) -> Result<Option<RoutingRule>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy::text, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status::text,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    SELECT
                        id, org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status,
                        created_at, updated_at, deleted_at
                    FROM routing_rules
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Partial update of a routing rule.
    pub async fn update_rule(
        &self,
        org_id: Uuid,
        rule_id: Uuid,
        rule: &RoutingRule,
    ) -> Result<RoutingRule, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    UPDATE routing_rules
                    SET name = $3,
                        description = $4,
                        strategy = $5::routing_strategy,
                        priority = $6,
                        match_model = $7,
                        match_tags = $8,
                        conditions = $9,
                        targets = $10,
                        timeout_ms = $11,
                        retries = $12,
                        status = $13,
                        updated_at = now()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING
                        id, org_id, name, description, strategy::text, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status::text,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .bind(&rule.name)
                .bind(&rule.description)
                .bind(&rule.strategy)
                .bind(rule.priority)
                .bind(&rule.match_model)
                .bind(&rule.match_tags)
                .bind(&rule.conditions)
                .bind(&rule.targets)
                .bind(rule.timeout_ms)
                .bind(rule.retries)
                .bind(&rule.status)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, RoutingRule>(
                    r#"
                    UPDATE routing_rules
                    SET name = $3,
                        description = $4,
                        strategy = $5,
                        priority = $6,
                        match_model = $7,
                        match_tags = $8,
                        conditions = $9,
                        targets = $10,
                        timeout_ms = $11,
                        retries = $12,
                        status = $13,
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING
                        id, org_id, name, description, strategy, priority,
                        match_model, match_tags, conditions, targets,
                        timeout_ms, retries, status,
                        created_at, updated_at, deleted_at
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .bind(&rule.name)
                .bind(&rule.description)
                .bind(&rule.strategy)
                .bind(rule.priority)
                .bind(&rule.match_model)
                .bind(&rule.match_tags)
                .bind(&rule.conditions)
                .bind(&rule.targets)
                .bind(rule.timeout_ms)
                .bind(rule.retries)
                .bind(&rule.status)
                .fetch_one(sqlite)
                .await?
            }
        };

        debug!(org_id = %org_id, rule_id = %rule_id, "Updated routing rule");
        Ok(row)
    }

    /// Soft delete a routing rule.
    pub async fn delete_rule(&self, org_id: Uuid, rule_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let result = sqlx::query(
                    r#"
                    UPDATE routing_rules
                    SET deleted_at = now(), updated_at = now()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .execute(pg)
                .await?;

                if result.rows_affected() == 0 {
                    warn!(org_id = %org_id, rule_id = %rule_id, "Routing rule not found for deletion");
                    return Err(DbError::NotFound(format!(
                        "Routing rule {} not found",
                        rule_id
                    )));
                }
            }
            DbBackend::Sqlite(sqlite) => {
                let result = sqlx::query(
                    r#"
                    UPDATE routing_rules
                    SET deleted_at = datetime('now'), updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(rule_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;

                if result.rows_affected() == 0 {
                    warn!(org_id = %org_id, rule_id = %rule_id, "Routing rule not found for deletion");
                    return Err(DbError::NotFound(format!(
                        "Routing rule {} not found",
                        rule_id
                    )));
                }
            }
        };

        debug!(org_id = %org_id, rule_id = %rule_id, "Deleted routing rule");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_rule_struct() {
        let rule = RoutingRule {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "default".to_string(),
            description: None,
            strategy: "single".to_string(),
            priority: 0,
            match_model: Some("gpt-4o".to_string()),
            match_tags: vec![].into(),
            conditions: serde_json::json!({}),
            targets: serde_json::json!([{"provider_config_id": Uuid::new_v4().to_string(), "model_id": "gpt-4o"}]),
            timeout_ms: 30000,
            retries: 1,
            status: "active".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(rule.strategy, "single");
        assert_eq!(rule.priority, 0);
    }
}
