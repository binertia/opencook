//! E2E tests for user management endpoints.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_user_crud_flow() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    // 1. List users (should be empty or have default)
    let list_resp = app
        .client
        .get(format!("{}/v1/users", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to list users");
    assert!(list_resp.status().is_success());
    let list_body: serde_json::Value = list_resp.json().await.expect("Failed to parse list");
    assert_eq!(list_body["object"], "list");

    // 2. Create a user
    let create_resp = app
        .post_json_auth(
            "/v1/users",
            &api_key,
            serde_json::json!({
                "email": "newuser@example.com",
                "name": "New User",
                "role": "member",
            }),
        )
        .await;
    assert!(create_resp.status().is_success(), "Create user failed: {:?}", create_resp.text().await);
    let create_body: serde_json::Value = create_resp.json().await.expect("Failed to parse create");
    let user_id = create_body["id"].as_str().unwrap();
    assert_eq!(create_body["email"], "newuser@example.com");
    assert_eq!(create_body["name"], "New User");
    assert_eq!(create_body["role"], "member");
    assert_eq!(create_body["status"], "pending");

    // 3. Update user role
    let update_resp = app
        .client
        .put(format!("{}/v1/users/{}", app.base_url(), user_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({ "role": "admin" }))
        .send()
        .await
        .expect("Failed to update user");
    assert!(update_resp.status().is_success());
    let update_body: serde_json::Value = update_resp.json().await.expect("Failed to parse update");
    assert_eq!(update_body["role"], "admin");

    // 4. Delete user
    let delete_resp = app
        .client
        .delete(format!("{}/v1/users/{}", app.base_url(), user_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to delete user");
    assert_eq!(delete_resp.status(), 204);
}

#[tokio::test]
async fn test_user_list_with_search_and_status() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    // Create a user
    app.post_json_auth(
        "/v1/users",
        &api_key,
        serde_json::json!({
            "email": "searchme@example.com",
            "name": "Search Me",
            "role": "viewer",
        }),
    )
    .await;

    // Search by email
    let search_resp = app
        .client
        .get(format!("{}/v1/users?search=searchme", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to search users");
    assert!(search_resp.status().is_success());
    let body: serde_json::Value = search_resp.json().await.expect("Failed to parse");
    let data = body["data"].as_array().unwrap();
    assert!(!data.is_empty());

    // Filter by status
    let status_resp = app
        .client
        .get(format!("{}/v1/users?status=pending", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("Failed to filter users");
    assert!(status_resp.status().is_success());
    let body: serde_json::Value = status_resp.json().await.expect("Failed to parse");
    let data = body["data"].as_array().unwrap();
    assert!(data.iter().all(|u| u["status"] == "pending"));
}
