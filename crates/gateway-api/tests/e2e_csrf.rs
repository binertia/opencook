//! E2E tests for CSRF protection.

mod helpers;
use helpers::test_app::spawn_test_app;
use uuid::Uuid;

const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn login_admin(app: &helpers::test_app::TestApp, email: &str) -> (String, String) {
    let org_id = Uuid::parse_str(DEFAULT_ORG_ID).unwrap();
    let password = "MyStr0ng!Pass";
    let hasher = gateway_auth::PasswordHasherService::new();
    let hash = hasher.hash_password(password).unwrap();
    let user_id = Uuid::new_v4();
    let pool = app.db_pool.sqlite();

    let _ = sqlx::query(
        "INSERT OR IGNORE INTO organizations (id, name, slug, status, settings, plan_tier) VALUES (?1, 'Default Org', 'default', 'active', '{}', 'free')"
    )
    .bind(org_id.as_bytes().as_slice())
    .execute(pool)
    .await;

    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, ?4, 'CSRF Admin', 'admin', 'active')
        "#,
    )
    .bind(user_id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(email)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("failed to insert admin user");

    let login_resp = app
        .client
        .post(format!("{}/v1/auth/login", app.base_url()))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("failed to login");
    assert!(login_resp.status().is_success());
    let body: serde_json::Value = login_resp.json().await.expect("failed to parse login");
    let token = body["access_token"].as_str().unwrap().to_string();
    let csrf = body["csrf_token"].as_str().unwrap().to_string();
    (token, csrf)
}

#[tokio::test]
async fn test_csrf_token_returned_on_login() {
    let app = spawn_test_app().await;
    let (_, csrf_token) = login_admin(&app, "csrf-login@example.com").await;
    assert!(!csrf_token.is_empty());
    assert_eq!(csrf_token.len(), 64); // 32 bytes hex-encoded
}

#[tokio::test]
async fn test_csrf_blocks_state_changing_request_without_token() {
    let app = spawn_test_app().await;
    let (token, _csrf) = login_admin(&app, "csrf-block@example.com").await;
    let org_id = Uuid::parse_str(DEFAULT_ORG_ID).unwrap();

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/organizations/{}/quotas",
            app.base_url(),
            org_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "name": "Test Quota",
            "metric": "requests",
            "period": "minute",
            "limit_value": "100",
            "applies_to": "org",
            "action": "block"
        }))
        .send()
        .await
        .expect("failed to send request");

    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.expect("failed to parse error");
    assert_eq!(body["error"]["code"], "csrf_token_missing_or_invalid");
}

#[tokio::test]
async fn test_csrf_allows_state_changing_request_with_token() {
    let app = spawn_test_app().await;
    let (token, csrf) = login_admin(&app, "csrf-allow@example.com").await;
    let org_id = Uuid::parse_str(DEFAULT_ORG_ID).unwrap();

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/organizations/{}/quotas",
            app.base_url(),
            org_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .header("X-CSRF-Token", &csrf)
        .json(&serde_json::json!({
            "name": "Test Quota",
            "metric": "requests",
            "period": "minute",
            "limit_value": "100",
            "applies_to": "org",
            "action": "block"
        }))
        .send()
        .await
        .expect("failed to send request");

    // Should succeed (201 or 200). Even if validation fails, it shouldn't be 403 CSRF.
    assert_ne!(resp.status(), 403);
}

#[tokio::test]
async fn test_csrf_skipped_for_get_requests() {
    let app = spawn_test_app().await;
    let (token, _csrf) = login_admin(&app, "csrf-get@example.com").await;
    let org_id = Uuid::parse_str(DEFAULT_ORG_ID).unwrap();

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log",
            app.base_url(),
            org_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("failed to send request");

    // Should not be blocked by CSRF (GET is safe method)
    assert_ne!(resp.status(), 403);
}
