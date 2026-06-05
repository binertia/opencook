//! Organization membership repository (multi-org support).

use chrono::{DateTime, Utc};
use crate::error::DbError;
use crate::models::{Organization, UserOrganization};
use crate::pool::DbBackend;
use uuid::Uuid;

/// Repository for user-organization memberships.
#[derive(Clone)]
pub struct OrgMemberRepo {
    pool: DbBackend,
}

impl OrgMemberRepo {
    /// Create a new org member repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// List all organization memberships for a user.
    pub async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<UserOrganization>, DbError> {
        let sql = r#"
            SELECT user_id, org_id, role, joined_at, created_by
            FROM user_organizations
            WHERE user_id = $1
            ORDER BY joined_at DESC
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, UserOrganization>(sql)
                    .bind(user_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows)
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, UserOrganization>(
                    sql.replace("$1", "?1").as_str(),
                )
                .bind(user_id)
                .fetch_all(sqlite)
                .await?;
                Ok(rows)
            }
        }
    }

    /// Get a specific membership (used for switch-org validation).
    pub async fn get_membership(
        &self,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Option<UserOrganization>, DbError> {
        let sql = r#"
            SELECT user_id, org_id, role, joined_at, created_by
            FROM user_organizations
            WHERE user_id = $1 AND org_id = $2
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row = sqlx::query_as::<_, UserOrganization>(sql)
                    .bind(user_id)
                    .bind(org_id)
                    .fetch_optional(pg)
                    .await?;
                Ok(row)
            }
            DbBackend::Sqlite(sqlite) => {
                let row = sqlx::query_as::<_, UserOrganization>(
                    sql.replace("$1", "?1").replace("$2", "?2").as_str(),
                )
                .bind(user_id)
                .bind(org_id)
                .fetch_optional(sqlite)
                .await?;
                Ok(row)
            }
        }
    }

    /// List members of an organization with joined org details.
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<(UserOrganization, String, Option<String>)>, DbError> {
        let sql = r#"
            SELECT
                uo.user_id, uo.org_id, uo.role, uo.joined_at, uo.created_by,
                u.email AS user_email,
                u.display_name AS user_name
            FROM user_organizations uo
            JOIN users u ON uo.user_id = u.id
            WHERE uo.org_id = $1
            ORDER BY uo.joined_at DESC
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, OrgMemberRow>(sql)
                    .bind(org_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows.into_iter().map(|r| {
                    let email = r.user_email.clone();
                    let name = r.user_name.clone();
                    (r.to_membership(), email, name)
                }).collect())
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, OrgMemberRow>(
                    sql.replace("$1", "?1").as_str(),
                )
                .bind(org_id)
                .fetch_all(sqlite)
                .await?;
                Ok(rows.into_iter().map(|r| {
                    let email = r.user_email.clone();
                    let name = r.user_name.clone();
                    (r.to_membership(), email, name)
                }).collect())
            }
        }
    }

    /// Create a new membership.
    pub async fn create(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
        created_by: Option<Uuid>,
    ) -> Result<UserOrganization, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    INSERT INTO user_organizations (user_id, org_id, role, created_by)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (user_id, org_id) DO UPDATE SET role = EXCLUDED.role
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(role)
                .bind(created_by)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    INSERT INTO user_organizations (user_id, org_id, role, created_by)
                    VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT (user_id, org_id) DO UPDATE SET role = excluded.role
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(role)
                .bind(created_by)
                .execute(sqlite)
                .await?;
            }
        };

        self.get_membership(user_id, org_id)
            .await?
            .ok_or_else(|| DbError::not_found("user_organization", user_id))
    }

    /// Remove a membership (hard delete — user left the org).
    pub async fn delete(&self, user_id: Uuid, org_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "DELETE FROM user_organizations WHERE user_id = $1 AND org_id = $2",
                )
                .bind(user_id)
                .bind(org_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "DELETE FROM user_organizations WHERE user_id = ?1 AND org_id = ?2",
                )
                .bind(user_id)
                .bind(org_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Get organizations a user belongs to.
    pub async fn list_orgs_for_user(&self, user_id: Uuid) -> Result<Vec<(Organization, String)>, DbError> {
        let sql = r#"
            SELECT
                o.id, o.name, o.slug, o.status, o.settings, o.billing_email, o.plan_tier,
                o.created_at, o.updated_at, o.deleted_at,
                uo.role AS membership_role
            FROM organizations o
            JOIN user_organizations uo ON o.id = uo.org_id
            WHERE uo.user_id = $1
              AND o.deleted_at IS NULL
            ORDER BY o.created_at DESC
            "#;
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let rows = sqlx::query_as::<_, OrgWithRole>(sql)
                    .bind(user_id)
                    .fetch_all(pg)
                    .await?;
                Ok(rows.into_iter().map(|r| {
                    let role = r.membership_role.clone();
                    (r.to_org(), role)
                }).collect())
            }
            DbBackend::Sqlite(sqlite) => {
                let rows = sqlx::query_as::<_, OrgWithRole>(
                    sql.replace("$1", "?1").as_str(),
                )
                .bind(user_id)
                .fetch_all(sqlite)
                .await?;
                Ok(rows.into_iter().map(|r| {
                    let role = r.membership_role.clone();
                    (r.to_org(), role)
                }).collect())
            }
        }
    }
}

// Internal helper row for joined member query.
#[derive(sqlx::FromRow)]
struct OrgMemberRow {
    user_id: Uuid,
    org_id: Uuid,
    role: String,
    joined_at: DateTime<Utc>,
    created_by: Option<Uuid>,
    user_email: String,
    user_name: Option<String>,
}

impl OrgMemberRow {
    fn to_membership(&self) -> UserOrganization {
        UserOrganization {
            user_id: self.user_id,
            org_id: self.org_id,
            role: self.role.clone(),
            joined_at: self.joined_at,
            created_by: self.created_by,
        }
    }
}

// Internal helper row for org-with-role query.
#[derive(sqlx::FromRow)]
struct OrgWithRole {
    id: Uuid,
    name: String,
    slug: String,
    status: String,
    settings: serde_json::Value,
    billing_email: Option<String>,
    plan_tier: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    membership_role: String,
}

impl OrgWithRole {
    fn to_org(&self) -> Organization {
        Organization {
            id: self.id,
            name: self.name.clone(),
            slug: self.slug.clone(),
            status: self.status.clone(),
            settings: self.settings.clone(),
            billing_email: self.billing_email.clone(),
            plan_tier: self.plan_tier.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::create_pool;
    use tempfile::TempDir;

    async fn setup_sqlite() -> (DbBackend, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let pool = create_pool(&db_path.to_string_lossy()).await.expect("sqlite pool creation failed");
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_membership_crud() {
        let (pool, _tmp) = setup_sqlite().await;
        let repo = OrgMemberRepo::new(pool.clone());
        let org_repo = crate::repos::organization_repo::OrganizationRepo::new(pool.clone());
        let user_repo = crate::repos::user_repo::UserRepo::new(pool);

        // Seed org and user
        let org = org_repo.create("Test Org", "test-org", None, "free").await.unwrap();
        let user = user_repo.create(org.id, "test@example.com", Some("hash"), Some("Test User"), "member", "active").await.unwrap();

        // Create membership
        let membership = repo.create(user.id, org.id, "admin", Some(user.id)).await.unwrap();
        assert_eq!(membership.user_id, user.id);
        assert_eq!(membership.org_id, org.id);
        assert_eq!(membership.role, "admin");

        // Get membership
        let found = repo.get_membership(user.id, org.id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.role, "admin");

        // List by user
        let orgs = repo.list_orgs_for_user(user.id).await.unwrap();
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].0.name, "Test Org");
        assert_eq!(orgs[0].1, "admin");

        // List by org
        let members = repo.list_by_org(org.id).await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].0.role, "admin");
        assert_eq!(members[0].1, "test@example.com");

        // Delete membership
        repo.delete(user.id, org.id).await.unwrap();
        let found = repo.get_membership(user.id, org.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_switch_org_validation() {
        let (pool, _tmp) = setup_sqlite().await;
        let repo = OrgMemberRepo::new(pool.clone());
        let org_repo = crate::repos::organization_repo::OrganizationRepo::new(pool.clone());
        let user_repo = crate::repos::user_repo::UserRepo::new(pool);

        let org_a = org_repo.create("Org A", "org-a", None, "free").await.unwrap();
        let org_b = org_repo.create("Org B", "org-b", None, "free").await.unwrap();
        let user = user_repo.create(org_a.id, "user@example.com", Some("hash"), Some("User"), "member", "active").await.unwrap();

        // User is only in org A (via legacy user.org_id + migration not run in test)
        // Manually add membership to org A
        repo.create(user.id, org_a.id, "member", Some(user.id)).await.unwrap();

        // Should succeed for org A
        let m = repo.get_membership(user.id, org_a.id).await.unwrap();
        assert!(m.is_some());

        // Should fail for org B
        let m = repo.get_membership(user.id, org_b.id).await.unwrap();
        assert!(m.is_none());
    }
}
