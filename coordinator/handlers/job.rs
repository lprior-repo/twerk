//! Job handler for job state change events.

use crate::handlers::{
    noop_job_handler, HandlerContext, HandlerError, JobEventType, JobHandlerFunc,
};
use tork::job::{
    Job, JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_PENDING,
    JOB_STATE_RESTART, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED,
};

/// Job handler for processing job state change events.
#[derive(Clone)]
pub struct JobHandler {
    handler: JobHandlerFunc,
}

impl std::fmt::Debug for JobHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandler").finish()
    }
}

impl JobHandler {
    /// Create a new job handler.
    pub fn new() -> Self {
        Self {
            handler: noop_job_handler(),
        }
    }

    /// Create a job handler with a custom handler function.
    pub fn with_handler(handler: JobHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a job state change event.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        job: &mut Job,
    ) -> Result<(), HandlerError> {
        (self.handler)(ctx, JobEventType::StateChange, job)
    }

    /// Process a job state transition based on the current state.
    pub fn process_state_transition(
        &self,
        job: &Job,
    ) -> Result<JobStateTransition, HandlerError> {
        let transition = match &*job.state {
            s if *s == *JOB_STATE_PENDING => JobStateTransition::Start,
            s if *s == *JOB_STATE_SCHEDULED => JobStateTransition::Schedule,
            s if *s == *JOB_STATE_RUNNING => JobStateTransition::Run,
            s if *s == *JOB_STATE_CANCELLED => JobStateTransition::Cancel,
            s if *s == *JOB_STATE_COMPLETED => JobStateTransition::Complete,
            s if *s == *JOB_STATE_FAILED => JobStateTransition::Fail,
            s if *s == *JOB_STATE_RESTART => JobStateTransition::Restart,
            other => {
                return Err(HandlerError::InvalidState(format!(
                    "unknown job state: {}",
                    other
                )))
            }
        };
        Ok(transition)
    }
}

impl Default for JobHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStateTransition {
    /// Job is being started
    Start,
    /// Job is being scheduled
    Schedule,
    /// Job is running
    Run,
    /// Job is being cancelled
    Cancel,
    /// Job has completed
    Complete,
    /// Job has failed
    Fail,
    /// Job is being restarted
    Restart,
}

impl std::fmt::Display for JobStateTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStateTransition::Start => write!(f, "START"),
            JobStateTransition::Schedule => write!(f, "SCHEDULE"),
            JobStateTransition::Run => write!(f, "RUN"),
            JobStateTransition::Cancel => write!(f, "CANCEL"),
            JobStateTransition::Complete => write!(f, "COMPLETE"),
            JobStateTransition::Fail => write!(f, "FAIL"),
            JobStateTransition::Restart => write!(f, "RESTART"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_job_handler_default() {
        let handler = JobHandler::new();
        let ctx = Arc::new(());
        let mut job = Job::default();
        assert!(handler.handle(ctx, &mut job).is_ok());
    }

    #[test]
    fn test_process_state_transition_pending() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_PENDING.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Start);
    }

    #[test]
    fn test_process_state_transition_scheduled() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_SCHEDULED.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Schedule);
    }

    #[test]
    fn test_process_state_transition_running() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_RUNNING.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Run);
    }

    #[test]
    fn test_process_state_transition_completed() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_COMPLETED.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Complete);
    }

    #[test]
    fn test_process_state_transition_failed() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_FAILED.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Fail);
    }

    #[test]
    fn test_process_state_transition_restart() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = JOB_STATE_RESTART.to_string();
        let transition = handler.process_state_transition(&mut job).unwrap();
        assert_eq!(transition, JobStateTransition::Restart);
    }

    #[test]
    fn test_process_state_transition_unknown() {
        let handler = JobHandler::new();
        let mut job = Job::default();
        job.state = "UNKNOWN".to_string();
        let result = handler.process_state_transition(&mut job);
        assert!(result.is_err());
    }
}