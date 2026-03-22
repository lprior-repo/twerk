//! Job middleware implementation.
//!
//! Provides a middleware pattern for processing tork jobs.

use std::fmt;
use std::sync::Arc;
use tork::job::new_job_summary;
use tork::job::Job;
use tork::job::JobSummary;
use tork::task::Webhook;

/// Event type for job middleware events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventType;

impl EventType {
    /// State change event - occurs when a job's state changes.
    pub const STATE_CHANGE: &'static str = "STATE_CHANGE";
    /// Progress event - occurs when a job's progress changes.
    pub const PROGRESS: &'static str = "PROGRESS";
    /// Read event - occurs when a Job is read by the client through the API.
    pub const READ: &'static str = "READ";
}

/// A handler function that processes job events.
pub type HandlerFunc =
    Arc<dyn Fn(Arc<Context>, EventType, &mut Job) -> Result<(), JobError> + Send + Sync>;

/// A middleware function that wraps a job handler.
pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

/// Context for job operations.
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
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Insert a value into the context.
    #[must_use]
    pub fn with_value(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.push((key.into(), value.into()));
        self
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in job middleware.
#[derive(Debug, Clone, thiserror::Error)]
pub enum JobError {
    #[error("job middleware error: {0}")]
    Middleware(String),
    #[error("job handler error: {0}")]
    Handler(String),
}

/// Create a no-op handler that does nothing.
pub fn noop_handler() -> HandlerFunc {
    Arc::new(|_ctx: Arc<Context>, _et: EventType, _job: &mut Job| Ok(()))
}

/// Apply middleware to a job handler function.
pub fn apply_middleware(h: HandlerFunc, mws: Vec<MiddlewareFunc>) -> HandlerFunc {
    mws.into_iter().fold(h, |next, mw| mw(next))
}

fn next_handler(
    ctx: Arc<Context>,
    index: usize,
    mws: &[MiddlewareFunc],
    h: HandlerFunc,
) -> HandlerFunc {
    if index >= mws.len() {
        return h;
    }
    let nx = next_handler(ctx, index + 1, mws, h);
    mws[index](nx)
}

/// Apply middleware chain - calls next handler in chain.
pub fn apply_middleware_chain(
    h: HandlerFunc,
    mws: &[MiddlewareFunc],
) -> impl Fn(Arc<Context>, EventType, &mut Job) -> Result<(), JobError> {
    let wrapped = if mws.is_empty() {
        h
    } else {
        next_handler(Arc::new(Context::new()), 0, mws, h)
    };

    move |ctx: Arc<Context>, et: EventType, job: &mut Job| wrapped(ctx, et, job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    fn make_test_job() -> Job {
        Job {
            id: Some("test-id".to_string()),
            state: tork::job::JOB_STATE_PENDING.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_middleware_before() {
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| {
            assert_eq!(order.load(Ordering::SeqCst), 3);
            Ok(())
        });

        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
                assert_eq!(order.load(Ordering::SeqCst), 1);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx, et, job)
            })
        });

        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
                assert_eq!(order.load(Ordering::SeqCst), 2);
                order.fetch_add(1, Ordering::SeqCst);
                next(ctx, et, job)
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let mut job = make_test_job();
        hm(ctx, EventType::STATE_CHANGE, &mut job).unwrap();
    }

    #[test]
    fn test_middleware_after() {
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| {
            assert_eq!(order.load(Ordering::SeqCst), 1);
            order.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let mw1: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
                let result = next(ctx.clone(), et, job);
                assert_eq!(order.load(Ordering::SeqCst), 3);
                order.fetch_add(1, Ordering::SeqCst);
                result
            })
        });

        let mw2: MiddlewareFunc = Arc::new(move |next: HandlerFunc| {
            let order = order.clone();
            Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
                let result = next(ctx.clone(), et, job);
                assert_eq!(order.load(Ordering::SeqCst), 2);
                order.fetch_add(1, Ordering::SeqCst);
                result
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let mut job = make_test_job();
        hm(ctx, EventType::STATE_CHANGE, &mut job).unwrap();
    }

    #[test]
    fn test_no_middleware() {
        let order = Arc::new(AtomicI32::new(1));

        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| {
            assert_eq!(order.load(Ordering::SeqCst), 1);
            order.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let hm = apply_middleware(h, vec![]);
        let ctx = Arc::new(Context::new());
        let mut job = make_test_job();
        hm(ctx, EventType::STATE_CHANGE, &mut job).unwrap();
    }

    #[test]
    fn test_middleware_error() {
        let h: HandlerFunc = Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| {
            panic!("should not get here");
        });

        let err = JobError::Middleware("something bad happened".to_string());
        let mw1: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| Err(err.clone()))
        });

        let mw2: MiddlewareFunc = Arc::new(move |_next: HandlerFunc| {
            Arc::new(move |_ctx: Arc<Context>, _et: EventType, _job: &mut Job| {
                panic!("should not get here");
            })
        });

        let hm = apply_middleware(h, vec![mw1, mw2]);
        let ctx = Arc::new(Context::new());
        let mut job = make_test_job();

        let result = hm(ctx, EventType::STATE_CHANGE, &mut job);
        assert!(result.is_err());
    }
}
