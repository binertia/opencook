//! Gateway API — HTTP server, routing, middleware, and handlers.

pub mod audit;
pub mod cli;
pub mod config_reload;
pub mod config_wizard;
pub mod dashboard;
pub mod error;
pub mod extractors;
pub mod health_worker;
pub mod middleware;
pub mod router;
pub mod routes;
pub mod shutdown;
pub mod state;
pub mod static_files;
pub mod tls;
pub mod validation;
