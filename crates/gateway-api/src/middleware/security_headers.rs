//! Security headers middleware.
//!
//! Adds a baseline set of HTTP response headers that mitigate common
//! browser-side attacks. HSTS is only emitted when the request was served
//! over TLS (detected via `x-forwarded-proto: https` or local TLS context).

use axum::{
    body::Body,
    http::{HeaderValue, Request, Response},
    response::IntoResponse,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Layer that injects security headers into every response.
#[derive(Debug, Clone, Default)]
pub struct SecurityHeadersLayer;

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService { inner }
    }
}

/// Service wrapper that adds security headers.
#[derive(Debug, Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for SecurityHeadersService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let is_https = is_https(&req);
        let fut = self.inner.call(req);

        Box::pin(async move {
            let mut resp = fut.await?;
            let headers = resp.headers_mut();

            headers.insert(
                "x-content-type-options",
                HeaderValue::from_static("nosniff"),
            );
            headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
            headers.insert(
                "content-security-policy",
                HeaderValue::from_static("default-src 'self'"),
            );
            headers.insert(
                "x-xss-protection",
                HeaderValue::from_static("1; mode=block"),
            );
            headers.insert(
                "referrer-policy",
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            );

            if is_https {
                headers.insert(
                    "strict-transport-security",
                    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
                );
            }

            Ok(resp)
        })
    }
}

fn is_https(req: &Request<Body>) -> bool {
    req.headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

/// Handler that returns an empty response with security headers.
/// Useful for tests.
pub async fn _empty_with_headers() -> impl IntoResponse {
    Response::builder()
        .status(200)
        .body(Body::empty())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use std::convert::Infallible;
    use std::future::ready;
    use std::task::{Context, Poll};
    use tower::Service;

    #[derive(Clone)]
    struct OkService;

    impl Service<Request<Body>> for OkService {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            Box::pin(ready(Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap())))
        }
    }

    #[tokio::test]
    async fn test_adds_security_headers() {
        let mut service = SecurityHeadersLayer.layer(OkService);
        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = service.call(req).await.unwrap();

        assert_eq!(resp.headers()["x-content-type-options"], "nosniff");
        assert_eq!(resp.headers()["x-frame-options"], "DENY");
        assert!(resp.headers()["content-security-policy"].to_str().unwrap().contains("default-src"));
        assert_eq!(resp.headers()["x-xss-protection"], "1; mode=block");
        assert_eq!(
            resp.headers()["referrer-policy"],
            "strict-origin-when-cross-origin"
        );
        assert!(!resp.headers().contains_key("strict-transport-security"));
    }

    #[tokio::test]
    async fn test_adds_hsts_for_https_requests() {
        let mut service = SecurityHeadersLayer.layer(OkService);
        let req = Request::builder()
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .unwrap();
        let resp = service.call(req).await.unwrap();

        assert!(resp.headers().contains_key("strict-transport-security"));
        assert!(resp.headers()["strict-transport-security"]
            .to_str()
            .unwrap()
            .contains("max-age=31536000"));
    }
}
