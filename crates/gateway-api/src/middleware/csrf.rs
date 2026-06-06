//! CSRF protection middleware — double-submit cookie pattern.
//!
//! For state-changing requests to `/api/v1/*` routes, the `X-CSRF-Token`
//! header must match the `csrf_token` cookie set at login.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use tower_cookies::{Cookie, Cookies};

use crate::error::ApiError;

/// Name of the CSRF cookie.
pub const CSRF_COOKIE_NAME: &str = "csrf_token";

/// Name of the header that carries the CSRF token.
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

/// Generate a new CSRF token (32 random bytes, hex-encoded).
pub fn generate_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(bytes)
}

/// Set the CSRF token cookie on the response.
pub fn set_csrf_cookie(cookies: &Cookies, token: &str, secure: bool) {
    let mut cookie = Cookie::new(CSRF_COOKIE_NAME, token.to_string());
    cookie.set_path("/");
    cookie.set_same_site(tower_cookies::cookie::SameSite::Strict);
    cookie.set_http_only(false); // frontend must read it for double-submit
    cookie.set_secure(secure);
    cookies.add(cookie);
}

/// CSRF middleware: verify double-submit cookie for state-changing admin requests.
pub async fn csrf_middleware(
    cookies: Cookies,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let method = req.method().clone();
    let path = req.uri().path();

    // Only enforce CSRF on state-changing methods under /api/v1/
    let is_state_changing = matches!(
        method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::DELETE
            | axum::http::Method::PATCH
    );

    // Bearer-token-authenticated requests are inherently CSRF-safe because
    // the Authorization header is not automatically sent by the browser.
    let has_bearer_auth = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|h| h.to_lowercase().starts_with("bearer "))
        .unwrap_or(false);

    if is_state_changing && path.starts_with("/api/v1/") && !has_bearer_auth {
        let cookie_token = cookies.get(CSRF_COOKIE_NAME).map(|c| c.value().to_string());

        let header_token = req
            .headers()
            .get(CSRF_HEADER_NAME)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        match (cookie_token, header_token) {
            (Some(cookie), Some(header)) if cookie == header => {
                // Token matches; proceed.
            }
            _ => {
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "csrf_token_missing_or_invalid",
                    "CSRF token missing or invalid. Ensure you include the X-CSRF-Token header matching the csrf_token cookie.",
                ));
            }
        }
    }

    Ok(next.run(req).await)
}
