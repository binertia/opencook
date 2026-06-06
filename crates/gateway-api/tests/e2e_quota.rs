//! E2E tests for quota enforcement.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_quota_can_be_created_and_queried() {
    let app = spawn_test_app().await;
    let org_id = fixtures::create_org(&app.db_pool, "Quota Test Org").await;

    let quota_id = fixtures::create_quota(
        &app.db_pool,
        org_id,
        "Rate Limit",
        "requests",
        "minute",
        "100",
    )
    .await;

    let pool = app.db_pool.sqlite();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM quotas WHERE id = ?1")
        .bind(quota_id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .expect("failed to query quota");
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn test_quota_endpoints_exist() {
    let app = spawn_test_app().await;
    let org_id = fixtures::create_org(&app.db_pool, "Quota API Test Org").await;

    // The quota admin endpoints require auth (API key)
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .client
        .get(format!("{}/api/v1/organizations/{}/quotas", app.base_url(), org_id))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("failed to send request");

    // Should not 404 — endpoint exists. May return empty list or error.
    assert_ne!(response.status(), 404, "Quota endpoint should exist");
}
