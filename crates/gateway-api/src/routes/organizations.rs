//! Organization routes — creation, listing, and management.

use axum::{extract::State, http::StatusCode, Extension, Json};
use gateway_auth::AuthContext;
use gateway_db::{models::AuditAction, OrgMemberRepo, OrganizationRepo};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{
    audit::{self, AuditRequestContext},
    error::ApiError,
    extractors::ValidatedJson,
    state::AppState,
    validation::sanitize_input,
};

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrgRequest {
    #[validate(length(
        min = 1,
        max = 128,
        message = "Organization name must be 1-128 characters"
    ))]
    pub name: String,
    #[validate(length(max = 128, message = "Billing email must be at most 128 characters"))]
    pub billing_email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub plan_tier: String,
    pub created_at: String,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Convert a name into a URL-safe slug.
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// Append a short random suffix to ensure slug uniqueness.
fn unique_slug(base: &str) -> String {
    let suffix: String =
        rand::Rng::sample_iter(&mut rand::thread_rng(), rand::distributions::Alphanumeric)
            .take(6)
            .map(char::from)
            .collect::<String>()
            .to_lowercase();
    format!("{}-{}", base, suffix)
}

/// Rate-limit org creation: max 5 per hour per user (tracked in Redis).
async fn check_org_creation_rate_limit(state: &AppState, user_id: Uuid) -> Result<(), ApiError> {
    let key = format!("rate_limit:org_create:{}", user_id);
    let mut conn = state.redis.clone();

    let count: i64 = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap_or(0);

    if count >= 5 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_exceeded",
            "Organization creation rate limit exceeded. Try again later.",
        ));
    }

    // Increment and set expiry if new
    let _: () = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap_or(());

    let _: i32 = redis::cmd("EXPIRE")
        .arg(&key)
        .arg(3600)
        .query_async(&mut conn)
        .await
        .unwrap_or(0);

    Ok(())
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /v1/organizations
///
/// Creates a new organization. The authenticated user becomes the owner
/// automatically. Rate-limited to 5 orgs per hour per user.
pub async fn create_organization(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(ctx): Extension<AuditRequestContext>,
    ValidatedJson(body): ValidatedJson<CreateOrgRequest>,
) -> Result<Json<OrgResponse>, ApiError> {
    let user_id = auth.user_id.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Not authenticated",
        )
    })?;

    // Rate limit org creation to prevent spam/abuse.
    check_org_creation_rate_limit(&state, user_id).await?;

    // Sanitize inputs.
    let name = sanitize_input(&body.name);
    let billing_email = body.billing_email.as_deref().map(sanitize_input);

    // Validate billing email format if provided.
    if let Some(ref email) = billing_email {
        if !email.contains('@') || email.len() > 254 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_billing_email",
                "Invalid billing email address",
            ));
        }
    }

    let org_repo = OrganizationRepo::new(state.db_pool.clone());
    let member_repo = OrgMemberRepo::new(state.db_pool.clone());

    // Generate a unique slug.
    let base_slug = slugify(&name);
    let mut slug = base_slug.clone();
    let mut attempts = 0;
    while org_repo
        .find_by_slug(&slug)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?
        .is_some()
    {
        slug = unique_slug(&base_slug);
        attempts += 1;
        if attempts > 10 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "slug_generation_failed",
                "Unable to generate a unique organization slug. Please try a different name.",
            ));
        }
    }

    // Create the organization.
    let org = org_repo
        .create(&name, &slug, billing_email.as_deref(), "free")
        .await
        .map_err(|e| {
            // Handle unique constraint violation gracefully.
            if e.to_string().contains("unique") || e.to_string().contains("UNIQUE") {
                ApiError::new(
                    StatusCode::CONFLICT,
                    "org_already_exists",
                    "An organization with this slug already exists",
                )
            } else {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    e.to_string(),
                )
            }
        })?;

    // Add creator as owner.
    let membership = member_repo
        .create(user_id, org.id, "owner", Some(user_id))
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            )
        })?;

    // Audit log the creation.
    let owner_auth = AuthContext {
        auth_type: gateway_auth::AuthType::Session,
        org_id: org.id,
        user_id: Some(user_id),
        key_id: None,
        role: Some("owner".to_string()),
        permissions: vec![],
        rate_limit_rps: None,
    };
    audit::record(
        &state,
        &owner_auth,
        &ctx,
        AuditAction::Create,
        "organization",
        Some(&org.id.to_string()),
        None,
        Some(json!({
            "name": name,
            "slug": slug,
            "creator_user_id": user_id.to_string(),
            "membership_role": membership.role
        })),
        "Organization created",
    )
    .await;

    Ok(Json(OrgResponse {
        id: org.id.to_string(),
        name: org.name,
        slug: org.slug,
        status: org.status,
        plan_tier: org.plan_tier,
        created_at: org.created_at.to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Test Org 123"), "test-org-123");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("My Org!@#"), "my-org");
        assert_eq!(slugify("A&B Corp"), "a-b-corp");
    }

    #[test]
    fn test_unique_slug_format() {
        let slug = unique_slug("test");
        assert!(slug.starts_with("test-"));
        assert_eq!(slug.len(), 11); // "test-" + 6 chars
    }
}
