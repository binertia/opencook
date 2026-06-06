//! Gateway Core — Request orchestration, routing, and transformation pipeline.

pub mod cancellation;
pub mod circuit_breaker;
pub mod fallback;
pub mod latency_tracker;
pub mod orchestrator;
pub mod profiles;
pub mod quota;
pub mod retry;
pub mod router;
pub mod strategies;
pub mod streaming;
pub mod types;
pub mod webhook_publisher;

pub use orchestrator::{orchestrate_chat_completion, OrchestratorError};
pub use streaming::LoggingStream;
pub use types::*;
