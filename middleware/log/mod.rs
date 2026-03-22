//! Log event types and middleware utilities.

pub mod redact;

use std::sync::Arc;
use tork::task::TaskLogPart;

/// Event type for log middleware events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Read event type.
    Read,
}

impl EventType {
    /// Convert the event type to a string slice.
    pub const fn as_str(&self) -> &'static str {
        match self {
            EventType::Read => "READ",
        }
    }
}

/// A handler function that processes log events.
pub type HandlerFunc =
    Arc<dyn Fn(Arc<Context>, EventType, &[TaskLogPart]) -> Result<(), LogError> + Send + Sync>;

/// Context for log operations.
#[derive(Debug, Clone)]
pub struct Context {
    #[allow(dead_code)]
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

/// A middleware function that wraps a log handler.
pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

/// Errors that can occur in log middleware.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("log middleware error: {0}")]
    Middleware(String),
    #[error("log handler error: {0}")]
    Handler(String),
}

/// Apply middleware to a log handler function.
pub fn apply_middleware(h: HandlerFunc, mws: Vec<MiddlewareFunc>) -> HandlerFunc {
    mws.into_iter().fold(h, |next, mw| mw(next))
}

/// Create a no-op handler that does nothing.
pub fn noop_handler() -> HandlerFunc {
    Arc::new(|_ctx: Arc<Context>, _et: EventType, _logs: &[TaskLogPart]| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_middleware_order() {
        let order = Arc::new(AtomicI32::new(1));
        let order_for_handler = order.clone();
        let order_for_mw1 = order.clone();
        let order_for_mw2 = order.clone();

        // Note: with apply_middleware, last element in vec is outermost (runs first)
        // vec![mw1, mw2] means mw2 runs first, then mw1, then handler
        let h: HandlerFunc = Arc::new(
            move |_ctx: Arc<Context>, _et: EventType, _logs: &[TaskLogPart]| {
                assert_eq!(order_for_handler.load(Ordering::SeqCst), 3);
                Ok(())
            },
        );

        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order_for_mw1.clone();
            Arc::new(
                move |ctx: Arc<Context>, et: EventType, logs: &[TaskLogPart]| {
                    assert_eq!(order.load(Ordering::SeqCst), 2);
                    order.fetch_add(1, Ordering::SeqCst);
                    next(ctx, et, logs)
                },
            )
        });

        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order_for_mw2.clone();
            Arc::new(
                move |ctx: Arc<Context>, et: EventType, logs: &[TaskLogPart]| {
                    assert_eq!(order.load(Ordering::SeqCst), 1);
                    order.fetch_add(1, Ordering::SeqCst);
                    next(ctx, et, logs)
                },
            )
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let result = hm(ctx, EventType::Read, &[]);
        assert!(result.is_ok());
    }
}
