//! Request timing and trace ID middleware.
//!
//! Injects `X-Gateway-Request-ID` (UUID v7) into every request and adds
//! timing headers to every response:
//!
//! - `X-Gateway-Request-ID` — time-sortable request trace ID
//! - `X-Total-Latency-Ms` — total wall-clock time for the request
//! - `X-Gateway-Latency-Ms` — time spent inside the gateway (total - provider)
//! - `X-Provider-Latency-Ms` — time spent waiting for the upstream provider
//! - `X-Request-Time-Ms` — backward-compatible alias for total latency
//!
//! Handlers can report provider latency by setting the `X-Provider-Latency-Ms`
//! response header before returning. If absent, provider latency is assumed
//! to be zero and gateway latency equals total latency.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::{
    body::Body,
    http::{HeaderValue, Request, Response},
};
use tower::{Layer, Service};
use uuid::Uuid;

/// Request extension key for the trace ID.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Request extension key for the request start time.
#[derive(Debug, Clone, Copy)]
pub struct RequestStart(pub Instant);

/// Request extension key for provider latency reported by handlers.
#[derive(Debug, Clone, Copy)]
pub struct ProviderLatencyMs(pub u64);

/// Tower layer that adds request IDs and timing to all requests.
#[derive(Debug, Clone, Default)]
pub struct TimingLayer;

impl<S> Layer<S> for TimingLayer {
    type Service = TimingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimingService { inner }
    }
}

/// Tower service that wraps requests with timing.
#[derive(Debug, Clone)]
pub struct TimingService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for TimingService<S>
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

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let start = Instant::now();
        let request_id = Uuid::now_v7().to_string();

        // Store request ID and start time in extensions for handlers / downstream middleware.
        req.extensions_mut().insert(RequestId(request_id.clone()));
        req.extensions_mut().insert(RequestStart(start));

        let fut = self.inner.call(req);

        Box::pin(async move {
            let result = fut.await;
            let total_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(mut resp) => {
                    // Read provider latency from response headers if the handler reported it.
                    let provider_ms = resp
                        .headers()
                        .get("x-provider-latency-ms")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);

                    let gateway_ms = total_ms.saturating_sub(provider_ms);

                    let headers = resp.headers_mut();
                    headers.insert(
                        "x-gateway-request-id",
                        HeaderValue::from_str(&request_id)
                            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                    );
                    headers.insert(
                        "x-total-latency-ms",
                        HeaderValue::from_str(&total_ms.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );
                    headers.insert(
                        "x-gateway-latency-ms",
                        HeaderValue::from_str(&gateway_ms.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );
                    headers.insert(
                        "x-provider-latency-ms",
                        HeaderValue::from_str(&provider_ms.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );
                    // Latency SLA header (default 5000ms; org-specific override planned).
                    headers.insert("x-gateway-latency-sla", HeaderValue::from_static("5000"));
                    // Backward-compatible alias.
                    headers.insert(
                        "x-request-time-ms",
                        HeaderValue::from_str(&total_ms.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );

                    Ok(resp)
                }
                Err(e) => Err(e),
            }
        })
    }
}

/// Helper to extract the request ID from request extensions.
pub fn request_id(req: &Request<Body>) -> Option<String> {
    req.extensions().get::<RequestId>().map(|r| r.0.clone())
}

/// Helper to extract the request start time from request extensions.
pub fn request_start(req: &Request<Body>) -> Option<Instant> {
    req.extensions().get::<RequestStart>().map(|r| r.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, Response, StatusCode};
    use std::convert::Infallible;
    use std::future::ready;
    use std::task::{Context, Poll};
    use tower::Service;

    #[derive(Clone)]
    struct DummyService;

    impl Service<Request<Body>> for DummyService {
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
    async fn test_adds_request_id_header() {
        let mut service = TimingLayer.layer(DummyService);
        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = service.call(req).await.unwrap();

        assert!(resp.headers().contains_key("x-gateway-request-id"));
        let id = resp.headers()["x-gateway-request-id"].to_str().unwrap();
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn test_adds_timing_headers() {
        let mut service = TimingLayer.layer(DummyService);
        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = service.call(req).await.unwrap();

        assert!(resp.headers().contains_key("x-total-latency-ms"));
        assert!(resp.headers().contains_key("x-gateway-latency-ms"));
        assert!(resp.headers().contains_key("x-provider-latency-ms"));
        assert!(resp.headers().contains_key("x-request-time-ms"));
    }

    #[tokio::test]
    async fn test_computes_gateway_latency_from_provider_header() {
        #[derive(Clone)]
        struct ProviderLatencyService;

        impl Service<Request<Body>> for ProviderLatencyService {
            type Response = Response<Body>;
            type Error = Infallible;
            type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _req: Request<Body>) -> Self::Future {
                Box::pin(async move {
                    // Sleep enough to guarantee non-zero total latency in milliseconds.
                    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("x-provider-latency-ms", "50")
                        .body(Body::empty())
                        .unwrap())
                })
            }
        }

        let mut service = TimingLayer.layer(ProviderLatencyService);
        let req = Request::builder().body(Body::empty()).unwrap();
        let resp = service.call(req).await.unwrap();

        let provider_ms: u64 = resp.headers()["x-provider-latency-ms"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let total_ms: u64 = resp.headers()["x-total-latency-ms"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let gateway_ms: u64 = resp.headers()["x-gateway-latency-ms"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();

        assert_eq!(provider_ms, 50);
        assert!(total_ms >= provider_ms);
        assert_eq!(gateway_ms, total_ms - provider_ms);
    }
}
