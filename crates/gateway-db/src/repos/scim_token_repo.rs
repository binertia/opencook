//! SCIM token repository.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// SCIM token record.
#[derive(Debug, Clone)]
pub struct ScimToken {
    pub id: Uuid,
    pub org_id: Uuid,
    pub token_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Repository for SCIM tokens.
#[derive(Clone)]
pub struct ScimTokenRepo {
    pool: PgPool,
}

impl ScimTokenRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, org_id: Uuid, token_hash: &str) -> Result<ScimToken, DbError> {
        let id = Uuid::new_v4();
        let row = sqlx::query_as::<_, ScimTokenRow>(
            r#"
            INSERT INTO scim_tokens (id, org_id, token_hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (org_id) DO UPDATE SET
                token_hash = EXCLUDED.token_hash,
                created_at = NOW()
            RETURNING id, org_id, token_hash, created_at, expires_at
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(token_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn find_by_hash(&self, token_hash: &str) -> Result<Option<ScimToken>, DbError> {
        let row = sqlx::query_as::<_, ScimTokenRow>(
            r#"
            SELECT id, org_id, token_hash, created_at, expires_at
            FROM scim_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn delete_by_org(&self, org_id: Uuid) -> Result<(), DbError> {
        sqlx::query("DELETE FROM scim_tokens WHERE org_id = $1")
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct ScimTokenRow {
    id: Uuid,
    org_id: Uuid,
    token_hash: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ScimTokenRow> for ScimToken {
    fn from(r: ScimTokenRow) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            token_hash: r.token_hash,
            created_at: r.created_at,
            expires_at: r.expires_at,
        }
    }
}
