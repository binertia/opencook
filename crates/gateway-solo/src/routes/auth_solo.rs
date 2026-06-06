//! Solo-mode auth stubs — no real authentication, returns a mock user
//! so the React dashboard thinks it's logged in.

use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct SoloUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub organizations: Vec<SoloOrg>,
}

#[derive(Serialize)]
pub struct SoloOrg {
    pub org_id: String,
    pub org_name: String,
    pub role: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: SoloUser,
}

pub async fn me() -> Json<SoloUser> {
    Json(mock_user())
}

fn generate_solo_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub async fn login(Json(_req): Json<LoginRequest>) -> Json<LoginResponse> {
    Json(LoginResponse {
        access_token: generate_solo_token(),
        refresh_token: generate_solo_token(),
        user: mock_user(),
    })
}

pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({"success": true}))
}

pub async fn refresh() -> Json<LoginResponse> {
    Json(LoginResponse {
        access_token: generate_solo_token(),
        refresh_token: generate_solo_token(),
        user: mock_user(),
    })
}

fn mock_user() -> SoloUser {
    SoloUser {
        id: "solo-user".to_string(),
        email: "solo@opencook.local".to_string(),
        name: "Solo Developer".to_string(),
        role: "owner".to_string(),
        permissions: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
        organizations: vec![SoloOrg {
            org_id: "00000000-0000-0000-0000-000000000000".to_string(),
            org_name: "Personal".to_string(),
            role: "owner".to_string(),
        }],
    }
}
