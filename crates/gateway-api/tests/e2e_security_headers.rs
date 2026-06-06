//! E2E tests for security headers and CORS configuration.

mod helpers;
use helpers::test_app::spawn_test_app;

#[tokio::test]
async fn test_security_headers_present_on_all_responses() {
    let app = spawn_test_app().await;

    let response = app.get("/health").await;
    assert_eq!(response.status(), 200);

    let headers = response.headers();
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert!(headers["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("default-src"));
    assert_eq!(headers["x-xss-protection"], "1; mode=block");
    assert_eq!(
        headers["referrer-policy"],
        "strict-origin-when-cross-origin"
    );
}

#[tokio::test]
async fn test_cors_preflight_succeeds_for_allowed_origin() {
    let app = spawn_test_app().await;
    let client = reqwest::Client::new();

    let response = client
        .request(
            reqwest::Method::from_bytes(b"OPTIONS").unwrap(),
            format!("{}/health", app.base_url()),
        )
        .header("Origin", "http://localhost:5173")
        .header("Access-Control-Request-Method", "GET")
        .header(
            "Access-Control-Request-Headers",
            "Content-Type, Authorization",
        )
        .send()
        .await
        .expect("failed to send preflight");

    // The default config allows any origin (development fallback).
    assert_eq!(response.status(), 200);
    let headers = response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));
    assert!(headers.contains_key("access-control-allow-methods"));
}

#[tokio::test]
async fn test_tls_config_loads_from_pem_files() {
    // Generate a temporary self-signed certificate and key.
    let temp_dir = tempfile::tempdir().unwrap();
    let cert_path = temp_dir.path().join("cert.pem");
    let key_path = temp_dir.path().join("key.pem");

    let cert_output = std::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            key_path.to_str().unwrap(),
            "-out",
            cert_path.to_str().unwrap(),
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=localhost",
        ])
        .output()
        .expect("openssl must be installed for this test");

    assert!(
        cert_output.status.success(),
        "openssl failed: {}",
        String::from_utf8_lossy(&cert_output.stderr)
    );

    let tls_config = gateway_api::tls::TlsConfig::from_env(
        cert_path.to_str().unwrap(),
        key_path.to_str().unwrap(),
    );
    let server_config = tls_config.to_server_config();
    assert!(
        server_config.is_ok(),
        "TLS config should load from PEM files"
    );
}
