//! Cancel handler for job cancellation.

use crate::handlers::{
    noop_job_handler, HandlerContext, HandlerError, JobEventType, JobHandlerFunc,
};
use tork::job::{Job, JOB_STATE_CANCELLED, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};

/// Cancel handler for processing job cancellations.
pub struct CancelHandler {
    handler: JobHandlerFunc,
}

impl CancelHandler {
    /// Create a new cancel handler.
    pub fn new() -> Self {
        Self {
            handler: noop_job_handler(),
        }
    }

    /// Create a cancel handler with a custom handler function.
    pub fn with_handler(handler: JobHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a job cancellation.
    ///
    /// Only running or scheduled jobs can be cancelled.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        job: &mut Job,
    ) -> Result<(), HandlerError> {
        // Only cancel if job is running or scheduled
        if job.state != *JOB_STATE_RUNNING && job.state != *JOB_STATE_SCHEDULED {
            return Err(HandlerError::InvalidState(format!(
                "job is {} and cannot be cancelled",
                job.state
            )));
        }
        job.state = JOB_STATE_CANCELLED.to_string();
        (self.handler)(ctx, JobEventType::StateChange, job)
    }
}

impl Default for CancelHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CancelHandler {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_cancel_handler_running_job() {
        let handler = CancelHandler::new();
        let ctx = Arc::new(());
        let mut job = Job::default();
        job.state = JOB_STATE_RUNNING.to_string();

        let result = handler.handle(ctx, &mut job);
        assert!(result.is_ok());
        assert_eq!(job.state, *JOB_STATE_CANCELLED);
    }

    #[test]
    fn test_cancel_handler_scheduled_job() {
        let handler = CancelHandler::new();
        let ctx = Arc::new(());
        let mut job = Job::default();
        job.state = JOB_STATE_SCHEDULED.to_string();

        let result = handler.handle(ctx, &mut job);
        assert!(result.is_ok());
        assert_eq!(job.state, *JOB_STATE_CANCELLED);
    }

    #[test]
    fn test_cancel_handler_completed_job() {
        let handler = CancelHandler::new();
        let ctx = Arc::new(());
        let mut job = Job::default();
        job.state = tork::job::JOB_STATE_COMPLETED.to_string();

        let result = handler.handle(ctx, &mut job);
        assert!(result.is_err());
        // Job state should not have changed
        assert_eq!(job.state, tork::job::JOB_STATE_COMPLETED.to_string());
    }
}