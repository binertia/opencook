//! SQL dialect helpers for cross-backend compatibility.
//!
//! PostgreSQL and SQLite have different syntax for enums, timestamps,
//! and NULL-safe comparisons. This module provides portable SQL fragments.

use crate::pool::DbBackend;

/// Return the current timestamp function for the backend.
pub fn now(backend: &DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres(_) => "NOW()",
        DbBackend::Sqlite(_) => "datetime('now')",
    }
}

/// Cast a column to TEXT (for enum columns in PostgreSQL).
/// SQLite stores enums as plain TEXT, so no cast is needed.
pub fn cast_text(backend: &DbBackend, col: &str) -> String {
    match backend {
        DbBackend::Postgres(_) => format!("{}::text", col),
        DbBackend::Sqlite(_) => col.to_string(),
    }
}

/// Cast a bound parameter to an enum type (used in INSERT/UPDATE).
/// PostgreSQL: `$1::text::enum_type`
/// SQLite:     `$1`
pub fn cast_param(backend: &DbBackend, param: &str, _enum_type: &str) -> String {
    match backend {
        DbBackend::Postgres(_) => format!("{}::text::{}", param, _enum_type),
        DbBackend::Sqlite(_) => param.to_string(),
    }
}

/// Portable NULL-safe equality comparison.
/// PostgreSQL: `col IS NOT DISTINCT FROM val`
/// SQLite:     `(col = val) OR (col IS NULL AND val IS NULL)`
pub fn is_not_distinct_from(left: &str, right: &str) -> String {
    format!(
        "({left} = {right}) OR ({left} IS NULL AND {right} IS NULL)",
    )
}

/// Build a SELECT column list with optional ::text casts for enums.
///
/// Usage:
/// ```ignore
/// let cols = select_cols(&backend, &["id", "name", "status::text", "metric::text"]);
/// format!("SELECT {cols} FROM ...")
/// ```
pub fn select_cols(backend: &DbBackend, cols: &[&str]) -> String {
    cols.iter()
        .map(|c| {
            if let Some((base, _)) = c.split_once("::text") {
                cast_text(backend, base)
            } else {
                c.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
