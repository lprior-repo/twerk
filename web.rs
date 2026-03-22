//! Web middleware types for HTTP request/response handling.
//!
//! This module provides the core abstractions for building web middleware
//! in a functional style, using traits and immutable data structures.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use thiserror::Error;

/// Errors that can occur during web middleware operations.
#[derive(Debug, Error)]
pub enum WebError {
    /// Failed to bind request data to the target type.
    #[error("binding error: {0}")]
    Bind(String),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP response error.
    #[error("response error: {0}")]
    Response(String),
}

/// A middleware function that wraps a handler.
pub type MiddlewareFunc = fn(next: HandlerFunc) -> HandlerFunc;

/// A handler function that processes HTTP requests.
pub type HandlerFunc =
    fn(ctx: Context) -> Pin<Box<dyn Future<Output = Result<(), WebError>> + Send>>;

/// Trait for accessing request/response context.
///
/// This trait provides access to the HTTP context needed by handlers
/// and middleware. Implementations typically wrap Actix or Axum contexts.
pub trait Context {
    /// Returns a reference to the underlying HTTP request.
    fn request(&self) -> &crate::web::hyper::Request<()>;

    /// Retrieves data stored in the context by key.
    fn get(&self, key: &str) -> Option<Box<dyn std::any::Any + Send + Sync>>;

    /// Sets data in the context with the given key.
    fn set(&mut self, key: String, val: Box<dyn std::any::Any + Send + Sync>);

    /// Returns a reference to the HTTP response writer.
    fn response(&self) -> &mut crate::web::hyper::Response<()>;

    /// Sends a response with no body and the given status code.
    fn no_content(self: Pin<&mut Self>, code: u16) -> impl Future<Output = Result<(), WebError>> + Send;

    /// Sends a string response with the given status code.
    fn string(
        self: Pin<&mut Self>,
        code: u16,
        s: String,
    ) -> impl Future<Output = Result<(), WebError>> + Send;

    /// Sends a JSON response with the given status code.
    fn json<T: serde::Serialize>(
        self: Pin<&mut Self>,
        code: u16,
        data: &T,
    ) -> impl Future<Output = Result<(), WebError>> + Send;

    /// Binds path params, query params, and request body into the provided type.
    fn bind<T: serde::de::DeserializeOwned>(
        &self,
    ) -> impl Future<Output = Result<T, WebError>> + Send;

    /// Sends an error response to the client.
    fn error(self: Pin<&mut Self>, code: u16, err: Box<dyn std::error::Error + Send + Sync>);

    /// Returns a channel that's closed when work should be cancelled.
    fn done(&self) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// Extension trait for extracting typed values from context.
pub trait ContextExt: Context {
    /// Gets a value of type `T` from context, if it exists.
    fn get_typed<T: 'static>(&self, key: &str) -> Option<&T> {
        self.get(key)
            .and_then(|v| v.downcast_ref::<T>())
    }
}

impl<C: Context> ContextExt for C {}

/// Middleware builder for composing middleware functions.
#[derive(Default)]
pub struct MiddlewareChain {
    middlewares: Vec<MiddlewareFunc>,
}

impl MiddlewareChain {
    /// Creates a new empty middleware chain.
    pub fn new() -> Self {
        Self { middlewares: vec![] }
    }

    /// Adds a middleware to the chain.
    #[must_use]
    pub fn add(mut self, mw: MiddlewareFunc) -> Self {
        self.middlewares.push(mw);
        self
    }

    /// Applies all middleware to the given handler, returning the wrapped handler.
    pub fn apply(self, handler: HandlerFunc) -> HandlerFunc {
        self.middlewares
            .into_iter()
            .fold(handler, |next, mw| mw(next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_chain_empty() {
        let chain = MiddlewareChain::new();
        let handler: HandlerFunc = |_ctx| Box::pin(async { Ok(()) });
        let result = chain.apply(handler);
        // Just verify it doesn't panic
        assert!(std::mem::size_of_val(&result) > 0);
    }

    #[test]
    fn test_middleware_chain_with_middleware() {
        let chain = MiddlewareChain::new().add(|next| next);
        let handler: HandlerFunc = |_ctx| Box::pin(async { Ok(()) });
        let result = chain.apply(handler);
        assert!(std::mem::size_of_val(&result) > 0);
    }
}
