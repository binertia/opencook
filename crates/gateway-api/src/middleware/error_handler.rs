//! Global error handling and response logging middleware.
//!
//! Inspects every response and logs client errors at WARN level and server
//! errors at ERROR level, always including the request ID. This middleware
//! does not mutate successful or client-error responses; it is a safety net
//! for unhandled service errors and an observability hook for error
//! responses returned by handlers.

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

/// Tower service that logs error responses and catches unhandled errors.
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
        let method = req.method().to_string();
        let path = req.uri().path().to_string();

        let fut = self.inner.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_server_error() {
                        tracing::error!(
                            request_id = %request_id,
                            method = %method,
                            path = %path,
                            status = status.as_u16(),
                            "Server error response"
                        );
                    } else if status.is_client_error() {
                        tracing::warn!(
                            request_id = %request_id,
                            method = %method,
                            path = %path,
                            status = status.as_u16(),
                            "Client error response"
                        );
                    }
                    Ok(resp)
                }
                Err(_) => {
                    // Last-resort safety net: the handler returned an Err that
                    // was not converted into a Response by Axum. This should
                    // not happen in practice because ApiError implements
                    // IntoResponse, but we handle it defensively.
                    tracing::error!(
                        request_id = %request_id,
                        method = %method,
                        path = %path,
                        "Unhandled service error"
                    );
                    let body = ErrorResponse {
                        error: ErrorDetail {
                            code: "gateway_error",
                            message: "An internal error occurred",
                            r#type: "gateway_error",
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
    #[serde(rename = "type")]
    r#type: &'a str,
    request_id: &'a str,
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
    struct OkService;

    impl Service<Request<Body>> for OkService {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, mut req: Request<Body>) -> Self::Future {
            let request_id = req
                .extensions_mut()
                .get::<RequestId>()
                .map(|r| r.0.clone())
                .unwrap_or_default();
            Box::pin(ready(Ok(Response::builder()
                .status(StatusCode::OK)
                .header("x-gateway-request-id", request_id)
                .body(Body::empty())
                .unwrap())))
        }
    }

    #[tokio::test]
    async fn test_passes_through_ok_response() {
        let mut service = ErrorHandlerLayer.layer(OkService);
        let req = Request::builder()
            .extension(RequestId("req-123".to_string()))
            .body(Body::empty())
            .unwrap();
        let resp = service.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()["x-gateway-request-id"].to_str().unwrap(),
            "req-123"
        );
    }

    #[tokio::test]
    async fn test_returns_json_for_unhandled_error() {
        #[derive(Clone)]
        struct FailService;

        impl Service<Request<Body>> for FailService {
            type Response = Response<Body>;
            type Error = std::io::Error;
            type Future =
                Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _req: Request<Body>) -> Self::Future {
                Box::pin(ready(Err(std::io::Error::other(
                    "boom",
                ))))
            }
        }

        let mut service = ErrorHandlerLayer.layer(FailService);
        let req = Request::builder()
            .extension(RequestId("req-456".to_string()))
            .body(Body::empty())
            .unwrap();
        let resp = service.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
