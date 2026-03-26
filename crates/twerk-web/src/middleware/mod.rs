//! Web middleware types for HTTP request/response handling.

use std::future::Future;
use std::pin::Pin;
use thiserror::Error;
use axum::http::{Request, Response, StatusCode};
use axum::middleware::Next;
use std::sync::Arc;

#[derive(Debug, Error)]
pub enum WebError {
    #[error("binding error: {0}")]
    Bind(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("response error: {0}")]
    Response(String),
}

/// A middleware function that wraps a handler.
pub type MiddlewareFunc = fn(next: HandlerFunc) -> HandlerFunc;

/// A handler function that processes HTTP requests.
pub type HandlerFunc =
    fn(ctx: Box<dyn Context + Send>) -> Pin<Box<dyn Future<Output = Result<(), WebError>> + Send>>;

/// Trait for accessing request/response context.
pub trait Context {
    fn request(&self) -> &Request<axum::body::Body>;
    fn get(&self, key: &str) -> Option<Box<dyn std::any::Any + Send + Sync>>;
    fn set(&mut self, key: String, val: Box<dyn std::any::Any + Send + Sync>);
    fn no_content(self: Pin<&mut Self>, code: u16) -> Pin<Box<dyn Future<Output = Result<(), WebError>> + Send>>;
}

/// Middleware builder for composing middleware functions.
#[derive(Default)]
pub struct MiddlewareChain {
    middlewares: Vec<MiddlewareFunc>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self { middlewares: vec![] }
    }
    #[must_use]
    pub fn add(mut self, mw: MiddlewareFunc) -> Self {
        self.middlewares.push(mw);
        self
    }
    pub fn apply(self, handler: HandlerFunc) -> HandlerFunc {
        self.middlewares
            .into_iter()
            .fold(handler, |next, mw| mw(next))
    }
}

// Axum middleware implementations from coordinator.rs

pub async fn logging_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    tracing::info!("HTTP request: {} {}", method, uri);
    let response = next.run(request).await;
    tracing::info!("HTTP response: {} {} -> {}", method, uri, response.status());
    response
}

pub async fn auth_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    // Placeholder for auth logic
    Ok(next.run(request).await)
}
