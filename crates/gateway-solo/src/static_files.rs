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
