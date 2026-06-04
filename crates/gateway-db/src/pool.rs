//! Database connection pool management with multi-backend support (PostgreSQL + SQLite).

use crate::error::DbError;
use sqlx::{
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqlitePool, SqlitePoolOptions},
};
use std::str::FromStr;
use std::time::Duration;

/// Default org ID for pool connections before tenant scoping.
pub const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Supported database backends.
#[derive(Debug, Clone)]
pub enum DbBackend {
    /// PostgreSQL backend (TEAM mode).
    Postgres(PgPool),
    /// SQLite backend (SOLO mode).
    Sqlite(SqlitePool),
}

impl DbBackend {
    /// Check if this is a PostgreSQL backend.
    pub fn is_postgres(&self) -> bool {
        matches!(self, DbBackend::Postgres(_))
    }

    /// Check if this is a SQLite backend.
    pub fn is_sqlite(&self) -> bool {
        matches!(self, DbBackend::Sqlite(_))
    }

    /// Get the PostgreSQL pool (panics if SQLite).
    pub fn pg(&self) -> &PgPool {
        match self {
            DbBackend::Postgres(pool) => pool,
            DbBackend::Sqlite(_) => panic!("Expected PostgreSQL backend, got SQLite"),
        }
    }

    /// Get the SQLite pool (panics if PostgreSQL).
    pub fn sqlite(&self) -> &SqlitePool {
        match self {
            DbBackend::Postgres(_) => panic!("Expected SQLite backend, got PostgreSQL"),
            DbBackend::Sqlite(pool) => pool,
        }
    }
}

/// Auto-detect backend from connection string and create pool.
///
/// - `postgres://...` → PostgreSQL pool with RLS context
/// - `sqlite://...` or file path → SQLite pool
/// - Default (no URL) → SQLite at `./data/gateway.db`
pub async fn create_pool(database_url: &str) -> Result<DbBackend, DbError> {
    if database_url.starts_with("postgres://") {
        create_postgres_pool(database_url).await.map(DbBackend::Postgres)
    } else {
        create_sqlite_pool(database_url).await.map(DbBackend::Sqlite)
    }
}

/// Create a PostgreSQL connection pool with tenant isolation support.
async fn create_postgres_pool(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .min_connections(5)
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
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

/// Create a SQLite connection pool.
///
/// Auto-creates the parent directory if it doesn't exist.
/// Runs `PRAGMA foreign_keys = ON` on every connection.
async fn create_sqlite_pool(database_url: &str) -> Result<SqlitePool, DbError> {
    // Normalize the URL: if it's just a path, prepend sqlite://
    let url = if database_url.starts_with("sqlite://") {
        database_url.to_string()
    } else {
        format!("sqlite://{}", database_url)
    };

    // Ensure parent directory exists
    if let Some(path) = url.strip_prefix("sqlite://") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| DbError::pool_init(format!("Failed to create data dir: {e}")))?;
            }
        }
    }

    // sqlx SQLite does not create the database file by default;
    // we need to explicitly enable it.
    let connect_options = sqlx::sqlite::SqliteConnectOptions::from_str(&url)
        .map_err(|e| DbError::pool_init(format!("Invalid SQLite URL: {e}")))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(connect_options)
        .await
        .map_err(|e| DbError::pool_init(format!("{e}")))?;

    // Initialize SQLite schema if needed
    init_sqlite_schema(&pool).await?;

    Ok(pool)
}

/// Initialize SQLite schema for SOLO mode.
///
/// Creates core tables if they don't exist. Simplified schema without:
/// - Partitioned tables (use regular tables)
/// - RLS policies (SQLite doesn't support them)
/// - Custom ENUMs (use TEXT with CHECK constraints)
async fn init_sqlite_schema(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS organizations (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'suspended')),
            settings TEXT NOT NULL DEFAULT '{}',
            billing_email TEXT,
            plan_tier TEXT NOT NULL DEFAULT 'free' CHECK (plan_tier IN ('free', 'starter', 'professional', 'enterprise')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS users (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            email TEXT NOT NULL,
            password_hash TEXT,
            display_name TEXT,
            role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'pending')),
            last_login_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS api_keys (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            user_id BLOB REFERENCES users(id),
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            key_prefix TEXT NOT NULL,
            scopes TEXT NOT NULL DEFAULT '[]',
            rate_limit_rps INTEGER NOT NULL DEFAULT 10,
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'revoked')),
            expires_at TEXT,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS provider_configs (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            api_base TEXT,
            api_key_enc BLOB,
            default_headers TEXT NOT NULL DEFAULT '{}',
            config TEXT NOT NULL DEFAULT '{}',
            priority INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'error')),
            last_error_at TEXT,
            last_error_msg TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS provider_models (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            provider_config_id BLOB NOT NULL REFERENCES provider_configs(id),
            model_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            aliases TEXT NOT NULL DEFAULT '[]',
            input_cost_per_1k TEXT NOT NULL DEFAULT '0',
            output_cost_per_1k TEXT NOT NULL DEFAULT '0',
            context_window INTEGER,
            max_tokens INTEGER,
            supports_streaming INTEGER NOT NULL DEFAULT 0,
            supports_tools INTEGER NOT NULL DEFAULT 0,
            supports_vision INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'deprecated')),
            config TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS routing_rules (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            name TEXT NOT NULL,
            description TEXT,
            strategy TEXT NOT NULL CHECK (strategy IN ('single', 'fallback', 'weighted', 'conditional')),
            priority INTEGER NOT NULL DEFAULT 0,
            match_model TEXT,
            match_tags TEXT NOT NULL DEFAULT '[]',
            conditions TEXT NOT NULL DEFAULT '{}',
            targets TEXT NOT NULL DEFAULT '{}',
            timeout_ms INTEGER NOT NULL DEFAULT 30000,
            retries INTEGER NOT NULL DEFAULT 1,
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS requests (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            api_key_id BLOB REFERENCES api_keys(id),
            user_id BLOB REFERENCES users(id),
            provider_config_id BLOB REFERENCES provider_configs(id),
            provider_model_id BLOB REFERENCES provider_models(id),
            routing_rule_id BLOB REFERENCES routing_rules(id),
            trace_id TEXT NOT NULL,
            parent_trace_id TEXT,
            method TEXT NOT NULL,
            path TEXT NOT NULL,
            model_requested TEXT,
            model_routed TEXT,
            request_headers TEXT NOT NULL DEFAULT '{}',
            request_body TEXT,
            request_body_truncated INTEGER NOT NULL DEFAULT 0,
            requested_at TEXT NOT NULL DEFAULT (datetime('now')),
            gateway_received_at TEXT NOT NULL DEFAULT (datetime('now')),
            provider_sent_at TEXT,
            provider_responded_at TEXT,
            completed_at TEXT,
            latency_gateway_ms INTEGER,
            latency_provider_ms INTEGER,
            latency_total_ms INTEGER,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            input_cost TEXT NOT NULL DEFAULT '0',
            output_cost TEXT NOT NULL DEFAULT '0',
            total_cost TEXT NOT NULL DEFAULT '0',
            status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'success', 'error', 'cancelled')),
            status_code INTEGER,
            error_code TEXT,
            error_message TEXT,
            metadata TEXT NOT NULL DEFAULT '{}',
            cache_hit INTEGER NOT NULL DEFAULT 0,
            cache_key_hash TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS quotas (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            api_key_id BLOB REFERENCES api_keys(id),
            name TEXT NOT NULL,
            description TEXT,
            metric TEXT NOT NULL CHECK (metric IN ('requests', 'tokens', 'cost_usd')),
            period TEXT NOT NULL CHECK (period IN ('minute', 'hour', 'day', 'month', 'total')),
            limit_value TEXT NOT NULL DEFAULT '0',
            warning_threshold TEXT NOT NULL DEFAULT '0.8',
            applies_to TEXT NOT NULL DEFAULT 'all' CHECK (applies_to IN ('all', 'api_key', 'model', 'provider')),
            scope_filter TEXT NOT NULL DEFAULT '{}',
            action TEXT NOT NULL DEFAULT 'block' CHECK (action IN ('block', 'warn', 'throttle')),
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS quota_usage (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            quota_id BLOB NOT NULL REFERENCES quotas(id),
            api_key_id BLOB REFERENCES api_keys(id),
            period_start TEXT NOT NULL,
            period_end TEXT NOT NULL,
            current_value TEXT NOT NULL DEFAULT '0',
            limit_value TEXT NOT NULL DEFAULT '0',
            metric TEXT NOT NULL,
            exceeded_at TEXT,
            warned_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        CREATE TABLE IF NOT EXISTS usage_records (
            id BLOB PRIMARY KEY NOT NULL DEFAULT (randomblob(16)),
            org_id BLOB NOT NULL REFERENCES organizations(id),
            api_key_id BLOB REFERENCES api_keys(id),
            provider_config_id BLOB REFERENCES provider_configs(id),
            provider_model_id BLOB REFERENCES provider_models(id),
            period TEXT NOT NULL,
            period_start TEXT NOT NULL,
            request_count INTEGER NOT NULL DEFAULT 0,
            request_success INTEGER NOT NULL DEFAULT 0,
            request_error INTEGER NOT NULL DEFAULT 0,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            input_cost TEXT NOT NULL DEFAULT '0',
            output_cost TEXT NOT NULL DEFAULT '0',
            total_cost TEXT NOT NULL DEFAULT '0',
            latency_ms_p50 INTEGER,
            latency_ms_p90 INTEGER,
            latency_ms_p99 INTEGER,
            latency_ms_avg INTEGER,
            cache_hits INTEGER NOT NULL DEFAULT 0,
            cache_misses INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );

        -- Seed default organization for SOLO mode
        INSERT OR IGNORE INTO organizations (id, name, slug, status, settings, billing_email, plan_tier)
        VALUES (X'00000000000000000000000000000000', 'Default Organization', 'default', 'active', '{}', NULL, 'free');

        -- Seed default user for SOLO mode
        INSERT OR IGNORE INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (X'00000000000000000000000000000000', X'00000000000000000000000000000000', 'admin@localhost', NULL, 'Admin', 'owner', 'active');
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| DbError::pool_init(format!("SQLite schema init failed: {e}")))?;

    Ok(())
}

/// Verify that the RLS context is correctly set on a PostgreSQL connection.
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

    fn test_pg_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| {
                "postgres://gateway:gateway_dev_password@localhost:5432/gateway_dev".into()
            })
    }

    #[tokio::test]
    async fn test_pg_pool_creation() {
        let pool = create_postgres_pool(&test_pg_url()).await.expect("pool creation failed");
        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("query failed");
        assert_eq!(row.0, 1);
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sqlite_pool_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let pool = create_sqlite_pool(&db_path.to_string_lossy())
            .await
            .expect("sqlite pool creation failed");

        // Verify schema was created
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizations")
            .fetch_one(&pool)
            .await
            .expect("query failed");
        assert_eq!(count.0, 1);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_rls_context_set() {
        let pool = create_postgres_pool(&test_pg_url()).await.expect("pool creation failed");
        let org_id = verify_rls_context(&pool).await.expect("RLS verification failed");
        assert_eq!(org_id, DEFAULT_ORG_ID);
        pool.close().await;
    }
}
