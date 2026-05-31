//! Request timing and trace ID middleware.
//!
//! Injects `X-Gateway-Request-ID` (UUID v7) into every request and adds
//! timing headers to every response.

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

        // Store request ID in extensions for handlers to access
        req.extensions_mut().insert(RequestId(request_id.clone()));

        let fut = self.inner.call(req);

        Box::pin(async move {
            let result = fut.await;
            let elapsed_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(mut resp) => {
                    let headers = resp.headers_mut();
                    headers.insert(
                        "x-gateway-request-id",
                        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                    );
                    headers.insert(
                        "x-request-time-ms",
                        HeaderValue::from_str(&elapsed_ms.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
                    );
                    Ok(resp)
                }
                Err(e) => Err(e),
            }
        })
    }
}
