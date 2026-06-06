//! Gateway Quota — Rate limiting, budget caps, and usage tracking.

pub mod aggregator;
pub mod budget;
pub mod models;
pub mod quota_engine;
pub mod rate_limiter;
pub mod types;
pub mod usage;

pub use models::{QuotaMetric, QuotaPeriod, QuotaResult, RequestContext};
pub use quota_engine::QuotaEngine;
pub use rate_limiter::{LayerCheck, RateLimiter};
pub use types::{default_tiers, LimitResult, RateLimitTier, WindowConfig};
