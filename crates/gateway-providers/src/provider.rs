//! Canonical `Provider` trait and request/response types.

use async_trait::async_trait;

/// Unified provider interface.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &'static str;
}
