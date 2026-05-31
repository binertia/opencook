//! Gateway Quota — Rate limiting, budget caps, and usage tracking.

pub mod rate_limiter;
pub mod budget;
pub mod usage;
pub mod types;

pub use types::{LimitResult, RateLimitTier, WindowConfig, default_tiers};
pub use rate_limiter::{RateLimiter, LayerCheck};
