//! Database error types.

use thiserror::Error;

/// Unified database error type.
#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Pool initialization failed: {0}")]
    PoolInit(String),

    #[error("RLS context error: {0}")]
    RlsContext(String),

    #[error("Connection error: {0}")]
    Connection(String),
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
}
