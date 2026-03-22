//! Job redaction middleware.
//!
//! Redacts sensitive information from jobs when they are read.

use crate::middleware::job::{
    apply_middleware, noop_handler, Context, EventType, HandlerFunc, JobError, MiddlewareFunc,
};
use std::sync::Arc;
use tork::job::Job;

/// Redacter trait for redacting sensitive information.
pub trait Redacter: Send + Sync {
    /// Redact a job.
    fn redact_job(&self, job: &mut Job);
}

/// Create a redaction middleware.
///
/// This middleware redacts sensitive information from jobs when they are read.
pub fn redact_middleware<R: Redacter + 'static>(redacter: Arc<R>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| {
        let redacter = redacter.clone();
        Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
            if et == EventType::READ {
                redacter.redact_job(job);
            }
            next(ctx, et, job)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRedacter;

    impl Redacter for MockRedacter {
        fn redact_job(&self, job: &mut Job) {
            if let Some(inputs) = &mut job.inputs {
                for (_, v) in inputs.iter_mut() {
                    if v.contains("1234") {
                        *v = "[REDACTED]".to_string();
                    }
                }
            }
        }
    }

    #[test]
    fn test_redact_on_read() {
        let redacter = Arc::new(MockRedacter);
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(std::collections::HashMap::from([(
                "secret".to_string(),
                "1234".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::READ, &mut job).unwrap();

        assert_eq!(
            job.inputs.as_ref().unwrap().get("secret"),
            Some(&"[REDACTED]".to_string())
        );
    }

    #[test]
    fn test_no_redact_on_state_change() {
        let redacter = Arc::new(MockRedacter);
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(std::collections::HashMap::from([(
                "secret".to_string(),
                "1234".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::STATE_CHANGE, &mut job).unwrap();

        // StateChange should not trigger redaction
        assert_eq!(
            job.inputs.as_ref().unwrap().get("secret"),
            Some(&"1234".to_string())
        );
    }
}
