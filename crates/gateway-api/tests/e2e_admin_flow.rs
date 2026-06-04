//! E2E tests for the admin dashboard flow.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_health_endpoint_returns_200() {
    let app = spawn_test_app().await;

    let response = app.get("/health").await;
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("failed to parse response");
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_ready_endpoint_returns_200() {
    let app = spawn_test_app().await;

    let response = app.get("/ready").await;
    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("failed to parse response");
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_db_is_initialized_with_default_org() {
    let app = spawn_test_app().await;
    let pool = app.db_pool.sqlite();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizations")
        .fetch_one(pool)
        .await
        .expect("failed to query organizations");

    // SQLite in-memory schema seeds a default organization
    assert!(count.0 >= 1, "Expected at least 1 organization");
}

#[tokio::test]
async fn test_admin_flow_create_org_user_api_key() {
    let app = spawn_test_app().await;

    let org_id = fixtures::create_org(&app.db_pool, "Test Org").await;
    let user_id = fixtures::create_user(&app.db_pool, org_id, "test@example.com", "admin").await;
    let key_id = fixtures::create_api_key(&app.db_pool, org_id, user_id, "Test Key", "hash123").await;

    // Verify data exists in DB
    let pool = app.db_pool.sqlite();

    let org_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizations WHERE id = ?1")
        .bind(org_id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("failed to query org");
    assert_eq!(org_count.0, 1);

    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?1")
        .bind(user_id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("failed to query user");
    assert_eq!(user_count.0, 1);

    let key_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM api_keys WHERE id = ?1")
        .bind(key_id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("failed to query api key");
    assert_eq!(key_count.0, 1);
}

#[tokio::test]
async fn test_request_is_logged_to_db() {
    let app = spawn_test_app().await;
    let (api_key, _hash, _prefix) = gateway_auth::generate_api_key();

    let initial_count = fixtures::count_requests(&app.db_pool).await;

    let _response = app
        .post_json_auth(
            "/v1/chat/completions",
            &api_key,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

    // Give the fire-and-forget DB logger time to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let final_count = fixtures::count_requests(&app.db_pool).await;
    assert!(
        final_count > initial_count,
        "Expected request to be logged ({} > {})",
        final_count,
        initial_count
    );
}
