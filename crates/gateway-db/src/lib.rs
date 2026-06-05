//! Gateway DB — Database connection pooling, migrations, and repositories.

pub mod dialect;
pub mod error;
pub mod migrations;
pub mod models;
pub mod pool;
pub mod repos;
pub mod repositories;
pub mod types;

pub use error::DbError;
pub use models::{
    ApiKey, AuditAction, AuditEntry, Capabilities, ModelEntry, Organization, PricingInfo,
    ProviderConfig, ProviderModel, Quota, QuotaUsage, Request, RoutingRule, Target, User,
    UserOrganization, Webhook, WebhookDelivery, WebhookEvent,
};
pub use pool::{create_pool, verify_rls_context, DbBackend, DEFAULT_ORG_ID};
pub use repos::api_key_repo::ApiKeyRepo;
pub use repos::model_registry::ModelRegistry;
pub use repos::org_member_repo::OrgMemberRepo;
pub use repos::organization_repo::OrganizationRepo;
pub use repos::quota_repo::QuotaRepo;
pub use repos::quota_usage_repo::QuotaUsageRepo;
pub use repos::request_repo::{RequestRepo, RequestStats};
pub use repos::routing_repo::RoutingRepo;
pub use repos::usage_repo::UsageRepo;
pub use repos::user_repo::UserRepo;
pub use repos::webhook_repo::WebhookRepo;
pub use repos::webhook_delivery_repo::WebhookDeliveryRepo;
pub use types::{DbDecimal, JsonVec};

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

#[async_trait::async_trait]
impl MigrationRunner for DbBackend {
    async fn run_migrations(&self) -> Result<(), DbError> {
        match self {
            DbBackend::Postgres(pg) => pg.run_migrations().await,
            DbBackend::Sqlite(_) => {
                // SQLite schema is initialized automatically on pool creation.
                // No external migrations needed for SOLO mode.
                Ok(())
            }
        }
    }
}
