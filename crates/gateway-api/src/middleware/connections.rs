//! Active connection tracking middleware.
//!
//! Increments/decrements `gateway_active_connections` gauge for every
//! in-flight request. Uses the same Tower Layer/Service pattern as TimingLayer.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::{
    body::Body,
    http::Request,
    response::Response,
};
use tower::{Layer, Service};

/// Tower layer that tracks active HTTP connections.
#[derive(Debug, Clone, Default)]
pub struct ConnectionLayer;

impl<S> Layer<S> for ConnectionLayer {
    type Service = ConnectionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectionService { inner }
    }
}

/// Tower service that wraps requests with connection counting.
#[derive(Debug, Clone)]
pub struct ConnectionService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for ConnectionService<S>
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
        gateway_observability::metrics::inc_active_connections();

        let fut = self.inner.call(req);

        Box::pin(async move {
            let result = fut.await;
            gateway_observability::metrics::dec_active_connections();
            result
        })
    }
}
