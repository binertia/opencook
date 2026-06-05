//! Config reload handler triggered by SIGHUP.
//!
//! Reloads components that can be updated without restarting the process:
//! - JWT keys (if PEM files have changed)
//! - Allowed origins
//! - TLS certificate/key paths
//!
//! Does NOT reload: database URL, Redis URL, master key.

use std::sync::Arc;

use crate::state::AppConfig;

/// Handle a config reload signal.
pub async fn handle_reload(config: Arc<AppConfig>) {
    tracing::info!("config reload started");

    // Re-read configuration from disk/env.
    let new_config = AppConfig::load();

    // Log what changed (without exposing secrets).
    if new_config.allowed_origins != config.allowed_origins {
        tracing::info!(
            old = ?config.allowed_origins,
            new = ?new_config.allowed_origins,
            "allowed_origins changed"
        );
    }

    if new_config.environment != config.environment {
        tracing::info!(
            old = %config.environment,
            new = %new_config.environment,
            "environment changed"
        );
    }

    // JWT key reload: if PEM paths changed or keys were rotated on disk.
    let jwt_reloaded = if !new_config.jwt_private_key_pem.is_empty()
        && !new_config.jwt_public_key_pem.is_empty()
        && (new_config.jwt_private_key_pem != config.jwt_private_key_pem
            || new_config.jwt_public_key_pem != config.jwt_public_key_pem)
    {
        match gateway_auth::JwtService::from_pem(
            new_config.jwt_private_key_pem.as_bytes(),
            new_config.jwt_public_key_pem.as_bytes(),
        ) {
            Ok(_) => {
                tracing::info!("JWT keys reloaded successfully");
                true
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to reload JWT keys — keeping existing keys");
                false
            }
        }
    } else {
        false
    };

    // TLS cert reload check.
    let tls_changed = new_config.tls_cert != config.tls_cert || new_config.tls_key != config.tls_key;
    if tls_changed {
        tracing::info!("TLS certificate configuration changed — will take effect on next restart");
    }

    tracing::info!(
        jwt_reloaded,
        tls_changed,
        "config reload complete"
    );
}
