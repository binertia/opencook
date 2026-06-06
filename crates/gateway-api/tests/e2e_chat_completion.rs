//! E2E tests for the chat completion endpoint.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_chat_completion_non_streaming_returns_200() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .post_json_auth(
            "/v1/chat/completions",
            &api_key,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": false
            }),
        )
        .await;

    assert_eq!(response.status(), 200, "Expected 200, got {}", response.status());

    let body: serde_json::Value = response.json().await.expect("failed to parse response");
    assert_eq!(body["object"], "chat.completion");
    assert!(!body["choices"].as_array().unwrap().is_empty());
    assert!(body["usage"]["total_tokens"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_chat_completion_streaming_returns_sse() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .post_json_auth(
            "/v1/chat/completions",
            &api_key,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": true
            }),
        )
        .await;

    assert_eq!(response.status(), 200);

    let content_type = response
        .headers()
        .get("content-type")
        .expect("missing content-type header")
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"), "Expected SSE, got {}", content_type);
}

#[tokio::test]
async fn test_chat_completion_missing_auth_returns_401() {
    let app = spawn_test_app().await;

    let response = app
        .post_json(
            "/v1/chat/completions",
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_chat_completion_invalid_model_returns_400() {
    let app = spawn_test_app().await;
    let api_key = fixtures::setup_api_key(&app.db_pool).await;

    let response = app
        .post_json_auth(
            "/v1/chat/completions",
            &api_key,
            serde_json::json!({
                "model": "",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

    // Validation error (empty model fails request validation)
    assert_eq!(response.status(), 400);
}
