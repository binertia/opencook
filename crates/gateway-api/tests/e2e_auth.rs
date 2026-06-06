//! E2E tests for authentication endpoints.

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_auth_login_with_valid_credentials() {
    let app = spawn_test_app().await;

    // Create a user with a properly hashed password
    let org_id = helpers::fixtures::create_org(&app.db_pool, "Auth Test Org").await;
    let password = "MyStr0ng!Pass";
    let hasher = gateway_auth::PasswordHasherService::new();
    let hash = hasher.hash_password(password).unwrap();

    let user_id = uuid::Uuid::new_v4();
    let pool = app.db_pool.sqlite();
    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, ?4, 'Auth Test User', 'admin', 'active')
        "#,
    )
    .bind(user_id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind("auth-test@example.com")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("failed to insert user");

    // Login
    let response = app
        .post_json(
            "/v1/auth/login",
            serde_json::json!({
                "email": "auth-test@example.com",
                "password": password
            }),
        )
        .await;

    assert_eq!(
        response.status(),
        200,
        "Expected 200, got {}",
        response.status()
    );

    let body: serde_json::Value = response
        .json()
        .await
        .expect("failed to parse login response");
    assert!(!body["access_token"].as_str().unwrap().is_empty());
    assert!(!body["refresh_token"].as_str().unwrap().is_empty());
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 900);
    assert_eq!(body["user"]["email"], "auth-test@example.com");
    assert_eq!(body["user"]["role"], "admin");
}

#[tokio::test]
async fn test_auth_login_with_invalid_credentials() {
    let app = spawn_test_app().await;

    let response = app
        .post_json(
            "/v1/auth/login",
            serde_json::json!({
                "email": "nonexistent@example.com",
                "password": "wrongpassword"
            }),
        )
        .await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_auth_me_returns_user_info() {
    let app = spawn_test_app().await;

    // Create a user with a properly hashed password
    let org_id = helpers::fixtures::create_org(&app.db_pool, "Auth Test Org").await;
    let password = "MyStr0ng!Pass";
    let hasher = gateway_auth::PasswordHasherService::new();
    let hash = hasher.hash_password(password).unwrap();

    let user_id = uuid::Uuid::new_v4();
    let pool = app.db_pool.sqlite();
    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, ?4, 'Me Test User', 'member', 'active')
        "#,
    )
    .bind(user_id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind("me-test@example.com")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("failed to insert user");

    // Login to get a token
    let login_response = app
        .post_json(
            "/v1/auth/login",
            serde_json::json!({
                "email": "me-test@example.com",
                "password": password
            }),
        )
        .await;

    let login_body: serde_json::Value = login_response.json().await.expect("failed to parse login");
    let access_token = login_body["access_token"].as_str().unwrap();

    // Call /me
    let me_response = app
        .client
        .get(format!("{}/v1/auth/me", app.base_url()))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("failed to send me request");

    assert_eq!(me_response.status(), 200);

    let me_body: serde_json::Value = me_response
        .json()
        .await
        .expect("failed to parse me response");
    assert_eq!(me_body["email"], "me-test@example.com");
    assert_eq!(me_body["role"], "member");
    assert_eq!(me_body["name"], "Me Test User");
}

#[tokio::test]
async fn test_auth_me_without_token_returns_401() {
    let app = spawn_test_app().await;

    let response = app
        .client
        .get(format!("{}/v1/auth/me", app.base_url()))
        .send()
        .await
        .expect("failed to send me request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_auth_refresh_issues_new_access_token() {
    let app = spawn_test_app().await;

    // Create a user with a properly hashed password
    let org_id = helpers::fixtures::create_org(&app.db_pool, "Auth Test Org").await;
    let password = "MyStr0ng!Pass";
    let hasher = gateway_auth::PasswordHasherService::new();
    let hash = hasher.hash_password(password).unwrap();

    let user_id = uuid::Uuid::new_v4();
    let pool = app.db_pool.sqlite();
    sqlx::query(
        r#"
        INSERT INTO users (id, org_id, email, password_hash, display_name, role, status)
        VALUES (?1, ?2, ?3, ?4, 'Refresh Test User', 'viewer', 'active')
        "#,
    )
    .bind(user_id.as_bytes().as_slice())
    .bind(org_id.as_bytes().as_slice())
    .bind("refresh-test@example.com")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("failed to insert user");

    // Login to get tokens
    let login_response = app
        .post_json(
            "/v1/auth/login",
            serde_json::json!({
                "email": "refresh-test@example.com",
                "password": password
            }),
        )
        .await;

    let login_body: serde_json::Value = login_response.json().await.expect("failed to parse login");
    let refresh_token = login_body["refresh_token"].as_str().unwrap();

    // Refresh
    let refresh_response = app
        .post_json(
            "/v1/auth/refresh",
            serde_json::json!({
                "refresh_token": refresh_token
            }),
        )
        .await;

    assert_eq!(refresh_response.status(), 200);

    let refresh_body: serde_json::Value = refresh_response
        .json()
        .await
        .expect("failed to parse refresh");
    assert!(!refresh_body["access_token"].as_str().unwrap().is_empty());
    assert!(!refresh_body["refresh_token"].as_str().unwrap().is_empty());
    assert_eq!(refresh_body["user"]["email"], "refresh-test@example.com");
}
