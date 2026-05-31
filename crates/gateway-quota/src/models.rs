//! Quota engine types.

use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// Result of a quota check.
#[derive(Debug, Clone, PartialEq)]
pub enum QuotaResult {
    /// Request is within quota.
    Allowed {
        /// Remaining quota before limit.
        remaining: f64,
        /// The limit value.
        limit: f64,
    },
    /// Request is within quota but above warning threshold.
    Warning {
        /// Threshold percentage that was crossed (e.g. 80.0).
        threshold: f64,
        /// Remaining quota before limit.
        remaining: f64,
    },
    /// Request exceeds the quota limit.
    Exceeded {
        /// Which metric exceeded (requests, tokens, cost_usd).
        metric: String,
        /// The limit that was exceeded.
        limit: f64,
    },
}

/// Request context for quota checks.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub org_id: uuid::Uuid,
    pub api_key_id: Option<uuid::Uuid>,
    pub model: String,
    pub provider: String,
    pub estimated_tokens: u64,
    pub estimated_cost: f64,
}

/// Quota metric types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaMetric {
    Requests,
    Tokens,
    CostUsd,
}

impl QuotaMetric {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuotaMetric::Requests => "requests",
            QuotaMetric::Tokens => "tokens",
            QuotaMetric::CostUsd => "cost_usd",
        }
    }
}

impl std::str::FromStr for QuotaMetric {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "requests" => Ok(QuotaMetric::Requests),
            "tokens" => Ok(QuotaMetric::Tokens),
            "cost_usd" => Ok(QuotaMetric::CostUsd),
            _ => Err(format!("unknown quota metric: {}", s)),
        }
    }
}

/// Quota period types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaPeriod {
    Minute,
    Hour,
    Day,
    Month,
    Total,
}

impl QuotaPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuotaPeriod::Minute => "minute",
            QuotaPeriod::Hour => "hour",
            QuotaPeriod::Day => "day",
            QuotaPeriod::Month => "month",
            QuotaPeriod::Total => "total",
        }
    }

    /// Compute the period boundary for a given timestamp.
    pub fn period_start(&self, now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
        use chrono::{Datelike, Timelike};
        match self {
            QuotaPeriod::Minute => now.with_second(0).unwrap().with_nanosecond(0).unwrap(),
            QuotaPeriod::Hour => now
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap(),
            QuotaPeriod::Day => now
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap(),
            QuotaPeriod::Month => now
                .with_day(1)
                .unwrap()
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap(),
            QuotaPeriod::Total => Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap(),
        }
    }

    /// Compute the period end for a given start.
    pub fn period_end(&self, start: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
        match self {
            QuotaPeriod::Minute => start + chrono::Duration::minutes(1),
            QuotaPeriod::Hour => start + chrono::Duration::hours(1),
            QuotaPeriod::Day => start + chrono::Duration::days(1),
            QuotaPeriod::Month => {
                // Add one month — handle month boundaries
                let year = start.year();
                let month = start.month();
                if month == 12 {
                    start.with_year(year + 1).unwrap().with_month(1).unwrap()
                } else {
                    start.with_month(month + 1).unwrap()
                }
            }
            QuotaPeriod::Total => Utc.with_ymd_and_hms(9999, 12, 31, 23, 59, 59).unwrap(),
        }
    }
}

impl std::str::FromStr for QuotaPeriod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "minute" => Ok(QuotaPeriod::Minute),
            "hour" => Ok(QuotaPeriod::Hour),
            "day" => Ok(QuotaPeriod::Day),
            "month" => Ok(QuotaPeriod::Month),
            "total" => Ok(QuotaPeriod::Total),
            _ => Err(format!("unknown quota period: {}", s)),
        }
    }
}
