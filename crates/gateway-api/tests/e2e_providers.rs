//! E2E tests for provider management endpoints.

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_provider_crud_flow() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    // 1. Create a provider
    let create_response = app
        .post_json_auth(
            "/v1/providers",
            &api_key,
            serde_json::json!({
                "name": "Test OpenAI",
                "kind": "openai",
                "api_key": "sk-test-key",
                "base_url": "https://api.openai.com",
                "models": ["gpt-4o", "gpt-4-turbo"],
                "priority": 10
            }),
        )
        .await;

    assert_eq!(create_response.status(), 200, "Create provider failed: {:?}", create_response.text().await);

    let create_body: serde_json::Value = create_response.json().await.expect("failed to parse create");
    let provider_id = create_body["id"].as_str().unwrap();
    assert_eq!(create_body["name"], "Test OpenAI");
    assert_eq!(create_body["kind"], "openai");
    assert_eq!(create_body["priority"], 10);

    // 2. List providers
    let list_response = app
        .client
        .get(format!("{}/v1/providers", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("failed to list providers");

    assert_eq!(list_response.status(), 200);
    let list_body: serde_json::Value = list_response.json().await.expect("failed to parse list");
    let data = list_body["data"].as_array().unwrap();
    assert!(data.len() >= 1);
    assert!(data.iter().any(|p| p["name"] == "Test OpenAI"));

    // 3. Get provider detail
    let detail_response = app
        .client
        .get(format!("{}/v1/providers/{}", app.base_url(), provider_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("failed to get provider");

    assert_eq!(detail_response.status(), 200);
    let detail_body: serde_json::Value = detail_response.json().await.expect("failed to parse detail");
    assert_eq!(detail_body["name"], "Test OpenAI");
    assert_eq!(detail_body["kind"], "openai");

    // 4. Update provider
    let update_response = app
        .client
        .put(format!("{}/v1/providers/{}", app.base_url(), provider_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "name": "Updated OpenAI",
            "status": "inactive"
        }))
        .send()
        .await
        .expect("failed to update provider");

    assert_eq!(update_response.status(), 200);
    let update_body: serde_json::Value = update_response.json().await.expect("failed to parse update");
    assert_eq!(update_body["name"], "Updated OpenAI");
    assert_eq!(update_body["status"], "inactive");

    // 5. Delete provider
    let delete_response = app
        .client
        .delete(format!("{}/v1/providers/{}", app.base_url(), provider_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("failed to delete provider");

    assert_eq!(delete_response.status(), 204);
}

#[tokio::test]
async fn test_provider_test_connection_endpoint_exists() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let response = app
        .post_json_auth(
            "/v1/providers/test",
            &api_key,
            serde_json::json!({
                "name": "Test",
                "kind": "openai",
                "api_key": "invalid-key",
                "base_url": "https://api.openai.com"
            }),
        )
        .await;

    // Should return a response (likely 200 with success=false since key is invalid)
    assert!(
        response.status() == 200 || response.status() == 500,
        "Expected 200 or 500, got {}",
        response.status()
    );
}
