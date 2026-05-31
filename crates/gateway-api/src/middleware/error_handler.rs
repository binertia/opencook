//! Global error handling middleware.
//!
//! Catches all handler errors and returns structured JSON with request ID.
//! Internal details are never leaked to the client.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use tower::{Layer, Service};

use super::timing::RequestId;

/// Tower layer for global error handling.
#[derive(Debug, Clone, Default)]
pub struct ErrorHandlerLayer;

impl<S> Layer<S> for ErrorHandlerLayer {
    type Service = ErrorHandlerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ErrorHandlerService { inner }
    }
}

/// Tower service that wraps errors in structured JSON.
#[derive(Debug, Clone)]
pub struct ErrorHandlerService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for ErrorHandlerService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.inner.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let request_id = req
            .extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let fut = self.inner.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(resp) => Ok(resp),
                Err(_) => {
                    // Generic error fallback — handler errors should be caught
                    // by axum's error handling before reaching this point.
                    // This is a last-resort safety net.
                    let body = ErrorResponse {
                        error: ErrorDetail {
                            code: "gateway_error",
                            message: "An internal error occurred",
                            request_id: &request_id,
                        },
                    };
                    Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response())
                }
            }
        })
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
    request_id: &'a str,
}
