//! Task middleware module.
//!
//! This module provides middleware functionality for task event handling.

mod hostenv;
mod redact;
mod task_error;
mod webhook;

pub use hostenv::HostEnv;
pub use redact::{redact_middleware, DefaultRedacter, Redacter as TaskRedacter};
pub use task_error::TaskMiddlewareError;
pub use webhook::{webhook_middleware, Datastore as WebhookDatastore};

pub use self::task_handler::{apply_middleware, noop_handler};

mod task_types {
    // Re-export types from tork crate

    pub use tork::task::Task;

    // Re-export task state constants

    // Re-export job state constants

    /// Event types for task lifecycle events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EventType {
        Started,
        StateChange,
        Redelivered,
        Progress,
        Read,
    }

    impl EventType {
        pub const fn as_str(&self) -> &'static str {
            match self {
                EventType::Started => "STARTED",
                EventType::StateChange => "STATE_CHANGE",
                EventType::Redelivered => "REDELIVERED",
                EventType::Progress => "PROGRESS",
                EventType::Read => "READ",
            }
        }
    }

    impl From<&str> for EventType {
        fn from(s: &str) -> Self {
            match s {
                "STARTED" => EventType::Started,
                "STATE_CHANGE" => EventType::StateChange,
                "REDELIVERED" => EventType::Redelivered,
                "PROGRESS" => EventType::Progress,
                "READ" => EventType::Read,
                _ => EventType::Read,
            }
        }
    }

    impl std::fmt::Display for EventType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_str())
        }
    }
}

mod task_handler {
    use super::task_types::*;
    use std::sync::Arc;

    /// Context type for task operations.
    pub type Context = Arc<std::sync::RwLock<()>>;

    /// Handler function type for task events.
    pub type HandlerFunc = Arc<
        dyn Fn(Context, EventType, &mut Task) -> Result<(), super::task_error::TaskMiddlewareError>
            + Send
            + Sync,
    >;

    /// No-op handler that does nothing.
    pub fn noop_handler() -> HandlerFunc {
        Arc::new(|_ctx: Context, _et: EventType, _task: &mut Task| Ok(()))
    }

    /// Middleware function type that wraps a handler.
    pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

    /// Applies a chain of middleware to a handler.
    pub fn apply_middleware(h: HandlerFunc, mws: &[MiddlewareFunc]) -> HandlerFunc {
        mws.iter().fold(h, |next, mw| mw(next))
    }
}

#[cfg(test)]
mod tests {
    use super::task_error::TaskMiddlewareError;
    use super::task_handler::*;
    use super::task_types::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Arc;

    fn make_ctx() -> Context {
        Arc::new(std::sync::RwLock::new(()))
    }

    // Go parity: TestMiddlewareBefore — middlewares run before the handler.
    // With apply_middleware's left-fold, vec![mw2, mw1] produces:
    //   mw1 → mw2 → handler
    fn make_task() -> tork::task::Task {
        tork::task::Task {
            id: Some("1".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_middleware_before() {
        // Go parity: TestMiddlewareBefore
        // Two middlewares run in order before the handler reaches execution.
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = {
            let order = order.clone();
            Arc::new(
                move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    assert_eq!(order.load(Ordering::SeqCst), 3);
                    Ok(())
                },
            )
        };

        // With fold-left [mw2, mw1]: mw1 wraps h, mw2 wraps that → mw2 runs first
        // To get Go order (mw1→mw2→handler), reverse: [mw2, mw1]
        let mw1: MiddlewareFunc = {
            let order = order.clone();
            Arc::new(move |next: HandlerFunc| {
                let order = order.clone();
                Arc::new(
                    move |ctx: Context, et: EventType, task: &mut tork::task::Task| {
                        assert_eq!(order.load(Ordering::SeqCst), 1);
                        order.fetch_add(1, Ordering::SeqCst);
                        next(ctx, et, task)
                    },
                )
            })
        };

        let mw2: MiddlewareFunc = {
            let order = order.clone();
            Arc::new(move |next: HandlerFunc| {
                let order = order.clone();
                Arc::new(
                    move |ctx: Context, et: EventType, task: &mut tork::task::Task| {
                        assert_eq!(order.load(Ordering::SeqCst), 2);
                        order.fetch_add(1, Ordering::SeqCst);
                        next(ctx, et, task)
                    },
                )
            })
        };

        // Reverse order to get Go semantics: mw1 runs first, mw2 second, handler last
        let hm = apply_middleware(h, &[mw2, mw1]);
        let mut task = make_task();
        let result = hm(make_ctx(), EventType::StateChange, &mut task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_middleware_after() {
        // Go parity: TestMiddlewareAfter
        // Handler runs first, then middlewares execute in reverse (inner→outer).
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = {
            let order = order.clone();
            Arc::new(
                move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    assert_eq!(order.load(Ordering::SeqCst), 1);
                    order.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        };

        // With fold-left [mw1, mw2]: mw2 is outermost (runs after handler last)
        // Call order: mw2→mw1→h→mw1_after(2)→mw2_after(3)
        let mw1: MiddlewareFunc = {
            let order = order.clone();
            Arc::new(move |next: HandlerFunc| {
                let order = order.clone();
                Arc::new(
                    move |ctx: Context, et: EventType, task: &mut tork::task::Task| {
                        let result = next(ctx, et, task);
                        assert_eq!(order.load(Ordering::SeqCst), 2);
                        order.fetch_add(1, Ordering::SeqCst);
                        result
                    },
                )
            })
        };

        let mw2: MiddlewareFunc = {
            let order = order.clone();
            Arc::new(move |next: HandlerFunc| {
                let order = order.clone();
                Arc::new(
                    move |ctx: Context, et: EventType, task: &mut tork::task::Task| {
                        let result = next(ctx, et, task);
                        assert_eq!(order.load(Ordering::SeqCst), 3);
                        order.fetch_add(1, Ordering::SeqCst);
                        result
                    },
                )
            })
        };

        let hm = apply_middleware(h, &[mw1, mw2]);
        let mut task = make_task();
        let result = hm(make_ctx(), EventType::StateChange, &mut task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_middleware() {
        // Go parity: TestNoMiddleware
        // With empty middleware slice, handler runs directly.
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = {
            let order = order.clone();
            Arc::new(
                move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    assert_eq!(order.load(Ordering::SeqCst), 1);
                    order.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        };

        let hm = apply_middleware(h, &[]);
        let mut task = make_task();
        let result = hm(make_ctx(), EventType::StateChange, &mut task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_middleware_error() {
        // Go parity: TestMiddlewareError
        // When a middleware returns an error, subsequent middleware and the handler
        // must not be invoked.
        let called_handler = Arc::new(AtomicI32::new(0));
        let called_mw2 = Arc::new(AtomicI32::new(0));

        let h: HandlerFunc = {
            let called = called_handler.clone();
            Arc::new(
                move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    called.store(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        };

        // mw1 is outermost (runs first) and returns an error
        let mw1: MiddlewareFunc = Arc::new(|_next: HandlerFunc| {
            Arc::new(
                |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    Err(TaskMiddlewareError::Middleware(
                        "something bad happened".to_string(),
                    ))
                },
            )
        });

        // mw2 is inner — should never be reached
        let mw2: MiddlewareFunc = {
            let called = called_mw2.clone();
            Arc::new(move |_next: HandlerFunc| {
                let called = called.clone();
                Arc::new(
                    move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                        called.store(1, Ordering::SeqCst);
                        Ok(())
                    },
                )
            })
        };

        // With fold-left [mw2, mw1]: mw1 wraps h, mw2 wraps mw1_wrapped
        // Execution: mw2 → mw1 (error) → stops
        // To get Go order where mw1 is outermost: [mw2, mw1]
        let hm = apply_middleware(h, &[mw2, mw1]);
        let mut task = make_task();
        let result = hm(make_ctx(), EventType::StateChange, &mut task);

        assert!(result.is_err());
        assert_eq!(
            called_handler.load(Ordering::SeqCst),
            0,
            "handler should not have been called"
        );
        assert_eq!(
            called_mw2.load(Ordering::SeqCst),
            0,
            "mw2 should not have been reached"
        );
    }

    #[test]
    fn test_middleware_error_stops_chain() {
        // Verify error propagation: when inner middleware errors,
        // outer middleware still sees the error.
        let order = Arc::new(AtomicI32::new(0));

        let h: HandlerFunc = {
            let order = order.clone();
            Arc::new(
                move |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    order.store(99, Ordering::SeqCst);
                    Ok(())
                },
            )
        };

        // mw1 is inner, returns error
        let mw1: MiddlewareFunc = Arc::new(|_next: HandlerFunc| {
            Arc::new(
                |_ctx: Context, _et: EventType, _task: &mut tork::task::Task| {
                    Err(TaskMiddlewareError::Middleware("inner error".to_string()))
                },
            )
        });

        // mw2 is outer, calls next and checks for error
        let mw2: MiddlewareFunc = {
            let order = order.clone();
            Arc::new(move |next: HandlerFunc| {
                let order = order.clone();
                Arc::new(
                    move |ctx: Context, et: EventType, task: &mut tork::task::Task| {
                        order.store(1, Ordering::SeqCst);
                        let result = next(ctx, et, task);
                        assert!(result.is_err(), "outer should see inner error");
                        order.store(2, Ordering::SeqCst);
                        result
                    },
                )
            })
        };

        // With fold-left [mw1, mw2]: mw2 is outer, mw1 is inner
        // Execution: mw2 → mw1 (errors) → mw2 sees error, propagates
        let hm = apply_middleware(h, &[mw1, mw2]);
        let mut task = make_task();
        let result = hm(make_ctx(), EventType::StateChange, &mut task);

        assert!(result.is_err());
        assert_eq!(order.load(Ordering::SeqCst), 2, "mw2 should have completed");
        // handler (99) should never have been reached since mw1 errored
        assert_ne!(order.load(Ordering::SeqCst), 99);
    }
}
