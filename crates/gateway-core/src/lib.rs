//! Gateway Core — Request orchestration, routing, and transformation pipeline.

pub mod router;
pub mod quota;
pub mod types;
pub mod orchestrator;
pub mod streaming;
pub mod strategies;
pub mod circuit_breaker;
pub mod retry;
pub mod profiles;

pub use types::*;
pub use orchestrator::{orchestrate_chat_completion, OrchestratorError};
pub use streaming::LoggingStream;
