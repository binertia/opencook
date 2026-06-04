//! User management routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use gateway_auth::AuthContext;
use gateway_db::{repos::user_repo::UserRepo, User as DbUser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ── Request / Response Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub org_id: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserItem {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub last_login_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UsersListResponse {
    pub object: String,
    pub data: Vec<UserItem>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: String,
    pub role: String,
    pub organization_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserDetailResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub last_login_at: Option<String>,
    pub created_at: String,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_users(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<UsersListResponse>, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());

    let org_id = query
        .org_id
        .and_then(|s| Uuid::parse_str(&s).ok())
        .unwrap_or(auth.org_id);

    let users = repo
        .list_by_org(org_id, query.search.as_deref(), query.status.as_deref())
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    let total = users.len() as i64;

    Ok(Json(UsersListResponse {
        object: "list".to_string(),
        data: users.iter().map(|u| db_to_user_item(u)).collect(),
        pagination: PaginationInfo {
            limit: total,
            offset: 0,
            total,
            has_more: false,
        },
    }))
}

pub async fn create_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserDetailResponse>, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());

    // Validate role
    let role = match body.role.as_str() {
        "admin" | "member" | "viewer" => body.role.as_str(),
        _ => "viewer",
    };

    let user = repo
        .create(
            auth.org_id,
            &body.email,
            None, // no password for invited users
            Some(&body.name),
            role,
            "pending",
        )
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(Json(db_to_detail(&user)))
}

pub async fn update_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<UserDetailResponse>, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());

    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_user_id", "Invalid user ID"))?;

    // Verify user exists and belongs to the org
    let user = repo
        .find_by_id(user_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user_not_found", "User not found"))?;

    if user.org_id != auth.org_id {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "User does not belong to your organization"));
    }

    if let Some(role) = body.role {
        let role = match role.as_str() {
            "admin" | "member" | "viewer" => role.as_str(),
            _ => return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid_role", "Invalid role")),
        };

        repo
            .update_role(user_uuid, role)
            .await
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;
    }

    // Re-fetch to get updated data
    let updated = repo
        .find_by_id(user_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user_not_found", "User not found"))?;

    Ok(Json(db_to_detail(&updated)))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(user_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repo = UserRepo::new(state.db_pool.clone());

    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid_user_id", "Invalid user ID"))?;

    // Verify user exists and belongs to the org
    let user = repo
        .find_by_id(user_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user_not_found", "User not found"))?;

    if user.org_id != auth.org_id {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "User does not belong to your organization"));
    }

    repo
        .delete(user_uuid)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database_error", e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Helpers ──────────────────────────────────────────────────────────

fn db_to_user_item(user: &DbUser) -> UserItem {
    UserItem {
        id: user.id.to_string(),
        email: user.email.clone(),
        name: user.display_name.clone().unwrap_or_else(|| user.email.clone()),
        role: user.role.clone(),
        status: user.status.clone(),
        last_login_at: user.last_login_at.map(|t| t.to_rfc3339()),
        created_at: user.created_at.to_rfc3339(),
    }
}

fn db_to_detail(user: &DbUser) -> UserDetailResponse {
    UserDetailResponse {
        id: user.id.to_string(),
        email: user.email.clone(),
        name: user.display_name.clone().unwrap_or_else(|| user.email.clone()),
        role: user.role.clone(),
        status: user.status.clone(),
        last_login_at: user.last_login_at.map(|t| t.to_rfc3339()),
        created_at: user.created_at.to_rfc3339(),
    }
}
