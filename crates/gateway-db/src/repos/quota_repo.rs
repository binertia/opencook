//! Quota definition repository.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::Quota;

/// Repository for quota definitions.
#[derive(Clone)]
pub struct QuotaRepo {
    pool: PgPool,
}

impl QuotaRepo {
    /// Create a new quota repository.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find all active quotas for an organization.
    pub async fn find_active_by_org(&self, org_id: Uuid) -> Result<Vec<Quota>, DbError> {
        let rows = sqlx::query_as::<_, Quota>(
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
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Find active quotas for an organization and optional API key.
    pub async fn find_active_for_context(
        &self,
        org_id: Uuid,
        api_key_id: Option<Uuid>,
    ) -> Result<Vec<Quota>, DbError> {
        let rows = if let Some(key_id) = api_key_id {
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
            .fetch_all(&self.pool)
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
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Get a single quota by ID.
    pub async fn get_by_id(&self, org_id: Uuid, quota_id: Uuid) -> Result<Option<Quota>, DbError> {
        let row = sqlx::query_as::<_, Quota>(
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
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}
