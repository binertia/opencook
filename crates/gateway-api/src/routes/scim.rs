//! SCIM 2.0 routes.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use gateway_auth::scim::*;
use gateway_db::{ScimTokenRepo, UserRepo, OrgMemberRepo, DbBackend};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ── Auth Extension ───────────────────────────────────────────────────

#[derive(Clone)]
pub struct ScimAuth {
    pub org_id: Uuid,
}

// ── Query Types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ScimListQuery {
    #[serde(default = "default_start_index")]
    pub start_index: usize,
    #[serde(default = "default_count")]
    pub count: usize,
    pub filter: Option<String>,
}

fn default_start_index() -> usize { 1 }
fn default_count() -> usize { 100 }

// ── Middleware: SCIM Token Auth ──────────────────────────────────────

pub async fn scim_auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return scim_error(StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header");
        }
    };

    let token_hash = gateway_auth::api_key::sha256_hex(token);

    let pool = match &state.db_pool {
        DbBackend::Postgres(pg) => pg.clone(),
        DbBackend::Sqlite(_) => {
            return scim_error(StatusCode::SERVICE_UNAVAILABLE, "SCIM requires PostgreSQL");
        }
    };

    let repo = ScimTokenRepo::new(pool);
    match repo.find_by_hash(&token_hash).await {
        Ok(Some(t)) => {
            req.extensions_mut().insert(ScimAuth { org_id: t.org_id });
            next.run(req).await
        }
        Ok(None) => scim_error(StatusCode::UNAUTHORIZED, "Invalid SCIM token"),
        Err(e) => {
            warn!(error = %e, "SCIM token lookup failed");
            scim_error(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

fn scim_error(status: StatusCode, detail: &str) -> Response {
    let err = ScimError::new(status.as_u16(), detail);
    let mut resp = Json(err).into_response();
    *resp.status_mut() = status;
    resp
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn service_provider_config() -> impl IntoResponse {
    let config = gateway_auth::scim::service_provider_config("");
    (StatusCode::OK, [("Content-Type", "application/scim+json")], Json(config))
}

pub async fn resource_types() -> impl IntoResponse {
    let types = gateway_auth::scim::resource_types();
    (StatusCode::OK, [("Content-Type", "application/scim+json")], Json(types))
}

pub async fn schemas() -> impl IntoResponse {
    let schemas = gateway_auth::scim::schemas();
    (StatusCode::OK, [("Content-Type", "application/scim+json")], Json(schemas))
}

pub async fn list_users(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Query(query): Query<ScimListQuery>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());

    let users = match user_repo.list_by_org(auth.org_id, None, Some("all")).await {
        Ok(u) => u,
        Err(e) => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let total = users.len();
    let start = query.start_index.saturating_sub(1);
    let end = (start + query.count).min(total);
    let page = &users[start..end];

    let scim_users: Vec<ScimUser> = page.iter().map(|u| db_user_to_scim(u)).collect();

    let resp = ScimListResponse::new(scim_users, total, query.start_index, query.count);
    let mut response = Json(resp).into_response();
    response.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
    response
}

pub async fn get_user(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Path(user_id): Path<Uuid>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());

    match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) if u.org_id == auth.org_id => {
            let mut resp = Json(db_user_to_scim(&u)).into_response();
            resp.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
            resp
        }
        Ok(Some(_)) => scim_error(StatusCode::NOT_FOUND, "User not found in organization"),
        Ok(None) => scim_error(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn create_user(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Json(body): Json<ScimUser>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());
    let member_repo = OrgMemberRepo::new(state.db_pool.clone());

    let display_name = body.name.as_ref().map(|n| {
        let mut parts = Vec::new();
        if let Some(g) = &n.given_name { parts.push(g.clone()); }
        if let Some(f) = &n.family_name { parts.push(f.clone()); }
        if parts.is_empty() {
            n.formatted.clone()
        } else {
            Some(parts.join(" "))
        }
    }).flatten();

    let user = match user_repo.create(
        auth.org_id,
        &body.user_name,
        None,
        display_name.as_deref(),
        "member",
        if body.active { "active" } else { "suspended" },
    ).await {
        Ok(u) => u,
        Err(e) => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Link to org
    let _ = member_repo.create(user.id, auth.org_id, "member", None).await;

    info!(user_id = %user.id, org_id = %auth.org_id, "SCIM user created");

    let mut resp = Json(db_user_to_scim(&user)).into_response();
    resp.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
    *resp.status_mut() = StatusCode::CREATED;
    resp
}

pub async fn update_user(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<ScimUser>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());

    let existing = match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) if u.org_id == auth.org_id => u,
        Ok(Some(_)) => return scim_error(StatusCode::NOT_FOUND, "User not found in organization"),
        Ok(None) => return scim_error(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Update status if changed
    let new_status = if body.active { "active" } else { "suspended" };
    if existing.status != new_status {
        if let Err(e) = user_repo.update_status(user_id, new_status).await {
            return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
        }
    }

    let user = match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) => u,
        _ => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to reload user"),
    };

    let mut resp = Json(db_user_to_scim(&user)).into_response();
    resp.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
    resp
}

#[derive(Debug, Deserialize)]
pub struct ScimPatchOp {
    pub schemas: Vec<String>,
    pub operations: Vec<ScimPatchOperation>,
}

#[derive(Debug, Deserialize)]
pub struct ScimPatchOperation {
    pub op: String,
    pub path: Option<String>,
    pub value: Option<serde_json::Value>,
}

pub async fn patch_user(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<ScimPatchOp>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());

    let existing = match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) if u.org_id == auth.org_id => u,
        Ok(Some(_)) => return scim_error(StatusCode::NOT_FOUND, "User not found in organization"),
        Ok(None) => return scim_error(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    for op in &body.operations {
        match op.op.as_str() {
            "Replace" => {
                if op.path.as_deref() == Some("active") {
                    let active = op.value.as_ref().and_then(|v| v.as_bool()).unwrap_or(true);
                    let new_status = if active { "active" } else { "suspended" };
                    if existing.status != new_status {
                        if let Err(e) = user_repo.update_status(user_id, new_status).await {
                            return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let user = match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) => u,
        _ => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to reload user"),
    };

    let mut resp = Json(db_user_to_scim(&user)).into_response();
    resp.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
    resp
}

pub async fn delete_user(
    State(state): State<AppState>,
    Extension(auth): Extension<ScimAuth>,
    Path(user_id): Path<Uuid>,
) -> Response {
    let user_repo = UserRepo::new(state.db_pool.clone());

    match user_repo.find_by_id(user_id).await {
        Ok(Some(u)) if u.org_id == auth.org_id => {}
        Ok(Some(_)) => return scim_error(StatusCode::NOT_FOUND, "User not found in organization"),
        Ok(None) => return scim_error(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }

    if let Err(e) = user_repo.update_status(user_id, "suspended").await {
        return scim_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }

    info!(user_id = %user_id, org_id = %auth.org_id, "SCIM user deactivated");
    StatusCode::NO_CONTENT.into_response()
}

pub async fn list_groups() -> Response {
    let groups: Vec<ScimGroup> = vec![];
    let resp = ScimListResponse::new(groups, 0, 1, 100);
    let mut response = Json(resp).into_response();
    response.headers_mut().insert("Content-Type", axum::http::HeaderValue::from_static("application/scim+json"));
    response
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_user_to_scim(user: &gateway_db::User) -> ScimUser {
    let mut scim = ScimUser::new(
        &user.id.to_string(),
        &user.email,
        user.status == "active",
    );
    scim.display_name = user.display_name.clone();
    scim.name = user.display_name.as_ref().map(|d| ScimName {
        formatted: Some(d.clone()),
        family_name: None,
        given_name: None,
    });
    scim.emails = Some(vec![ScimEmail {
        value: user.email.clone(),
        email_type: Some("work".to_string()),
        primary: Some(true),
    }]);
    scim.meta.created = Some(user.created_at.to_rfc3339());
    scim.meta.last_modified = Some(user.updated_at.to_rfc3339());
    scim
}
