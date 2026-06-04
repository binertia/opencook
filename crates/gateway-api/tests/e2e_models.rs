//! E2E tests for model listing endpoints (OpenCode compatibility).

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_list_models_returns_openai_compatible_format() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let response = app
        .client
        .get(format!("{}/v1/models", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["object"], "list");
    let data = body["data"].as_array().unwrap();
    // May be empty if no providers configured, but should still return valid format
    if !data.is_empty() {
        let first = &data[0];
        assert!(first.get("id").is_some());
        assert_eq!(first["object"], "model");
        assert!(first.get("created").is_some());
        assert!(first.get("owned_by").is_some());
    }
}

#[tokio::test]
async fn test_get_single_model_exists() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let response = app
        .client
        .get(format!("{}/v1/models/gpt-4o", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["id"], "gpt-4o");
    assert_eq!(body["object"], "model");
    assert!(body.get("created").is_some());
    assert!(body.get("owned_by").is_some());
}

#[tokio::test]
async fn test_get_single_model_not_found() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let response = app
        .client
        .get(format!("{}/v1/models/nonexistent-model", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 404);
}
