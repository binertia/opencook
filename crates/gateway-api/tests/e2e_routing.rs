//! E2E tests for the routing engine.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_routing_rule_can_be_created_and_queried() {
    let app = spawn_test_app().await;
    let org_id = fixtures::create_org(&app.db_pool, "Routing Test Org").await;

    let rule_id = fixtures::create_routing_rule(
        &app.db_pool,
        org_id,
        "Fallback Rule",
        "fallback",
        Some("gpt-4o"),
    )
    .await;

    let pool = app.db_pool.sqlite();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM routing_rules WHERE id = ?1")
        .bind(rule_id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("failed to query routing rule");
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn test_request_with_valid_api_key_reaches_chat_endpoint() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .post_json_auth(
            "/v1/chat/completions",
            &api_key,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Test routing"}]
            }),
        )
        .await;

    // Should reach the endpoint (auth passes) and return a response
    assert_eq!(response.status(), 200);
}
