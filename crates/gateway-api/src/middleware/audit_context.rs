//! Injects an `AuditRequestContext` extension into every request.

use axum::{extract::Request, middleware::Next, response::Response};

use crate::audit::AuditRequestContext;

/// Middleware that extracts request metadata useful for audit logging.
pub async fn audit_context_middleware(mut req: Request, next: Next) -> Response {
    let ctx = AuditRequestContext::from_request(&req);
    req.extensions_mut().insert(ctx);
    next.run(req).await
}

impl AuditRequestContext {
    fn from_request(req: &Request) -> Self {
        use axum::http::header::USER_AGENT;
        use uuid::Uuid;

        let request_id = req
            .extensions()
            .get::<crate::middleware::timing::RequestId>()
            .and_then(|r| Uuid::parse_str(&r.0).ok());

        let user_agent = req
            .headers()
            .get(USER_AGENT)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let ip_address = forwarded_for(req)
            .or_else(|| x_forwarded_for(req))
            .or_else(|| x_real_ip(req));

        Self {
            request_id,
            ip_address,
            user_agent,
        }
    }
}

fn forwarded_for(req: &Request) -> Option<String> {
    req.headers()
        .get("forwarded")
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
