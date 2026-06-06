//! E2E tests for the audit log system.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;
use uuid::Uuid;

/// Default organization ID used by API-key auth in the test harness.
const DEFAULT_ORG_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn login_admin(app: &helpers::test_app::TestApp, email: &str) -> String {
    let org_id = Uuid::parse_str(DEFAULT_ORG_ID).unwrap();
    let password = "MyStr0ng!Pass";
    let hasher = gateway_auth::PasswordHasherService::new();
    let hash = hasher.hash_password(password).unwrap();
    let user_id = Uuid::new_v4();
    let pool = app.db_pool.sqlite();

    // Ensure default org exists.
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO organizations (id, name, slug, status, settings, plan_tier) VALUES (?1, 'Default Org', 'default', 'active', '{}', 'free')"
    )
    .bind(org_id.as_bytes().as_slice())
    .execute(pool)
    .await;

    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, ?4, 'Audit Admin', 'admin', 'active')
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
    body["access_token"].as_str().unwrap().to_string()
}

async fn login_viewer(app: &helpers::test_app::TestApp, email: &str) -> String {
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
        VALUES (?1, ?2, ?3, ?4, 'Audit Viewer', 'viewer', 'active')
        "#,
    )
    .bind(user_id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind(email)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("failed to insert viewer user");

    let login_resp = app
        .client
        .post(format!("{}/v1/auth/login", app.base_url()))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("failed to login");
    assert!(login_resp.status().is_success());
    let body: serde_json::Value = login_resp.json().await.expect("failed to parse login");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_audit_log_records_api_key_lifecycle() {
    let app = spawn_test_app().await;
    let admin_token = login_admin(&app, "audit-admin@example.com").await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    // 1. Create an API key — should produce an audit entry in the default org.
    let create_resp = app
        .client
        .post(format!("{}/v1/api-keys", app.base_url()))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "name": "Audit Test Key",
            "scopes": ["chat"],
            "rate_limit_rps": 5,
        }))
        .send()
        .await
        .expect("Failed to create API key");
    assert!(
        create_resp.status().is_success(),
        "Create API key failed: {:?}",
        create_resp.text().await
    );

    // 2. List audit entries for the default org.
    let list_resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log",
            app.base_url(),
            DEFAULT_ORG_ID
        ))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to list audit entries");
    assert!(
        list_resp.status().is_success(),
        "List audit failed: {:?}",
        list_resp.text().await
    );
    let list_body: serde_json::Value = list_resp.json().await.expect("Failed to parse audit list");
    assert_eq!(list_body["object"], "list");
    let data = list_body["data"].as_array().expect("data is array");
    assert!(!data.is_empty(), "Expected at least one audit entry");

    let created_entry = data
        .iter()
        .find(|e| e["action"] == "api_key.created")
        .expect("Missing api_key.created audit entry");
    assert_eq!(created_entry["entity_type"], "api_key");
    assert_eq!(created_entry["summary"], "API key created");
    assert!(created_entry["new_values"]["name"]
        .as_str()
        .unwrap()
        .contains("Audit Test Key"));

    // 3. Filter by action.
    let filtered_resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log?action=api_key.created",
            app.base_url(),
            DEFAULT_ORG_ID
        ))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to filter audit entries");
    let filtered_body: serde_json::Value = filtered_resp
        .json()
        .await
        .expect("Failed to parse filtered audit");
    let filtered = filtered_body["data"].as_array().unwrap();
    assert!(!filtered.is_empty());
    assert!(filtered.iter().all(|e| e["action"] == "api_key.created"));

    // 4. Get single entry by ID.
    let entry_id = created_entry["id"].as_str().unwrap();
    let get_resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log/{}",
            app.base_url(),
            DEFAULT_ORG_ID,
            entry_id
        ))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to get audit entry");
    assert!(
        get_resp.status().is_success(),
        "Get audit entry failed: {:?}",
        get_resp.text().await
    );
    let get_body: serde_json::Value = get_resp.json().await.expect("Failed to parse audit entry");
    assert_eq!(get_body["object"], "audit_entry");
    assert_eq!(get_body["data"]["id"], entry_id);
}

#[tokio::test]
async fn test_audit_log_pagination() {
    let app = spawn_test_app().await;
    let admin_token = login_admin(&app, "audit-admin-paginate@example.com").await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    for name in ["Key One", "Key Two"] {
        let resp = app
            .client
            .post(format!("{}/v1/api-keys", app.base_url()))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({ "name": name, "scopes": ["chat"] }))
            .send()
            .await
            .expect("Failed to create API key");
        assert!(resp.status().is_success());
    }

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log?limit=1&offset=0",
            app.base_url(),
            DEFAULT_ORG_ID
        ))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to list audit entries");
    let body: serde_json::Value = resp.json().await.expect("Failed to parse audit list");
    assert_eq!(body["limit"], 1);
    assert_eq!(body["offset"], 0);
    assert!(body["total"].as_i64().unwrap() >= 2);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_viewer_cannot_access_audit_log() {
    let app = spawn_test_app().await;
    let viewer_token = login_viewer(&app, "audit-viewer@example.com").await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log",
            app.base_url(),
            DEFAULT_ORG_ID
        ))
        .header("Authorization", format!("Bearer {}", viewer_token))
        .send()
        .await
        .expect("Failed to request audit log");
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_audit_log_cross_org_access_denied() {
    let app = spawn_test_app().await;
    let admin_token = login_admin(&app, "audit-admin-cross@example.com").await;
    let other_org = Uuid::new_v4();

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/organizations/{}/audit-log",
            app.base_url(),
            other_org
        ))
        .header("Authorization", format!("Bearer {}", admin_token))
        .send()
        .await
        .expect("Failed to request audit log");
    assert_eq!(resp.status(), 403);
}
