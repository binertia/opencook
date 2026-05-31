//! Gateway DB — Database connection pooling, migrations, and repositories.

pub mod error;
pub mod migrations;
pub mod models;
pub mod pool;
pub mod repos;
pub mod repositories;

pub use error::DbError;
pub use models::{ApiKey, Organization, Quota, QuotaUsage, Request, User};
pub use pool::{create_pool, verify_rls_context, DEFAULT_ORG_ID};
pub use repos::quota_repo::QuotaRepo;
pub use repos::quota_usage_repo::QuotaUsageRepo;
pub use repos::request_repo::RequestRepo;

use sqlx::PgPool;

/// Trait for running database migrations.
#[async_trait::async_trait]
pub trait MigrationRunner {
    /// Run all pending migrations.
    async fn run_migrations(&self) -> Result<(), DbError>;
}

#[async_trait::async_trait]
impl MigrationRunner for PgPool {
    async fn run_migrations(&self) -> Result<(), DbError> {
        sqlx::migrate!("../../migrations")
            .run(self)
            .await
            .map_err(|e| DbError::migration(format!("{e}")))?;
        Ok(())
    }
}
