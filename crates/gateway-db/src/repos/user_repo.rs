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

    /// List users for an organization.
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<User>, DbError> {
        let rows = match &self.pool {
            DbBackend::Postgres(pg) => {
                let mut query = String::from(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
                           created_at, updated_at, deleted_at
                    FROM users
                    WHERE org_id = $1
                      AND deleted_at IS NULL
                    "#,
                );
                if search.is_some() {
                    query.push_str(" AND (email ILIKE $2 OR display_name ILIKE $2)");
                }
                if status.is_some() && status != Some("all") {
                    query.push_str(" AND status = $3");
                }
                query.push_str(" ORDER BY created_at DESC");

                let mut q = sqlx::query_as::<_, User>(&query).bind(org_id);
                if let Some(s) = search {
                    q = q.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        q = q.bind(st);
                    }
                }
                q.fetch_all(pg).await?
            }
            DbBackend::Sqlite(sqlite) => {
                let mut query = String::from(
                    r#"
                    SELECT id, org_id, email, password_hash, display_name,
                           role, status, last_login_at,
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
                query.push_str(" ORDER BY created_at DESC");

                let mut q = sqlx::query_as::<_, User>(&query).bind(org_id);
                if let Some(s) = search {
                    q = q.bind(format!("%{}%", s));
                }
                if let Some(st) = status {
                    if st != "all" {
                        q = q.bind(st);
                    }
                }
                q.fetch_all(sqlite).await?
            }
        };
        Ok(rows)
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
