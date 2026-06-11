//! Provider configuration repository.

use crate::pool::DbBackend;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::ProviderConfig;

/// Repository for provider_configs table.
#[derive(Clone)]
pub struct ProviderConfigRepo {
    pool: DbBackend,
}

impl ProviderConfigRepo {
    /// Create a new provider config repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Create a new provider config.
    /// `api_key_enc` should be the AES-256-GCM encrypted API key (or empty vec for local providers).
    pub async fn create(
        &self,
        org_id: Uuid,
        name: &str,
        kind: &str,
        api_base: Option<&str>,
        api_key_enc: &[u8],
        default_headers: serde_json::Value,
        config: serde_json::Value,
        priority: i32,
    ) -> Result<ProviderConfig, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, ProviderConfig>(
                    r#"
                    INSERT INTO provider_configs (
                        org_id, name, kind, api_base, api_key_enc,
                        default_headers, config, priority, status,
                        created_at, updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', NOW(), NOW())
                    RETURNING *
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(kind)
                .bind(api_base)
                .bind(api_key_enc)
                .bind(default_headers)
                .bind(config)
                .bind(priority)
                .fetch_one(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, ProviderConfig>(
                    r#"
                    INSERT INTO provider_configs (
                        org_id, name, kind, api_base, api_key_enc,
                        default_headers, config, priority, status,
                        created_at, updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', datetime('now'), datetime('now'))
                    RETURNING *
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(kind)
                .bind(api_base)
                .bind(api_key_enc)
                .bind(default_headers)
                .bind(config)
                .bind(priority)
                .fetch_one(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// Get a provider config by ID, scoped to org.
    pub async fn get_by_id(
        &self,
        id: Uuid,
        org_id: Uuid,
    ) -> Result<Option<ProviderConfig>, DbError> {
        let sql = r#"
            SELECT id, org_id, name, kind, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE id = $1
              AND org_id = $2
              AND deleted_at IS NULL
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, ProviderConfig>(sql)
                    .bind(id)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, ProviderConfig>(sql)
                    .bind(id)
                    .bind(org_id)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// List all provider configs for an organization.
    pub async fn list_by_org(&self, org_id: Uuid) -> Result<Vec<ProviderConfig>, DbError> {
        let sql_pg = r#"
            SELECT id, org_id, name, kind::text, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE org_id = $1
              AND deleted_at IS NULL
            ORDER BY priority DESC, created_at
            "#;
        let sql_sqlite = r#"
            SELECT id, org_id, name, kind, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE org_id = $1
              AND deleted_at IS NULL
            ORDER BY priority DESC, created_at
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_pg)
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_sqlite)
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }

    /// List all active provider configs for an organization.
    pub async fn list_active_by_org(&self, org_id: Uuid) -> Result<Vec<ProviderConfig>, DbError> {
        let sql_pg = r#"
            SELECT id, org_id, name, kind::text, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE org_id = $1
              AND status = 'active'
              AND deleted_at IS NULL
            ORDER BY priority DESC, created_at
            "#;
        let sql_sqlite = r#"
            SELECT id, org_id, name, kind, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE org_id = $1
              AND status = 'active'
              AND deleted_at IS NULL
            ORDER BY priority DESC, created_at
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_pg)
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_sqlite)
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }

    /// List all active provider configs across all organizations.
    pub async fn list_all_active(&self) -> Result<Vec<ProviderConfig>, DbError> {
        let sql_pg = r#"
            SELECT id, org_id, name, kind::text, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE status = 'active'
              AND deleted_at IS NULL
            ORDER BY org_id, priority DESC, created_at
            "#;
        let sql_sqlite = r#"
            SELECT id, org_id, name, kind, api_base, api_key_enc,
                   default_headers, config, priority, status,
                   last_error_at, last_error_msg,
                   created_at, updated_at, deleted_at
            FROM provider_configs
            WHERE status = 'active'
              AND deleted_at IS NULL
            ORDER BY org_id, priority DESC, created_at
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_pg)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, ProviderConfig>(sql_sqlite)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }

    /// Update a provider config. `api_key_enc` is updated only if non-empty.
    pub async fn update(
        &self,
        id: Uuid,
        org_id: Uuid,
        name: Option<&str>,
        api_base: Option<Option<&str>>,
        api_key_enc: Option<&[u8]>,
        default_headers: Option<serde_json::Value>,
        config: Option<serde_json::Value>,
        priority: Option<i32>,
        status: Option<&str>,
    ) -> Result<ProviderConfig, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, ProviderConfig>(
                    r#"
                    UPDATE provider_configs
                    SET name = COALESCE($1, name),
                        api_base = CASE WHEN $2 IS NOT NULL THEN $3 ELSE api_base END,
                        api_key_enc = COALESCE($4, api_key_enc),
                        default_headers = COALESCE($5, default_headers),
                        config = COALESCE($6, config),
                        priority = COALESCE($7, priority),
                        status = COALESCE($8, status),
                        updated_at = NOW()
                    WHERE id = $9
                      AND org_id = $10
                      AND deleted_at IS NULL
                    RETURNING *
                    "#,
                )
                .bind(name)
                .bind(api_base.is_some())
                .bind(api_base.flatten())
                .bind(api_key_enc)
                .bind(default_headers)
                .bind(config)
                .bind(priority)
                .bind(status)
                .bind(id)
                .bind(org_id)
                .fetch_one(pg)
                .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, ProviderConfig>(
                    r#"
                    UPDATE provider_configs
                    SET name = COALESCE($1, name),
                        api_base = CASE WHEN $2 IS NOT NULL THEN $3 ELSE api_base END,
                        api_key_enc = COALESCE($4, api_key_enc),
                        default_headers = COALESCE($5, default_headers),
                        config = COALESCE($6, config),
                        priority = COALESCE($7, priority),
                        status = COALESCE($8, status),
                        updated_at = datetime('now')
                    WHERE id = $9
                      AND org_id = $10
                      AND deleted_at IS NULL
                    RETURNING *
                    "#,
                )
                .bind(name)
                .bind(api_base.is_some())
                .bind(api_base.flatten())
                .bind(api_key_enc)
                .bind(default_headers)
                .bind(config)
                .bind(priority)
                .bind(status)
                .bind(id)
                .bind(org_id)
                .fetch_one(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// Soft delete a provider config.
    pub async fn soft_delete(&self, id: Uuid, org_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE provider_configs
                    SET deleted_at = NOW(),
                        status = 'inactive',
                        updated_at = NOW()
                    WHERE id = $1
                      AND org_id = $2
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(id)
                .bind(org_id)
                .execute(pg)
                .await?;
                Ok(())
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE provider_configs
                    SET deleted_at = datetime('now'),
                        status = 'inactive',
                        updated_at = datetime('now')
                    WHERE id = $1
                      AND org_id = $2
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
                Ok(())
            }
        }
    }

    /// Update provider status and last error.
    pub async fn update_status(
        &self,
        id: Uuid,
        status: &str,
        last_error_msg: Option<&str>,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE provider_configs
                    SET status = $1,
                        last_error_at = CASE WHEN $2 IS NOT NULL THEN NOW() ELSE last_error_at END,
                        last_error_msg = $2,
                        updated_at = NOW()
                    WHERE id = $3
                    "#,
                )
                .bind(status)
                .bind(last_error_msg)
                .bind(id)
                .execute(pg)
                .await?;
                Ok(())
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE provider_configs
                    SET status = $1,
                        last_error_at = CASE WHEN $2 IS NOT NULL THEN datetime('now') ELSE last_error_at END,
                        last_error_msg = $2,
                        updated_at = datetime('now')
                    WHERE id = $3
                    "#,
                )
                .bind(status)
                .bind(last_error_msg)
                .bind(id)
                .execute(sqlite)
                .await?;
                Ok(())
            }
        }
    }
}
