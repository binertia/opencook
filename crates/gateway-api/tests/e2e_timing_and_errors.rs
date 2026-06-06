//! E2E tests for request timing headers and structured error responses.

mod helpers;
use helpers::fixtures;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_success_response_includes_timing_headers() {
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

    assert_eq!(response.status(), 200);

    let headers = response.headers();
    assert!(
        headers.contains_key("x-gateway-request-id"),
        "missing x-gateway-request-id"
    );
    assert!(
        headers.contains_key("x-total-latency-ms"),
        "missing x-total-latency-ms"
    );
    assert!(
        headers.contains_key("x-gateway-latency-ms"),
        "missing x-gateway-latency-ms"
    );
    assert!(
        headers.contains_key("x-provider-latency-ms"),
        "missing x-provider-latency-ms"
    );
    assert!(
        headers.contains_key("x-request-time-ms"),
        "missing x-request-time-ms"
    );

    let total: u64 = headers["x-total-latency-ms"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let gateway: u64 = headers["x-gateway-latency-ms"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let provider: u64 = headers["x-provider-latency-ms"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    assert!(
        total >= gateway,
        "total latency should be >= gateway latency"
    );
    assert_eq!(
        gateway + provider,
        total,
        "gateway + provider should equal total"
    );
}

#[tokio::test]
async fn test_error_response_includes_trace_id_and_error_code() {
    let app = spawn_test_app().await;

    // Missing auth triggers a 401 with structured error body.
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

    let headers = response.headers();
    assert!(
        headers.contains_key("x-gateway-request-id"),
        "error response should include x-gateway-request-id"
    );

    let body: serde_json::Value = response.json().await.expect("failed to parse error body");
    let error = body.get("error").expect("error field missing");
    assert!(error.get("code").is_some(), "error.code missing");
    assert!(error.get("message").is_some(), "error.message missing");
    assert!(error.get("type").is_some(), "error.type missing");
    assert!(
        error.get("request_id").is_some(),
        "error.request_id missing"
    );
}

#[tokio::test]
async fn test_internal_errors_not_exposed_to_client() {
    let app = spawn_test_app().await;

    // Missing auth on a protected endpoint triggers a 401; the body should
    // not leak internal paths or stack traces.
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

    let headers = response.headers();
    assert!(
        headers.contains_key("x-gateway-request-id"),
        "error response should include x-gateway-request-id"
    );

    let body: serde_json::Value = response.json().await.expect("failed to parse error body");
    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(
        !message.contains("stack trace"),
        "error body should not contain stack traces: {message}"
    );
    assert!(
        !message.contains("internal server"),
        "client error body should not contain internal server details: {message}"
    );
}
