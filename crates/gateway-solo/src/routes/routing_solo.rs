//! Solo-mode routing rule stubs.

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct RoutingRuleListResponse {
    pub data: Vec<RoutingRule>,
}

#[derive(Serialize)]
pub struct RoutingRule {
    pub id: String,
    pub name: String,
    pub priority: i32,
    pub model_pattern: String,
    pub provider_id: String,
    pub fallback_provider_id: Option<String>,
    pub status: String,
    pub created_at: String,
}

pub async fn list_routing_rules() -> Json<RoutingRuleListResponse> {
    Json(RoutingRuleListResponse {
        data: vec![RoutingRule {
            id: "solo-rule-1".to_string(),
            name: "Default Route".to_string(),
            priority: 1,
            model_pattern: "*".to_string(),
            provider_id: "openai".to_string(),
            fallback_provider_id: Some("anthropic".to_string()),
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
    })
}
