//! Node middleware implementation.
//!
//! Provides a middleware pattern for processing tork nodes.

use std::sync::Arc;
use tork::node::Node;
use tork::node::NodeStatus;

/// A handler function that processes a node within a context.
pub type HandlerFunc = Arc<dyn Fn(Arc<Context>, &Node) -> Result<(), NodeError> + Send + Sync>;

/// A middleware function that wraps a handler.
pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

/// Context for node operations.
#[derive(Debug, Clone)]
pub struct Context {
    values: Vec<(String, String)>,
}

impl Context {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self { values: vec![] }
    }

    /// Get a value from the context.
    pub fn get(&self, _key: &str) -> Option<&str> {
        None
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in node middleware.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("middleware error: {0}")]
    Middleware(String),
    #[error("handler error: {0}")]
    Handler(String),
}

/// Apply middleware to a handler function.
///
/// This composes middleware in a chain where each middleware's `next` handler
/// points to the next middleware in the chain, ultimately reaching the final handler.
pub fn apply_middleware(h: HandlerFunc, mws: Vec<MiddlewareFunc>) -> HandlerFunc {
    mws.into_iter().fold(h, |next, mw| mw(next))
}

/// Create a no-op handler that does nothing.
pub fn noop_handler() -> HandlerFunc {
    Arc::new(|_ctx: Arc<Context>, _node: &Node| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_before() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let order = Arc::new(AtomicI32::new(1));
        let order_clone = order.clone();

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _node: &Node| {
            assert_eq!(order_clone.load(Ordering::SeqCst), 3);
            Ok(())
        });

        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, node: &Node| {
                assert_eq!(order.load(Ordering::SeqCst), 1);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx, node)
            })
        });

        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, node: &Node| {
                assert_eq!(order.load(Ordering::SeqCst), 2);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx, node)
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let node = Node {
            id: Some("test-id".to_string()),
            name: Some("test-name".to_string()),
            ..Default::default()
        };
        hm(ctx, &node).unwrap();
    }

    #[test]
    fn test_middleware_after() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _node: &Node| {
            assert_eq!(order.load(Ordering::SeqCst), 1);
            order.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, node: &Node| {
                let result = next(ctx.clone(), node);
                assert_eq!(order.load(Ordering::SeqCst), 3);
                order.fetch_add(1, Ordering::SeqCst);
                result
            })
        });

        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, node: &Node| {
                let result = next(ctx.clone(), node);
                assert_eq!(order.load(Ordering::SeqCst), 2);
                order.fetch_add(1, Ordering::SeqCst);
                result
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let node = Node {
            id: Some("test-id".to_string()),
            name: Some("test-name".to_string()),
            ..Default::default()
        };
        hm(ctx, &node).unwrap();
    }

    #[test]
    fn test_no_middleware() {
        let h: HandlerFunc = Arc::new(|_ctx: Arc<Context>, _node: &Node| Ok(()));

        let hm = apply_middleware(h, vec![]);
        let ctx = Arc::new(Context::new());
        let node = Node {
            id: Some("test-id".to_string()),
            name: Some("test-name".to_string()),
            ..Default::default()
        };
        hm(ctx, &node).unwrap();
    }

    #[test]
    fn test_middleware_error_short_circuits() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _node: &Node| {
            called_clone.store(true, Ordering::SeqCst);
            Ok(())
        });

        let err = NodeError::Middleware("something bad happened".to_string());
        let mw1: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<Context>, _node: &Node| Err(err.clone()))
        });

        let mw2: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<Context>, _node: &Node| {
                panic!("should not get here");
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let node = Node {
            id: Some("test-id".to_string()),
            name: Some("test-name".to_string()),
            ..Default::default()
        };

        let result = hm(ctx, &node);
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
    }
}
