//! Database error types.

use thiserror::Error;

/// Unified database error type.
#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database operation failed")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Pool initialization failed: {0}")]
    PoolInit(String),

    #[error("RLS context error: {0}")]
    RlsContext(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),
}

impl DbError {
    pub fn migration(msg: impl Into<String>) -> Self {
        Self::Migration(msg.into())
    }

    pub fn pool_init(msg: impl Into<String>) -> Self {
        Self::PoolInit(msg.into())
    }

    pub fn rls_context(msg: impl Into<String>) -> Self {
        Self::RlsContext(msg.into())
    }

    pub fn not_found(entity: impl Into<String>, id: impl std::fmt::Display) -> Self {
        Self::NotFound(format!("{} with id {} not found", entity.into(), id))
    }
}
