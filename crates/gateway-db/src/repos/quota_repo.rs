//! Quota definition repository.

use crate::pool::DbBackend;
use crate::types::DbDecimal;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::Quota;

/// Repository for quota definitions.
#[derive(Clone)]
pub struct QuotaRepo {
    pool: DbBackend,
}

impl QuotaRepo {
    /// Create a new quota repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Find all active quotas for an organization.
    pub async fn find_active_by_org(&self, org_id: Uuid) -> Result<Vec<Quota>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric::text, period::text, limit_value, warning_threshold,
                           applies_to, scope_filter, action::text, status::text,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE org_id = $1
                      AND status = 'active'
                      AND deleted_at IS NULL
                    ORDER BY created_at
                    "#,
                )
                .bind(org_id)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric, period, limit_value, warning_threshold,
                           applies_to, scope_filter, action, status,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE org_id = $1
                      AND status = 'active'
                      AND deleted_at IS NULL
                    ORDER BY created_at
                    "#,
                )
                .bind(org_id)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }

    /// Find active quotas for an organization and optional API key.
    pub async fn find_active_for_context(
        &self,
        org_id: Uuid,
        api_key_id: Option<Uuid>,
    ) -> Result<Vec<Quota>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                if let Some(key_id) = api_key_id {
                    sqlx::query_as::<_, Quota>(
                        r#"
                        SELECT id, org_id, api_key_id, name, description,
                               metric::text, period::text, limit_value, warning_threshold,
                               applies_to, scope_filter, action::text, status::text,
                               created_at, updated_at, deleted_at
                        FROM quotas
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND (api_key_id IS NULL OR api_key_id = $2)
                        ORDER BY created_at
                        "#,
                    )
                    .bind(org_id)
                    .bind(key_id)
                    .fetch_all(pg)
                    .await?
                } else {
                    sqlx::query_as::<_, Quota>(
                        r#"
                        SELECT id, org_id, api_key_id, name, description,
                               metric::text, period::text, limit_value, warning_threshold,
                               applies_to, scope_filter, action::text, status::text,
                               created_at, updated_at, deleted_at
                        FROM quotas
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND api_key_id IS NULL
                        ORDER BY created_at
                        "#,
                    )
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?
                }
            }
            DbBackend::Sqlite(sqlite) => {
                if let Some(key_id) = api_key_id {
                    sqlx::query_as::<_, Quota>(
                        r#"
                        SELECT id, org_id, api_key_id, name, description,
                               metric, period, limit_value, warning_threshold,
                               applies_to, scope_filter, action, status,
                               created_at, updated_at, deleted_at
                        FROM quotas
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND (api_key_id IS NULL OR api_key_id = $2)
                        ORDER BY created_at
                        "#,
                    )
                    .bind(org_id)
                    .bind(key_id)
                    .fetch_all(sqlite)
                    .await?
                } else {
                    sqlx::query_as::<_, Quota>(
                        r#"
                        SELECT id, org_id, api_key_id, name, description,
                               metric, period, limit_value, warning_threshold,
                               applies_to, scope_filter, action, status,
                               created_at, updated_at, deleted_at
                        FROM quotas
                        WHERE org_id = $1
                          AND status = 'active'
                          AND deleted_at IS NULL
                          AND api_key_id IS NULL
                        ORDER BY created_at
                        "#,
                    )
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?
                }
            }
        };

        Ok(rows)
    }

    /// Get a single quota by ID.
    pub async fn get_by_id(&self, org_id: Uuid, quota_id: Uuid) -> Result<Option<Quota>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric::text, period::text, limit_value, warning_threshold,
                           applies_to, scope_filter, action::text, status::text,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric, period, limit_value, warning_threshold,
                           applies_to, scope_filter, action, status,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// List all quotas for an organization (including inactive).
    pub async fn list_by_org(&self, org_id: Uuid) -> Result<Vec<Quota>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric::text, period::text, limit_value, warning_threshold,
                           applies_to, scope_filter, action::text, status::text,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE org_id = $1
                      AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    "#,
                )
                .bind(org_id)
                .fetch_all(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    SELECT id, org_id, api_key_id, name, description,
                           metric, period, limit_value, warning_threshold,
                           applies_to, scope_filter, action, status,
                           created_at, updated_at, deleted_at
                    FROM quotas
                    WHERE org_id = $1
                      AND deleted_at IS NULL
                    ORDER BY created_at DESC
                    "#,
                )
                .bind(org_id)
                .fetch_all(sqlite)
                .await?
            }
        };

        Ok(rows)
    }

    /// Create a new quota.
    pub async fn create(
        &self,
        org_id: Uuid,
        api_key_id: Option<Uuid>,
        name: &str,
        description: Option<&str>,
        metric: &str,
        period: &str,
        limit_value: impl Into<DbDecimal>,
        warning_threshold: impl Into<DbDecimal>,
        applies_to: &str,
        scope_filter: serde_json::Value,
        action: &str,
        status: &str,
    ) -> Result<Quota, DbError> {
        let limit_value = limit_value.into();
        let warning_threshold = warning_threshold.into();
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    INSERT INTO quotas (
                        org_id, api_key_id, name, description,
                        metric, period, limit_value, warning_threshold,
                        applies_to, scope_filter, action, status
                    )
                    VALUES ($1, $2, $3, $4, $5::text::quota_metric, $6::text::quota_period,
                            $7, $8, $9, $10, $11, $12)
                    RETURNING id, org_id, api_key_id, name, description,
                              metric::text, period::text, limit_value, warning_threshold,
                              applies_to, scope_filter, action::text, status::text,
                              created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(name)
                .bind(description)
                .bind(metric)
                .bind(period)
                .bind(limit_value)
                .bind(warning_threshold)
                .bind(applies_to)
                .bind(scope_filter)
                .bind(action)
                .bind(status)
                .fetch_one(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    INSERT INTO quotas (
                        org_id, api_key_id, name, description,
                        metric, period, limit_value, warning_threshold,
                        applies_to, scope_filter, action, status
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    RETURNING id, org_id, api_key_id, name, description,
                              metric, period, limit_value, warning_threshold,
                              applies_to, scope_filter, action, status,
                              created_at, updated_at, deleted_at
                    "#,
                )
                .bind(org_id)
                .bind(api_key_id)
                .bind(name)
                .bind(description)
                .bind(metric)
                .bind(period)
                .bind(limit_value)
                .bind(warning_threshold)
                .bind(applies_to)
                .bind(scope_filter)
                .bind(action)
                .bind(status)
                .fetch_one(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Update an existing quota.
    pub async fn update(
        &self,
        org_id: Uuid,
        quota_id: Uuid,
        name: Option<&str>,
        description: Option<Option<&str>>,
        metric: Option<&str>,
        period: Option<&str>,
        limit_value: Option<impl Into<DbDecimal>>,
        warning_threshold: Option<impl Into<DbDecimal>>,
        applies_to: Option<&str>,
        scope_filter: Option<serde_json::Value>,
        action: Option<&str>,
        status: Option<&str>,
    ) -> Result<Option<Quota>, DbError> {
        let limit_value = limit_value.map(Into::into);
        let warning_threshold = warning_threshold.map(Into::into);
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    UPDATE quotas
                    SET
                        name = COALESCE($3, name),
                        description = COALESCE($4, description),
                        metric = COALESCE($5::text::quota_metric, metric),
                        period = COALESCE($6::text::quota_period, period),
                        limit_value = COALESCE($7, limit_value),
                        warning_threshold = COALESCE($8, warning_threshold),
                        applies_to = COALESCE($9, applies_to),
                        scope_filter = COALESCE($10, scope_filter),
                        action = COALESCE($11, action),
                        status = COALESCE($12, status),
                        updated_at = NOW()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING id, org_id, api_key_id, name, description,
                              metric::text, period::text, limit_value, warning_threshold,
                              applies_to, scope_filter, action::text, status::text,
                              created_at, updated_at, deleted_at
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .bind(name)
                .bind(description)
                .bind(metric)
                .bind(period)
                .bind(limit_value)
                .bind(warning_threshold)
                .bind(applies_to)
                .bind(scope_filter)
                .bind(action)
                .bind(status)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, Quota>(
                    r#"
                    UPDATE quotas
                    SET
                        name = COALESCE($3, name),
                        description = COALESCE($4, description),
                        metric = COALESCE($5, metric),
                        period = COALESCE($6, period),
                        limit_value = COALESCE($7, limit_value),
                        warning_threshold = COALESCE($8, warning_threshold),
                        applies_to = COALESCE($9, applies_to),
                        scope_filter = COALESCE($10, scope_filter),
                        action = COALESCE($11, action),
                        status = COALESCE($12, status),
                        updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    RETURNING id, org_id, api_key_id, name, description,
                              metric, period, limit_value, warning_threshold,
                              applies_to, scope_filter, action, status,
                              created_at, updated_at, deleted_at
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .bind(name)
                .bind(description)
                .bind(metric)
                .bind(period)
                .bind(limit_value)
                .bind(warning_threshold)
                .bind(applies_to)
                .bind(scope_filter)
                .bind(action)
                .bind(status)
                .fetch_optional(sqlite)
                .await?
            }
        };

        Ok(row)
    }

    /// Soft-delete a quota.
    pub async fn delete(&self, org_id: Uuid, quota_id: Uuid) -> Result<bool, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let result = sqlx::query(
                    r#"
                    UPDATE quotas
                    SET deleted_at = NOW(), updated_at = NOW()
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .execute(pg)
                .await?;

                Ok(result.rows_affected() > 0)
            }
            DbBackend::Sqlite(sqlite) => {
                let result = sqlx::query(
                    r#"
                    UPDATE quotas
                    SET deleted_at = datetime('now'), updated_at = datetime('now')
                    WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
                    "#,
                )
                .bind(quota_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;

                Ok(result.rows_affected() > 0)
            }
        }
    }
}
