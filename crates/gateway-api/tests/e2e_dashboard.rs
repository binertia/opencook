//! E2E tests for the dashboard KPI endpoint.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_dashboard_endpoint_exists_and_returns_data() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .client
        .get(format!("{}/v1/dashboard?range=today", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Expected 200, got {}: {}", status, body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");

    assert!(body.get("total_requests").is_some());
    assert!(body.get("total_cost_usd").is_some());
    assert!(body.get("cache_hit_rate").is_some());
    assert!(body.get("avg_latency_ms").is_some());
    assert!(body.get("recent_requests").is_some());
    assert!(body.get("active_providers").is_some());

    let recent = body["recent_requests"].as_array().expect("recent_requests should be an array");
    assert!(recent.len() <= 10);

    let providers = body["active_providers"].as_array().expect("active_providers should be an array");
    // Should be empty since no providers configured in test
    assert_eq!(providers.len(), 0);
}

#[tokio::test]
async fn test_dashboard_with_providers() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    // Create a provider first
    let create_resp = app
        .post_json_auth(
            "/v1/providers",
            &api_key,
            serde_json::json!({
                "name": "Test Provider",
                "kind": "openai",
                "api_key": "test-key",
                "base_url": "https://api.openai.com/v1",
                "models": ["gpt-4o"],
                "priority": 1,
            }),
        )
        .await;
    assert!(create_resp.status().is_success());

    // Now fetch dashboard
    let response = app
        .client
        .get(format!("{}/v1/dashboard?range=7d", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Expected 200, got {}: {}", status, body_text);
    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");
    let providers = body["active_providers"].as_array().expect("active_providers should be an array");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["name"], "Test Provider");
}
