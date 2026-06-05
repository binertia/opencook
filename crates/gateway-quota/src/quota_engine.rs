//! Quota checking engine with budget caps.

use chrono::Utc;
use rust_decimal::Decimal;
use tracing::{debug, warn};

use crate::models::{QuotaMetric, QuotaPeriod, QuotaResult, RequestContext};
use gateway_db::models::Quota;
use gateway_db::{QuotaRepo, QuotaUsageRepo};

/// Quota engine: checks quotas and enforces budget caps.
#[derive(Clone)]
pub struct QuotaEngine {
    quota_repo: QuotaRepo,
    usage_repo: QuotaUsageRepo,
}

impl QuotaEngine {
    /// Create a new quota engine.
    pub fn new(quota_repo: QuotaRepo, usage_repo: QuotaUsageRepo) -> Self {
        Self {
            quota_repo,
            usage_repo,
        }
    }

    /// Check all quotas for a request context.
    ///
    /// Returns the most restrictive result:
    /// - If any quota is exceeded → Exceeded
    /// - If any quota is in warning → Warning (with lowest remaining)
    /// - Otherwise → Allowed
    pub async fn check_quota(&self, context: &RequestContext) -> QuotaResult {
        let quotas = match self
            .quota_repo
            .find_active_for_context(context.org_id, context.api_key_id)
            .await
        {
            Ok(q) => q,
            Err(e) => {
                warn!(error = %e, "Failed to load quotas, failing open");
                return QuotaResult::Allowed {
                    remaining: f64::INFINITY,
                    limit: f64::INFINITY,
                };
            }
        };

        if quotas.is_empty() {
            debug!(org_id = %context.org_id, "No quotas configured, allowing request");
            return QuotaResult::Allowed {
                remaining: f64::INFINITY,
                limit: f64::INFINITY,
            };
        }

        let now = Utc::now();
        let mut most_restrictive: Option<QuotaResult> = None;

        for quota in quotas {
            let result = self.check_single_quota(quota, context, now).await;

            match &result {
                QuotaResult::Exceeded { .. } => return result,
                QuotaResult::Warning { remaining, .. } => {
                    if let Some(QuotaResult::Warning {
                        remaining: best, ..
                    }) = most_restrictive
                    {
                        if *remaining < best {
                            most_restrictive = Some(result);
                        }
                    } else {
                        most_restrictive = Some(result);
                    }
                }
                QuotaResult::Allowed { remaining: _, .. } => {
                    if most_restrictive.is_none() {
                        most_restrictive = Some(result);
                    }
                }
            }
        }

        most_restrictive.unwrap_or(QuotaResult::Allowed {
            remaining: f64::INFINITY,
            limit: f64::INFINITY,
        })
    }

    /// Check a single quota definition against the request context.
    async fn check_single_quota(
        &self,
        quota: Quota,
        context: &RequestContext,
        now: chrono::DateTime<Utc>,
    ) -> QuotaResult {
        let metric = match quota.metric.parse::<QuotaMetric>() {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "Unknown quota metric, skipping");
                return QuotaResult::Allowed {
                    remaining: f64::INFINITY,
                    limit: f64::INFINITY,
                };
            }
        };

        let period = match quota.period.parse::<QuotaPeriod>() {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "Unknown quota period, skipping");
                return QuotaResult::Allowed {
                    remaining: f64::INFINITY,
                    limit: f64::INFINITY,
                };
            }
        };

        // Compute estimated usage for this quota
        let estimated_usage = match metric {
            QuotaMetric::Requests => Decimal::from(1),
            QuotaMetric::Tokens => Decimal::from(context.estimated_tokens as i64),
            QuotaMetric::CostUsd => Decimal::try_from(context.estimated_cost).unwrap_or_default(),
        };

        let limit_value = quota.limit_value;
        let warning_threshold = quota.warning_threshold;

        // Compute period boundaries
        let period_start = period.period_start(now);
        let period_end = period.period_end(period_start);

        // Get or create usage record
        let usage = match self
            .usage_repo
            .get_or_create(
                quota.org_id,
                quota.id,
                quota.api_key_id,
                period_start,
                period_end,
                limit_value,
                &quota.metric,
            )
            .await
        {
            Ok(u) => u,
            Err(e) => {
                warn!(error = %e, "Failed to get quota usage, failing open");
                return QuotaResult::Allowed {
                    remaining: f64::INFINITY,
                    limit: f64::INFINITY,
                };
            }
        };

        let current = usage.current_value;
        let projected = current + estimated_usage;

        // Check if already exceeded
        if usage.exceeded_at.is_some() || projected > limit_value {
            debug!(
                quota_id = %quota.id,
                metric = %quota.metric,
                current = %current,
                projected = %projected,
                limit = %limit_value,
                "Quota exceeded"
            );

            // Mark as exceeded if not already
            if usage.exceeded_at.is_none() {
                let _ = self
                    .usage_repo
                    .mark_exceeded(quota.org_id, quota.id, quota.api_key_id, period_start)
                    .await;
            }

            return QuotaResult::Exceeded {
                metric: quota.metric,
                limit: limit_value.try_into().unwrap_or(f64::MAX),
            };
        }

        // Check warning threshold
        let threshold_value = limit_value * (warning_threshold / Decimal::from(100));
        if projected >= threshold_value {
            let remaining = (limit_value - projected)
                .try_into()
                .unwrap_or(0.0);

            debug!(
                quota_id = %quota.id,
                metric = %quota.metric,
                current = %current,
                projected = %projected,
                threshold = %threshold_value,
                "Quota warning"
            );

            // Mark as warned if not already
            if usage.warned_at.is_none() {
                let _ = self
                    .usage_repo
                    .mark_warned(quota.org_id, quota.id, quota.api_key_id, period_start)
                    .await;
            }

            return QuotaResult::Warning {
                threshold: warning_threshold.try_into().unwrap_or(80.0),
                remaining,
            };
        }

        // Within limit
        let remaining = (limit_value - projected)
            .try_into()
            .unwrap_or(0.0);
        let limit = limit_value.try_into().unwrap_or(f64::MAX);

        QuotaResult::Allowed { remaining, limit }
    }

    /// Record actual usage after a request completes.
    ///
    /// This should be called post-request to update the actual usage.
    pub async fn record_usage(
        &self,
        org_id: uuid::Uuid,
        api_key_id: Option<uuid::Uuid>,
        metric: QuotaMetric,
        amount: Decimal,
    ) -> Result<(), gateway_db::error::DbError> {
        // For now, we increment usage for all active quotas matching the metric.
        // In production, you'd want to match the exact quota.
        let quotas = self.quota_repo.find_active_for_context(org_id, api_key_id).await?;
        let now = Utc::now();

        for quota in quotas {
            if quota.metric != metric.as_str() {
                continue;
            }

            let period = match quota.period.parse::<QuotaPeriod>() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let period_start = period.period_start(now);
            let period_end = period.period_end(period_start);

            // Ensure record exists
            let _ = self
                .usage_repo
                .get_or_create(
                    org_id,
                    quota.id,
                    api_key_id,
                    period_start,
                    period_end,
                    quota.limit_value,
                    &quota.metric,
                )
                .await?;

            // Increment
            let _ = self
                .usage_repo
                .increment(org_id, quota.id, api_key_id, period_start, amount)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_period_boundaries() {
        let dt = chrono::DateTime::parse_from_rfc3339("2024-01-15T14:32:45Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            QuotaPeriod::Minute.period_start(dt),
            chrono::DateTime::parse_from_rfc3339("2024-01-15T14:32:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );

        assert_eq!(
            QuotaPeriod::Hour.period_start(dt),
            chrono::DateTime::parse_from_rfc3339("2024-01-15T14:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );

        assert_eq!(
            QuotaPeriod::Day.period_start(dt),
            chrono::DateTime::parse_from_rfc3339("2024-01-15T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn test_quota_metric_parsing() {
        assert_eq!(
            "requests".parse::<QuotaMetric>().unwrap(),
            QuotaMetric::Requests
        );
        assert_eq!("tokens".parse::<QuotaMetric>().unwrap(), QuotaMetric::Tokens);
        assert_eq!(
            "cost_usd".parse::<QuotaMetric>().unwrap(),
            QuotaMetric::CostUsd
        );
        assert!("unknown".parse::<QuotaMetric>().is_err());
    }
}
