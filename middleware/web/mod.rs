//! Web middleware implementation.
//!
//! Provides a middleware pattern for HTTP request/response handling.

use std::sync::Arc;

/// A middleware function that wraps a handler.
pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

/// A handler function that processes HTTP requests.
pub type HandlerFunc = Arc<dyn Fn(Arc<dyn Context>) -> Result<(), WebError> + Send + Sync>;

/// Context for HTTP operations.
///
/// This trait provides access to HTTP request/response handling
/// capabilities needed by web middleware.
pub trait Context: Send + Sync {
    /// Get the request object.
    fn request(&self) -> Arc<dyn Request>;

    /// Get the response writer.
    fn response(&self) -> Arc<dyn Response>;

    /// Get a value from the context.
    fn get(&self, key: &str) -> Option<Arc<dyn std::any::Any + Send + Sync>>;

    /// Set a value in the context.
    fn set(&self, key: &str, val: Arc<dyn std::any::Any + Send + Sync>);

    /// Send a response with no body.
    fn no_content(&self, code: u16) -> Result<(), WebError>;

    /// Send a string response.
    fn string(&self, code: u16, s: &str) -> Result<(), WebError>;

    /// Send a JSON response.
    fn json(&self, code: u16, data: &dyn serde::Serialize) -> Result<(), WebError>;

    /// Send an error response.
    fn error(&self, code: u16, err: &dyn std::error::Error);

    /// Check if the request is done (cancelled/timed out).
    fn is_done(&self) -> bool;
}

/// Request trait for HTTP request data.
pub trait Request: Send + Sync {
    /// Get the request method.
    fn method(&self) -> &str;

    /// Get the request URI.
    fn uri(&self) -> &str;

    /// Get a header value.
    fn header(&self, name: &str) -> Option<&str>;

    /// Get the request body as bytes.
    fn body(&self) -> Result<Vec<u8>, WebError>;
}

/// Response trait for HTTP response handling.
pub trait Response: Send + Sync {
    /// Set a response header.
    fn set_header(&self, name: &str, value: &str);

    /// Write data to the response body.
    fn write(&self, data: &[u8]) -> Result<(), WebError>;

    /// Set the status code.
    fn set_status(&self, code: u16);
}

/// Errors that can occur in web middleware.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WebError {
    #[error("web middleware error: {0}")]
    Middleware(String),
    #[error("web handler error: {0}")]
    Handler(String),
    #[error("context error: {0}")]
    Context(String),
    #[error("request error: {0}")]
    Request(String),
    #[error("response error: {0}")]
    Response(String),
    #[error("bind error: {0}")]
    Bind(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Apply middleware to a web handler function.
pub fn apply_middleware(h: HandlerFunc, mws: Vec<MiddlewareFunc>) -> HandlerFunc {
    mws.into_iter().fold(h, |next, mw| mw(next))
}

/// Create a no-op handler that does nothing.
pub fn noop_handler() -> HandlerFunc {
    Arc::new(|_ctx: Arc<dyn Context>| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_middleware_order() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let order = Arc::new(AtomicI32::new(1));
        let order_clone = order.clone();

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<dyn Context>| {
            assert_eq!(order_clone.load(Ordering::SeqCst), 3);
            Ok(())
        });

        let order_for_mw1 = order.clone();
        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order_for_mw1;
            Arc::new(move |ctx: Arc<dyn Context>| {
                assert_eq!(order.load(Ordering::SeqCst), 1);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx)
            })
        });

        let order_for_mw2 = order.clone();
        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order_for_mw2;
            Arc::new(move |ctx: Arc<dyn Context>| {
                assert_eq!(order.load(Ordering::SeqCst), 2);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx)
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx: Arc<dyn Context> = Arc::new(MockContext::new());
        hm(ctx).unwrap();
    }

    #[test]
    fn test_no_middleware() {
        let h: HandlerFunc = Arc::new(|_ctx: Arc<dyn Context>| Ok(()));

        let hm = apply_middleware(h, vec![]);
        let ctx: Arc<dyn Context> = Arc::new(MockContext::new());
        hm(ctx).unwrap();
    }

    #[test]
    fn test_middleware_error_short_circuits() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<dyn Context>| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(())
        });

        let err = WebError::Middleware("something bad happened".to_string());
        let mw1: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<dyn Context>| Err(err.clone()))
        });

        let mw2: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<dyn Context>| {
                panic!("should not get here");
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx: Arc<dyn Context> = Arc::new(MockContext::new());

        let result = hm(ctx);
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
    }

    /// Mock context for testing.
    struct MockContext;

    impl MockContext {
        fn new() -> Self {
            Self
        }
    }

    impl Context for MockContext {
        fn request(&self) -> Arc<dyn Request> {
            Arc::new(MockRequest::new())
        }

        fn response(&self) -> Arc<dyn Response> {
            Arc::new(MockResponse::new())
        }

        fn get(&self, _key: &str) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
            None
        }

        fn set(&self, _key: &str, _val: Arc<dyn std::any::Any + Send + Sync>) {}

        fn no_content(&self, _code: u16) -> Result<(), WebError> {
            Ok(())
        }

        fn string(&self, _code: u16, _s: &str) -> Result<(), WebError> {
            Ok(())
        }

        fn json(&self, _code: u16, _data: &dyn serde::Serialize) -> Result<(), WebError> {
            Ok(())
        }

        fn bind<T: serde::de::DeserializeOwned>(&self) -> Result<T, WebError> {
            Err(WebError::Bind("not implemented".to_string()))
        }

        fn error(&self, _code: u16, _err: &dyn std::error::Error) {}

        fn is_done(&self) -> bool {
            false
        }
    }

    /// Mock request for testing.
    struct MockRequest;

    impl MockRequest {
        fn new() -> Self {
            Self
        }
    }

    impl Request for MockRequest {
        fn method(&self) -> &str {
            "GET"
        }

        fn uri(&self) -> &str {
            "/test"
        }

        fn header(&self, _name: &str) -> Option<&str> {
            None
        }

        fn body(&self) -> Result<Vec<u8>, WebError> {
            Ok(vec![])
        }
    }

    /// Mock response for testing.
    struct MockResponse;

    impl MockResponse {
        fn new() -> Self {
            Self
        }
    }

    impl Response for MockResponse {
        fn set_header(&self, _name: &str, _value: &str) {}

        fn write(&self, _data: &[u8]) -> Result<(), WebError> {
            Ok(())
        }

        fn set_status(&self, _code: u16) {}
    }
}
