//! Audit log recording helpers.

use axum::{
    extract::{ConnectInfo, Request},
    http::header::{FORWARDED, USER_AGENT},
};
use gateway_auth::AuthContext;
use gateway_db::{
    models::AuditAction,
    repos::audit_repo::AuditRepo,
};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::{middleware::timing::RequestId, state::AppState};

/// Security-relevant context extracted from the HTTP request.
#[derive(Debug, Clone, Default)]
pub struct AuditRequestContext {
    pub request_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Extract audit context from an Axum request.
pub fn extract_context(req: &Request) -> AuditRequestContext {
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .and_then(|r| Uuid::parse_str(&r.0).ok());

    let user_agent = req
        .headers()
        .get(USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let ip_address = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .or_else(|| forwarded_for(req))
        .or_else(|| x_forwarded_for(req))
        .or_else(|| x_real_ip(req));

    AuditRequestContext {
        request_id,
        ip_address,
        user_agent,
    }
}

fn forwarded_for(req: &Request) -> Option<String> {
    req.headers()
        .get(FORWARDED)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(';').find(|part| part.trim().starts_with("for=")))
        .map(|part| {
            part.trim()
                .strip_prefix("for=")
                .map(|v| v.trim_matches('"').to_string())
                .unwrap_or_else(|| part.trim().to_string())
        })
}

fn x_forwarded_for(req: &Request) -> Option<String> {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
}

fn x_real_ip(req: &Request) -> Option<String> {
    req.headers()
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_string())
}

/// Record an audit entry, logging errors instead of failing the request.
pub async fn record(
    state: &AppState,
    auth: &AuthContext,
    ctx: &AuditRequestContext,
    action: AuditAction,
    entity_type: &str,
    entity_id: Option<&str>,
    old_values: Option<serde_json::Value>,
    new_values: Option<serde_json::Value>,
    summary: &str,
) {
    let repo = AuditRepo::new(state.db_pool.clone());
    if let Err(e) = repo
        .record(
            auth.org_id,
            auth.user_id,
            auth.key_id,
            action,
            entity_type,
            entity_id,
            old_values,
            new_values,
            summary,
            ctx.ip_address.as_deref(),
            ctx.user_agent.as_deref(),
            ctx.request_id,
        )
        .await
    {
        tracing::warn!(error = %e, "failed to write audit log entry");
    }
}
