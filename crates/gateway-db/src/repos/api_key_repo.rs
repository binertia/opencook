//! API key repository.

use crate::error::DbError;
use crate::models::ApiKey;
use crate::pool::DbBackend;
use uuid::Uuid;

/// Repository for API keys.
#[derive(Clone)]
pub struct ApiKeyRepo {
    pool: DbBackend,
}

impl ApiKeyRepo {
    /// Create a new API key repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Find an active API key by its SHA-256 hash.
    pub async fn find_by_key_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, DbError> {
        let sql = r#"
            SELECT id, org_id, user_id, name, key_hash, key_prefix, scopes,
                   rate_limit_rps, status, expires_at, last_used_at,
                   created_at, updated_at, deleted_at
            FROM api_keys
            WHERE key_hash = $1
              AND status = 'active'
              AND deleted_at IS NULL
              AND (expires_at IS NULL OR expires_at > NOW())
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, ApiKey>(sql)
                    .bind(key_hash)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let sql = sql.replace("NOW()", "datetime('now')");
                let row = sqlx::query_as::<_, ApiKey>(&sql)
                    .bind(key_hash)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// List API keys for an organization.
    pub async fn list_by_org(&self, org_id: Uuid) -> Result<Vec<ApiKey>, DbError> {
        let sql = r#"
            SELECT id, org_id, user_id, name, key_hash, key_prefix, scopes,
                   rate_limit_rps, status, expires_at, last_used_at,
                   created_at, updated_at, deleted_at
            FROM api_keys
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, ApiKey>(sql)
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, ApiKey>(sql)
                    .bind(org_id)
                    .fetch_all(sqlite)
                    .await?;
                Ok(rows)
            }
        }
    }

    /// Get an API key by ID.
    pub async fn get_by_id(&self, org_id: Uuid, key_id: Uuid) -> Result<Option<ApiKey>, DbError> {
        let sql = r#"
            SELECT id, org_id, user_id, name, key_hash, key_prefix, scopes,
                   rate_limit_rps, status, expires_at, last_used_at,
                   created_at, updated_at, deleted_at
            FROM api_keys
            WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, ApiKey>(sql)
                    .bind(key_id)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, ApiKey>(sql)
                    .bind(key_id)
                    .bind(org_id)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// Create a new API key.
    pub async fn create(
        &self,
        org_id: Uuid,
        user_id: Option<Uuid>,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
        scopes: Vec<String>,
        rate_limit_rps: i32,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<ApiKey, DbError> {
        let key_id = Uuid::new_v4();
        let scopes_json = crate::types::JsonVec(scopes);
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    INSERT INTO api_keys (id, org_id, user_id, name, key_hash, key_prefix, scopes, rate_limit_rps, status, expires_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', $9)
                    "#,
                )
                .bind(key_id)
                .bind(org_id)
                .bind(user_id)
                .bind(name)
                .bind(key_hash)
                .bind(key_prefix)
                .bind(scopes_json)
                .bind(rate_limit_rps)
                .bind(expires_at)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    INSERT INTO api_keys (id, org_id, user_id, name, key_hash, key_prefix, scopes, rate_limit_rps, status, expires_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9)
                    "#,
                )
                .bind(key_id)
                .bind(org_id)
                .bind(user_id)
                .bind(name)
                .bind(key_hash)
                .bind(key_prefix)
                .bind(scopes_json)
                .bind(rate_limit_rps)
                .bind(expires_at)
                .execute(sqlite)
                .await?;
            }
        };

        self.get_by_id(org_id, key_id)
            .await?
            .ok_or_else(|| DbError::not_found("api_key", key_id))
    }

    /// Update an API key name or status.
    pub async fn update(
        &self,
        org_id: Uuid,
        key_id: Uuid,
        name: Option<&str>,
        status: Option<&str>,
    ) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    UPDATE api_keys
                    SET name = COALESCE($1, name),
                        status = COALESCE($2, status),
                        updated_at = NOW()
                    WHERE id = $3 AND org_id = $4
                    "#,
                )
                .bind(name)
                .bind(status)
                .bind(key_id)
                .bind(org_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    UPDATE api_keys
                    SET name = COALESCE(?1, name),
                        status = COALESCE(?2, status),
                        updated_at = datetime('now')
                    WHERE id = ?3 AND org_id = ?4
                    "#,
                )
                .bind(name)
                .bind(status)
                .bind(key_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Soft-delete an API key.
    pub async fn delete(&self, org_id: Uuid, key_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "UPDATE api_keys SET status = 'revoked', deleted_at = NOW() WHERE id = $1 AND org_id = $2",
                )
                .bind(key_id)
                .bind(org_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE api_keys SET status = 'revoked', deleted_at = datetime('now') WHERE id = ?1 AND org_id = ?2",
                )
                .bind(key_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }
}
