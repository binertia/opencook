//! Organization repository.

use crate::error::DbError;
use crate::models::Organization;
use crate::pool::DbBackend;
use uuid::Uuid;

/// Repository for organizations.
#[derive(Clone)]
pub struct OrganizationRepo {
    pool: DbBackend,
}

impl OrganizationRepo {
    /// Create a new organization repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Find an organization by its slug.
    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, DbError> {
        let sql = r#"
            SELECT id, name, slug, status, settings, billing_email, plan_tier,
                   created_at, updated_at, deleted_at
            FROM organizations
            WHERE slug = $1
              AND deleted_at IS NULL
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, Organization>(sql)
                    .bind(slug)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, Organization>(sql.replace("$1", "?1").as_str())
                    .bind(slug)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// Find an organization by ID.
    pub async fn find_by_id(&self, org_id: Uuid) -> Result<Option<Organization>, DbError> {
        let sql = r#"
            SELECT id, name, slug, status, settings, billing_email, plan_tier,
                   created_at, updated_at, deleted_at
            FROM organizations
            WHERE id = $1
              AND deleted_at IS NULL
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, Organization>(sql)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, Organization>(sql.replace("$1", "?1").as_str())
                    .bind(org_id)
                    .fetch_optional(sqlite)
                    .await?;
                Ok(row)
            }
        }
    }

    /// Create a new organization within an explicit transaction.
    /// Returns the created organization.
    pub async fn create(
        &self,
        name: &str,
        slug: &str,
        billing_email: Option<&str>,
        plan_tier: &str,
    ) -> Result<Organization, DbError> {
        let org_id = Uuid::new_v4();
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    INSERT INTO organizations (id, name, slug, billing_email, plan_tier)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(slug)
                .bind(billing_email)
                .bind(plan_tier)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    INSERT INTO organizations (id, name, slug, billing_email, plan_tier)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                )
                .bind(org_id)
                .bind(name)
                .bind(slug)
                .bind(billing_email)
                .bind(plan_tier)
                .execute(sqlite)
                .await?;
            }
        };

        self.find_by_id(org_id)
            .await?
            .ok_or_else(|| DbError::not_found("organization", org_id))
    }
}
