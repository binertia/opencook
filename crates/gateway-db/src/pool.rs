//! PostgreSQL connection pool management with RLS context.

use crate::error::DbError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Default org ID for pool connections before tenant scoping.
pub const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Create a connection pool with tenant isolation support.
pub async fn create_pool(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .min_connections(5)
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // SET commands do not support parameter binding; use string interpolation.
                let sql = format!("SET app.org_id = '{}'", DEFAULT_ORG_ID);
                sqlx::query(&sql).execute(conn).await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
        .map_err(|e| DbError::pool_init(format!("{e}")))?;

    Ok(pool)
}

/// Verify that the RLS context is correctly set on a connection.
pub async fn verify_rls_context(pool: &PgPool) -> Result<String, DbError> {
    let org_id: String = sqlx::query_scalar("SELECT current_setting('app.org_id')")
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::rls_context(format!("{e}")))?;

    Ok(org_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| {
                "postgres://gateway:gateway_dev_password@localhost:5432/gateway_dev".into()
            })
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = create_pool(&test_db_url()).await.expect("pool creation failed");
        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("query failed");
        assert_eq!(row.0, 1);
        pool.close().await;
    }

    #[tokio::test]
    async fn test_rls_context_set() {
        let pool = create_pool(&test_db_url()).await.expect("pool creation failed");
        let org_id = verify_rls_context(&pool).await.expect("RLS verification failed");
        assert_eq!(org_id, DEFAULT_ORG_ID);
        pool.close().await;
    }
}
