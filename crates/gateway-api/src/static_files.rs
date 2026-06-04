//! Static file serving for the React SPA dashboard.
//!
//! Serves files from `frontend/dist/` at `/admin/*`.
//! Any non-asset path falls back to `index.html` for client-side routing.

use std::path::PathBuf;

use axum::{http::StatusCode, Router};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

/// Check if we're running in development mode.
fn is_dev() -> bool {
    std::env::var("APP_ENV")
        .map(|v| v == "development")
        .unwrap_or(false)
}

/// Find the frontend dist directory, checking common locations.
fn find_dist_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("frontend/dist"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend/dist"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }
    None
}

/// Build the static file router for `/admin/*`.
///
/// In development mode, this returns an empty router (Vite dev server
/// is expected to serve the frontend separately).
pub fn build_static_router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    if is_dev() {
        tracing::info!("APP_ENV=development — skipping static file serving");
        return Router::new();
    }

    let Some(dist) = find_dist_dir() else {
        tracing::warn!(
            "Static asset directory not found. \
             Dashboard will not be available. Run `cd frontend && npm run build` first."
        );
        return Router::new().fallback(fallback_no_dist);
    };

    let index_html = dist.join("index.html");

    // ServeDir with SPA fallback: any missing file serves index.html
    let serve_dir = ServeDir::new(&dist).fallback(ServeFile::new(&index_html));

    // Cache headers for asset files (1 year for hashed assets)
    let cache_layer = SetResponseHeaderLayer::if_not_present(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Router::new()
        .nest_service("/admin", serve_dir)
        .layer(CompressionLayer::new())
        .layer(cache_layer)
}

/// Fallback handler when the dist directory is missing.
async fn fallback_no_dist() -> (StatusCode, &'static str) {
    (
        StatusCode::NOT_FOUND,
        "Dashboard not built. Run `cd frontend && npm run build`.",
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    async fn body_to_string(body: Body) -> String {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn test_dev_mode_returns_empty_router() {
        {
            let _guard = ENV_LOCK.lock().unwrap();
            std::env::set_var("APP_ENV", "development");
        }
        let app: Router = build_static_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        {
            let _guard = ENV_LOCK.lock().unwrap();
            std::env::remove_var("APP_ENV");
        }
    }

    #[tokio::test]
    async fn test_admin_serves_index_html() {
        {
            let _guard = ENV_LOCK.lock().unwrap();
            std::env::remove_var("APP_ENV");
        }
        let app: Router = build_static_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_string(response.into_body()).await;
        assert!(body.contains("<html") || body.contains("<script"));
    }

    #[tokio::test]
    async fn test_admin_sp_fallback_serves_index_html() {
        {
            let _guard = ENV_LOCK.lock().unwrap();
            std::env::remove_var("APP_ENV");
        }
        let app: Router = build_static_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/providers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_string(response.into_body()).await;
        assert!(body.contains("<html") || body.contains("<script"));
    }

    #[tokio::test]
    async fn test_admin_assets_returns_actual_file() {
        {
            let _guard = ENV_LOCK.lock().unwrap();
            std::env::remove_var("APP_ENV");
        }
        let app: Router = build_static_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/assets/index-fdO9d0TQ.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Should have cache-control header
        let cache = response
            .headers()
            .get("cache-control")
            .expect("Missing Cache-Control header");
        assert!(cache.to_str().unwrap().contains("max-age=31536000"));

        let body = body_to_string(response.into_body()).await;
        // CSS file should contain style rules
        assert!(
            body.contains("{") || body.contains("html"),
            "Expected asset file to contain CSS content, got: {}",
            &body[..body.len().min(200)]
        );
    }
}
