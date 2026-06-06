//! E2E tests for API key management endpoints.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_api_key_crud_flow() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    // 1. List API keys
    let list_resp = app
        .client
        .get(format!("{}/v1/api-keys", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to list API keys");
    assert!(list_resp.status().is_success());

    // 2. Create an API key
    let create_resp = app
        .post_json_auth(
            "/v1/api-keys",
            &api_key,
            serde_json::json!({
                "name": "Test Key",
                "scopes": ["chat"],
                "rate_limit_rps": 5,
            }),
        )
        .await;
    assert!(
        create_resp.status().is_success(),
        "Create API key failed: {:?}",
        create_resp.text().await
    );
    let create_body: serde_json::Value = create_resp.json().await.expect("Failed to parse create");
    let key_id = create_body["id"].as_str().unwrap();
    assert_eq!(create_body["name"], "Test Key");
    assert!(create_body["key"].as_str().unwrap().starts_with("sk_gw_"));
    assert_eq!(create_body["scopes"], serde_json::json!(["chat"]));
    assert_eq!(create_body["rate_limit_rps"], 5);

    // 3. List API keys (should have initial + 1)
    let list_resp = app
        .client
        .get(format!("{}/v1/api-keys", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to list API keys");
    let list_body: serde_json::Value = list_resp.json().await.expect("Failed to parse list");
    let data = list_body["data"].as_array().unwrap();
    assert!(data.iter().any(|k| k["name"] == "Test Key"));

    // 4. Update API key
    let update_resp = app
        .client
        .put(format!("{}/v1/api-keys/{}", app.base_url(), key_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({ "name": "Updated Key" }))
        .send()
        .await
        .expect("Failed to update API key");
    assert!(update_resp.status().is_success());
    let update_body: serde_json::Value = update_resp.json().await.expect("Failed to parse update");
    assert_eq!(update_body["name"], "Updated Key");

    // 5. Delete API key (delete handler also revokes)
    let delete_resp = app
        .client
        .delete(format!("{}/v1/api-keys/{}", app.base_url(), key_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to delete API key");
    assert_eq!(delete_resp.status(), 204);
}
