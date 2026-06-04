//! E2E tests for analytics endpoint.

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_analytics_endpoint_exists_and_returns_data() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let response = app
        .client
        .get(format!("{}/v1/analytics?range=30d", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "Expected 200, got {}: {}", status, body_text);

    let body: serde_json::Value = serde_json::from_str(&body_text).expect("Failed to parse JSON");

    assert!(body.get("total_requests").is_some());
    assert!(body.get("total_tokens").is_some());
    assert!(body.get("total_cost_usd").is_some());
    assert!(body.get("avg_latency_ms").is_some());
    assert!(body.get("cache_hit_rate").is_some());
    assert!(body.get("error_rate").is_some());
    assert!(body.get("time_series").is_some());
    assert!(body.get("by_model").is_some());
    assert!(body.get("by_status").is_some());
}
