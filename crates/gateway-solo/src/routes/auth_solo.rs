//! Solo-mode auth stubs — no real authentication, returns a mock user
//! so the React dashboard thinks it's logged in.

use axum::Json;
use serde::Serialize;

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

pub async fn me() -> Json<SoloUser> {
    Json(SoloUser {
        id: "solo-user".to_string(),
        email: "solo@opencook.local".to_string(),
        name: "Solo Developer".to_string(),
        role: "owner".to_string(),
        permissions: vec![
            "read".to_string(),
            "write".to_string(),
            "admin".to_string(),
        ],
        organizations: vec![SoloOrg {
            org_id: "00000000-0000-0000-0000-000000000000".to_string(),
            org_name: "Personal".to_string(),
            role: "owner".to_string(),
        }],
    })
}
