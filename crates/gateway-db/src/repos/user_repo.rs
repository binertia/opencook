//! User repository.

use crate::error::DbError;
use crate::models::User;
use crate::pool::DbBackend;
use uuid::Uuid;

/// Repository for dashboard users.
#[derive(Clone)]
pub struct UserRepo {
    pool: DbBackend,
}

impl UserRepo {
    /// Create a new user repository.
    pub fn new(pool: DbBackend) -> Self {
        Self { pool }
    }

    /// Find a user by email address.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, User>(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE email = $1
                      AND status = 'active'
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(email)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, User>(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE email = ?1
                      AND status = 'active'
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(email)
                .fetch_optional(sqlite)
                .await?
            }
        };
        Ok(row)
    }

    /// Find a user by ID.
    pub async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>, DbError> {
        let row = match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query_as::<_, User>(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE id = $1
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(user_id)
                .fetch_optional(pg)
                .await?
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query_as::<_, User>(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE id = ?1
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(user_id)
                .fetch_optional(sqlite)
                .await?
            }
        };
        Ok(row)
    }

    /// Update a user's last login timestamp.
    pub async fn update_last_login(&self, user_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
                    .bind(user_id)
                    .execute(pg)
                    .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query("UPDATE users SET last_login_at = datetime('now') WHERE id = ?1")
                    .bind(user_id)
                    .execute(sqlite)
                    .await?;
            }
        };
        Ok(())
    }

    /// List users for an organization with pagination.
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        search: Option<&str>,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<User>, i64), DbError> {
        let (rows, total) = match &self.pool {
            DbBackend::Postgres(pg) => {
                // Count total
                let mut count_query = String::from(
                    "SELECT COUNT(*) FROM users WHERE org_id = $1 AND deleted_at IS NULL",
                );
                if search.is_some() {
                    count_query.push_str(" AND (email ILIKE $2 OR display_name ILIKE $2)");
                }
                if status.is_some() && status != Some("all") {
                    count_query.push_str(" AND status = $3");
                }
                let mut cq = sqlx::query_scalar::<_, i64>(&count_query).bind(org_id);
                if let Some(s) = search {
                    cq = cq.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        cq = cq.bind(st);
                    }
                }
                let total = cq.fetch_one(pg).await?;

                // Fetch page
                let mut query = String::from(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE org_id = $1
                      AND deleted_at IS NULL
                    "#,
                );
                let mut param_idx = 2u32;
                if search.is_some() {
                    query.push_str(&format!(
                        " AND (email ILIKE ${} OR display_name ILIKE ${})",
                        param_idx, param_idx
                    ));
                    param_idx += 1;
                }
                if status.is_some() && status != Some("all") {
                    query.push_str(&format!(" AND status = ${}", param_idx));
                    param_idx += 1;
                }
                query.push_str(&format!(
                    " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
                    param_idx,
                    param_idx + 1
                ));

                let mut q = sqlx::query_as::<_, User>(&query).bind(org_id);
                if let Some(s) = search {
                    q = q.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        q = q.bind(st);
                    }
                }
                let rows = q.bind(limit).bind(offset).fetch_all(pg).await?;
                (rows, total)
            }
            DbBackend::Sqlite(sqlite) => {
                // Count total
                let mut count_query = String::from(
                    "SELECT COUNT(*) FROM users WHERE org_id = ?1 AND deleted_at IS NULL",
                );
                if search.is_some() {
                    count_query.push_str(" AND (email LIKE ?2 OR display_name LIKE ?2)");
                }
                if status.is_some() && status != Some("all") {
                    count_query.push_str(" AND status = ?3");
                }
                let mut cq = sqlx::query_scalar::<_, i64>(&count_query).bind(org_id);
                if let Some(s) = search {
                    cq = cq.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        cq = cq.bind(st);
                    }
                }
                let total = cq.fetch_one(sqlite).await?;

                // Fetch page
                let mut query = String::from(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           failed_login_attempts, locked_until,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE org_id = ?1
                      AND deleted_at IS NULL
                    "#,
                );
                if search.is_some() {
                    query.push_str(" AND (email LIKE ?2 OR display_name LIKE ?2)");
                }
                if status.is_some() && status != Some("all") {
                    query.push_str(" AND status = ?3");
                }
                query.push_str(" ORDER BY created_at DESC LIMIT ?4 OFFSET ?5");

                let mut q = sqlx::query_as::<_, User>(&query).bind(org_id);
                if let Some(s) = search {
                    q = q.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        q = q.bind(st);
                    }
                }
                let rows = q.bind(limit).bind(offset).fetch_all(sqlite).await?;
                (rows, total)
            }
        };
        Ok((rows, total))
    }

    /// Create a new user.
    pub async fn create(
        &self,
        org_id: Uuid,
        email: &str,
        password_hash: Option<&str>,
        display_name: Option<&str>,
        role: &str,
        status: &str,
    ) -> Result<User, DbError> {
        let user_id = Uuid::new_v4();
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(email)
                .bind(password_hash)
                .bind(display_name)
                .bind(role)
                .bind(status)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(email)
                .bind(password_hash)
                .bind(display_name)
                .bind(role)
                .bind(status)
                .execute(sqlite)
                .await?;
            }
        };

        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| DbError::not_found("user", user_id))
    }

    /// Create a new SSO user (no password, sso status).
    pub async fn create_sso_user(
        &self,
        org_id: Uuid,
        email: &str,
        display_name: Option<&str>,
    ) -> Result<User, DbError> {
        let user_id = Uuid::new_v4();
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
                    VALUES ($1, $2, $3, NULL, $4, 'member', 'active')
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(email)
                .bind(display_name)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    r#"
                    INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
                    VALUES (?1, ?2, ?3, NULL, ?4, 'member', 'active')
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(email)
                .bind(display_name)
                .execute(sqlite)
                .await?;
            }
        };

        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| DbError::not_found("user", user_id))
    }

    /// Update a user's status.
    pub async fn update_status(&self, user_id: Uuid, status: &str) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query("UPDATE users SET status = $1, updated_at = NOW() WHERE id = $2")
                    .bind(status)
                    .bind(user_id)
                    .execute(pg)
                    .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
                )
                .bind(status)
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Update a user's role.
    pub async fn update_role(&self, user_id: Uuid, role: &str) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query("UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2")
                    .bind(role)
                    .bind(user_id)
                    .execute(pg)
                    .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET role = ?1, updated_at = datetime('now') WHERE id = ?2",
                )
                .bind(role)
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Increment failed login attempts and return the new count.
    pub async fn increment_failed_login(&self, user_id: Uuid) -> Result<i32, DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                let row: (i32,) = sqlx::query_as(
                    "UPDATE users SET failed_login_attempts = failed_login_attempts + 1, updated_at = NOW() WHERE id = $1 RETURNING failed_login_attempts",
                )
                .bind(user_id)
                .fetch_one(pg)
                .await?;
                Ok(row.0)
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET failed_login_attempts = failed_login_attempts + 1, updated_at = datetime('now') WHERE id = ?1",
                )
                .bind(user_id)
                .execute(sqlite)
                .await?;
                let row: (i32,) =
                    sqlx::query_as("SELECT failed_login_attempts FROM users WHERE id = ?1")
                        .bind(user_id)
                        .fetch_one(sqlite)
                        .await?;
                Ok(row.0)
            }
        }
    }

    /// Reset failed login attempts to 0 on successful login.
    pub async fn reset_failed_login(&self, user_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "UPDATE users SET failed_login_attempts = 0, locked_until = NULL, updated_at = NOW() WHERE id = $1",
                )
                .bind(user_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET failed_login_attempts = 0, locked_until = NULL, updated_at = datetime('now') WHERE id = ?1",
                )
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Lock an account for a specified duration.
    pub async fn lock_account(&self, user_id: Uuid, minutes: i64) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "UPDATE users SET locked_until = NOW() + INTERVAL '1 minute' * $1, updated_at = NOW() WHERE id = $2",
                )
                .bind(minutes)
                .bind(user_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET locked_until = datetime('now', '+' || ?1 || ' minutes'), updated_at = datetime('now') WHERE id = ?2",
                )
                .bind(minutes)
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Update a user's password hash.
    pub async fn update_password(&self, user_id: Uuid, password_hash: &str) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "UPDATE users SET password_hash = $1, failed_login_attempts = 0, locked_until = NULL, updated_at = NOW() WHERE id = $2",
                )
                .bind(password_hash)
                .bind(user_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET password_hash = ?1, failed_login_attempts = 0, locked_until = NULL, updated_at = datetime('now') WHERE id = ?2",
                )
                .bind(password_hash)
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
    }

    /// Soft-delete a user.
    pub async fn delete(&self, user_id: Uuid) -> Result<(), DbError> {
        match &self.pool {
            DbBackend::Postgres(pg) => {
                sqlx::query(
                    "UPDATE users SET status = 'inactive', deleted_at = NOW() WHERE id = $1",
                )
                .bind(user_id)
                .execute(pg)
                .await?;
            }
            DbBackend::Sqlite(sqlite) => {
                sqlx::query(
                    "UPDATE users SET status = 'inactive', deleted_at = datetime('now') WHERE id = ?1",
                )
                .bind(user_id)
                .execute(sqlite)
                .await?;
            }
        };
        Ok(())
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
        let pool = create_pool(&db_path.to_string_lossy())
            .await
            .expect("sqlite pool creation failed");
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_increment_and_reset_failed_login() {
        let (pool, _tmp) = setup_sqlite().await;
        let repo = UserRepo::new(pool.clone());
        let org_repo = crate::repos::organization_repo::OrganizationRepo::new(pool);

        let org = org_repo
            .create("Test Org", "test-org", None, "free")
            .await
            .unwrap();
        let user = repo
            .create(
                org.id,
                "test@example.com",
                Some("hash"),
                Some("Test"),
                "member",
                "active",
            )
            .await
            .unwrap();

        // Initially 0
        assert_eq!(user.failed_login_attempts, 0);

        // Increment
        let attempts = repo.increment_failed_login(user.id).await.unwrap();
        assert_eq!(attempts, 1);

        let attempts = repo.increment_failed_login(user.id).await.unwrap();
        assert_eq!(attempts, 2);

        // Reset
        repo.reset_failed_login(user.id).await.unwrap();
        let user = repo.find_by_id(user.id).await.unwrap().unwrap();
        assert_eq!(user.failed_login_attempts, 0);
        assert!(user.locked_until.is_none());
    }

    #[tokio::test]
    async fn test_lock_account() {
        let (pool, _tmp) = setup_sqlite().await;
        let repo = UserRepo::new(pool.clone());
        let org_repo = crate::repos::organization_repo::OrganizationRepo::new(pool);

        let org = org_repo
            .create("Test Org", "test-org", None, "free")
            .await
            .unwrap();
        let user = repo
            .create(
                org.id,
                "test@example.com",
                Some("hash"),
                Some("Test"),
                "member",
                "active",
            )
            .await
            .unwrap();

        // Lock for 30 minutes
        repo.lock_account(user.id, 30).await.unwrap();
        let user = repo.find_by_id(user.id).await.unwrap().unwrap();
        assert!(user.locked_until.is_some());
    }

    #[tokio::test]
    async fn test_update_password_clears_lockout() {
        let (pool, _tmp) = setup_sqlite().await;
        let repo = UserRepo::new(pool.clone());
        let org_repo = crate::repos::organization_repo::OrganizationRepo::new(pool);

        let org = org_repo
            .create("Test Org", "test-org", None, "free")
            .await
            .unwrap();
        let user = repo
            .create(
                org.id,
                "test@example.com",
                Some("old_hash"),
                Some("Test"),
                "member",
                "active",
            )
            .await
            .unwrap();

        // Set some failed attempts and lock
        repo.increment_failed_login(user.id).await.unwrap();
        repo.lock_account(user.id, 30).await.unwrap();

        // Update password
        repo.update_password(user.id, "new_hash").await.unwrap();
        let user = repo.find_by_id(user.id).await.unwrap().unwrap();
        assert_eq!(user.password_hash, Some("new_hash".to_string()));
        assert_eq!(user.failed_login_attempts, 0);
        assert!(user.locked_until.is_none());
    }
}
