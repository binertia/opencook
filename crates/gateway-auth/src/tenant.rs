//! Tenant isolation enforcement.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use tracing;

use crate::error::AuthError;
use crate::models::AuthContext;

/// Middleware that validates the request org_id matches the auth context org_id.
pub async fn tenant_isolation_middleware(
    auth: axum::extract::Extension<AuthContext>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract org_id from URL path if present
    let path_org_id = extract_org_id_from_path(req.uri().path());

    if let Some(path_org_id) = path_org_id {
        if path_org_id != auth.org_id.to_string() {
            tracing::warn!(
                auth_org_id = %auth.org_id,
                path_org_id = %path_org_id,
                "cross-organization access attempt detected"
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    Ok(next.run(req).await)
}

/// Extract org_id from paths like `/api/v1/organizations/{org_id}/...`
fn extract_org_id_from_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    // Look for pattern: .../organizations/{org_id}/...
    for (i, part) in parts.iter().enumerate() {
        if *part == "organizations" && i + 1 < parts.len() {
            let candidate = parts[i + 1];
            // Validate UUID-like format (simple check)
            if candidate.len() == 36 && candidate.chars().filter(|&c| c == '-').count() == 4 {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Require that the authenticated user's org_id matches the given org_id.
pub fn require_same_org(auth: &AuthContext, org_id: &uuid::Uuid) -> Result<(), AuthError> {
    if &auth.org_id != org_id {
        return Err(AuthError::PermissionDenied);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_org_id_from_path() {
        assert_eq!(
            extract_org_id_from_path(
                "/api/v1/organizations/550e8400-e29b-41d4-a716-446655440000/keys"
            ),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(extract_org_id_from_path("/api/v1/health"), None);
    }
}
