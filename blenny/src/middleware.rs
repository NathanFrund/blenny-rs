// blenny/src/middleware.rs
use axum::{
    body::Body,
    http::Request,
    response::{IntoResponse, Response},
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::error::BlennyError;

/// Layer that catches panics in downstream services and returns a 500 error.
#[derive(Clone)]
pub struct AntiFragileLayer;

impl<S> Layer<S> for AntiFragileLayer {
    type Service = AntiFragileService<S>;

    fn layer(&self, service: S) -> Self::Service {
        AntiFragileService { inner: service }
    }
}

#[derive(Clone)]
pub struct AntiFragileService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for AntiFragileService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let future = self.inner.call(req);
        Box::pin(async move {
            tokio::task::spawn(future).await.unwrap_or_else(|_| {
                // Panic caught: return 500
                Ok(BlennyError::Internal("Request handler panicked".into()).into_response())
            })
        })
    }
}
