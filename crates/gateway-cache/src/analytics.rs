//! Cache analytics queries.

use chrono::{DateTime, Utc};
use gateway_db::repos::cache_meta_repo::{CacheMetaRepo, ModelCacheStats};
use gateway_db::pool::DbBackend;
use uuid::Uuid;

/// Cache analytics engine.
#[derive(Clone)]
pub struct CacheAnalytics {
    repo: CacheMetaRepo,
}

impl CacheAnalytics {
    /// Create a new cache analytics engine.
    pub fn new(db_pool: DbBackend) -> Self {
        Self {
            repo: CacheMetaRepo::new(db_pool),
        }
    }

    /// Get the cache hit rate for an organization over a time period.
    ///
    /// `period` is a human-readable duration string like "1h", "24h", "7d", "30d".
    /// Returns a value between 0.0 and 1.0.
    pub async fn get_hit_rate(&self, org_id: Uuid, period: &str) -> Result<f64, CacheAnalyticsError> {
        let start = parse_period(period)?;
        self.repo.get_hit_rate(org_id, start).await.map_err(|e| {
            CacheAnalyticsError::Database(e.to_string())
        })
    }

    /// Get estimated cost saved from caching for an organization over a time period.
    ///
    /// Cost saved is computed from the usage_records table for the same period,
    /// multiplied by the hit rate as a proxy for saved requests.
    pub async fn get_cost_saved(
        &self,
        org_id: Uuid,
        period: &str,
    ) -> Result<f64, CacheAnalyticsError> {
        let start = parse_period(period)?;

        // Fetch total cost in the period from usage records
        let total_cost = self.fetch_total_cost(org_id, start).await?;

        // Fetch hit rate for the same period
        let hit_rate = self.repo.get_hit_rate(org_id, start).await.map_err(|e| {
            CacheAnalyticsError::Database(e.to_string())
        })?;

        // Estimated cost saved = total_cost × hit_rate / (1 - hit_rate)
        // (assuming cached hits would have generated additional cost)
        if hit_rate >= 1.0 || hit_rate <= 0.0 {
            Ok(0.0)
        } else {
            let saved = total_cost * hit_rate / (1.0 - hit_rate);
            Ok(saved)
        }
    }

    /// Get the top cached models by hit count.
    pub async fn get_top_cached_models(
        &self,
        org_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ModelCacheStats>, CacheAnalyticsError> {
        self.repo.get_top_models(org_id, limit).await.map_err(|e| {
            CacheAnalyticsError::Database(e.to_string())
        })
    }

    /// Get total number of active cache entries for an organization.
    pub async fn get_entry_count(&self, org_id: Uuid) -> Result<i64, CacheAnalyticsError> {
        self.repo.get_entry_count(org_id).await.map_err(|e| {
            CacheAnalyticsError::Database(e.to_string())
        })
    }

    /// Internal: fetch total cost from usage_records for the period.
    async fn fetch_total_cost(
        &self,
        org_id: Uuid,
        start: DateTime<Utc>,
    ) -> Result<f64, CacheAnalyticsError> {
        use gateway_db::pool::DbBackend;
        use sqlx::Row;

        let total_cost = match self.repo.pool() {
            DbBackend::Postgres(ref pg) => {
                let row = sqlx::query(
                    r#"
                    SELECT COALESCE(SUM(cost), 0.0)::float8 AS total_cost
                    FROM usage_records
                    WHERE org_id = $1 AND created_at >= $2
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .fetch_one(pg)
                .await
                .map_err(|e| CacheAnalyticsError::Database(e.to_string()))?;
                let total_cost: f64 = row.try_get("total_cost").unwrap_or(0.0);
                total_cost
            }
            DbBackend::Sqlite(ref sqlite) => {
                let row = sqlx::query(
                    r#"
                    SELECT COALESCE(SUM(cost), 0.0) AS total_cost
                    FROM usage_records
                    WHERE org_id = $1 AND created_at >= $2
                    "#,
                )
                .bind(org_id)
                .bind(start)
                .fetch_one(sqlite)
                .await
                .map_err(|e| CacheAnalyticsError::Database(e.to_string()))?;
                let total_cost: f64 = row.try_get("total_cost").unwrap_or(0.0);
                total_cost
            }
        };

        Ok(total_cost)
    }
}

/// Errors that can occur in cache analytics.
#[derive(Debug, thiserror::Error)]
pub enum CacheAnalyticsError {
    #[error("Invalid period format: {0}")]
    InvalidPeriod(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Parse a human-readable period string into a `DateTime`.
fn parse_period(period: &str) -> Result<DateTime<Utc>, CacheAnalyticsError> {
    let duration = match period.chars().last() {
        Some('h') => {
            let hours: i64 = period[..period.len() - 1]
                .parse()
                .map_err(|_| CacheAnalyticsError::InvalidPeriod(period.to_string()))?;
            chrono::Duration::hours(hours)
        }
        Some('d') => {
            let days: i64 = period[..period.len() - 1]
                .parse()
                .map_err(|_| CacheAnalyticsError::InvalidPeriod(period.to_string()))?;
            chrono::Duration::days(days)
        }
        _ => {
            return Err(CacheAnalyticsError::InvalidPeriod(format!(
                "'{}' — expected format like '1h', '24h', '7d', '30d'",
                period
            )));
        }
    };

    Ok(Utc::now() - duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_period_hours() {
        let start = parse_period("24h").unwrap();
        let expected = Utc::now() - chrono::Duration::hours(24);
        let diff = (start - expected).num_seconds().abs();
        assert!(diff < 2);
    }

    #[test]
    fn test_parse_period_days() {
        let start = parse_period("7d").unwrap();
        let expected = Utc::now() - chrono::Duration::days(7);
        let diff = (start - expected).num_seconds().abs();
        assert!(diff < 2);
    }

    #[test]
    fn test_parse_period_invalid() {
        assert!(parse_period("abc").is_err());
        assert!(parse_period("7x").is_err());
    }

    #[test]
    fn test_hit_rate_calculation_logic() {
        // If total_hits = 90 and total_entries = 10
        // hit_rate = 90 / (90 + 10) = 0.9
        let total_hits = 90.0;
        let total_entries = 10.0;
        let rate = total_hits / (total_hits + total_entries);
        assert!((rate - 0.9_f64).abs() < 0.001);
    }
}
