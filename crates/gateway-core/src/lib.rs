//! Gateway Core — Request orchestration, routing, and transformation pipeline.

pub mod router;
pub mod quota;
pub mod types;
pub mod orchestrator;
pub mod streaming;

pub use types::*;
pub use orchestrator::{orchestrate_chat_completion, OrchestratorError};
pub use streaming::LoggingStream;
