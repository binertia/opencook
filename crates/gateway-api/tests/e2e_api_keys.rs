//! E2E tests for API key management endpoints.

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_api_key_crud_flow() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    // 1. List API keys (should be empty)
    let list_resp = app
        .client
        .get(format!("{}/v1/api-keys", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to list API keys");
    assert!(list_resp.status().is_success());
    let list_body: serde_json::Value = list_resp.json().await.expect("Failed to parse list");
    assert_eq!(list_body["object"], "list");
    let data = list_body["data"].as_array().unwrap();
    assert_eq!(data.len(), 0);

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
    assert!(create_resp.status().is_success(), "Create API key failed: {:?}", create_resp.text().await);
    let create_body: serde_json::Value = create_resp.json().await.expect("Failed to parse create");
    let key_id = create_body["id"].as_str().unwrap();
    assert_eq!(create_body["name"], "Test Key");
    assert!(create_body["key"].as_str().unwrap().starts_with("sk_gw_"));
    assert_eq!(create_body["scopes"], serde_json::json!(["chat"]));
    assert_eq!(create_body["rate_limit_rps"], 5);

    // 3. List API keys (should have 1)
    let list_resp = app
        .client
        .get(format!("{}/v1/api-keys", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to list API keys");
    let list_body: serde_json::Value = list_resp.json().await.expect("Failed to parse list");
    let data = list_body["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], "Test Key");

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

    // 5. Revoke API key
    let revoke_resp = app
        .client
        .put(format!("{}/v1/api-keys/{}", app.base_url(), key_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({ "status": "revoked" }))
        .send()
        .await
        .expect("Failed to revoke API key");
    assert!(revoke_resp.status().is_success());

    // 6. Delete API key
    let delete_resp = app
        .client
        .delete(format!("{}/v1/api-keys/{}", app.base_url(), key_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to delete API key");
    assert_eq!(delete_resp.status(), 204);
}
