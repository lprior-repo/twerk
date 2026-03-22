//! Schedule handler for scheduled job events.

use crate::handlers::{
    noop_scheduled_job_handler, HandlerContext, HandlerError, ScheduledJobHandlerFunc,
};
use tork::job::{ScheduledJob, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED};

/// Schedule handler for processing scheduled job events.
#[derive(Clone)]
pub struct ScheduleHandler {
    handler: ScheduledJobHandlerFunc,
}

impl std::fmt::Debug for ScheduleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduleHandler").finish()
    }
}

impl ScheduleHandler {
    /// Create a new schedule handler.
    pub fn new() -> Self {
        Self {
            handler: noop_scheduled_job_handler(),
        }
    }

    /// Create a schedule handler with a custom handler function.
    pub fn with_handler(handler: ScheduledJobHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a scheduled job event.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        job: &mut ScheduledJob,
    ) -> Result<(), HandlerError> {
        (self.handler)(ctx, job)
    }

    /// Process a scheduled job state transition.
    pub fn process_state_transition(
        &self,
        job: &ScheduledJob,
    ) -> Result<ScheduledJobStateTransition, HandlerError> {
        let transition = match &*job.state {
            s if *s == *SCHEDULED_JOB_STATE_ACTIVE => {
                ScheduledJobStateTransition::Activate
            }
            s if *s == *SCHEDULED_JOB_STATE_PAUSED => {
                ScheduledJobStateTransition::Pause
            }
            other => {
                return Err(HandlerError::InvalidState(format!(
                    "unknown scheduled job state: {}",
                    other
                )));
            }
        };
        Ok(transition)
    }
}

impl Default for ScheduleHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a scheduled job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledJobStateTransition {
    /// Scheduled job is being activated
    Activate,
    /// Scheduled job is being paused
    Pause,
}

impl std::fmt::Display for ScheduledJobStateTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduledJobStateTransition::Activate => write!(f, "ACTIVATE"),
            ScheduledJobStateTransition::Pause => write!(f, "PAUSE"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_schedule_handler_default() {
        let handler = ScheduleHandler::new();
        let ctx = Arc::new(());
        let mut job = ScheduledJob::default();
        assert!(handler.handle(ctx, &mut job).is_ok());
    }

    #[test]
    fn test_process_state_transition_active() {
        let handler = ScheduleHandler::new();
        let mut job = ScheduledJob::default();
        job.state = SCHEDULED_JOB_STATE_ACTIVE.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, ScheduledJobStateTransition::Activate);
    }

    #[test]
    fn test_process_state_transition_paused() {
        let handler = ScheduleHandler::new();
        let mut job = ScheduledJob::default();
        job.state = SCHEDULED_JOB_STATE_PAUSED.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, ScheduledJobStateTransition::Pause);
    }

    #[test]
    fn test_process_state_transition_unknown() {
        let handler = ScheduleHandler::new();
        let mut job = ScheduledJob::default();
        job.state = "UNKNOWN".to_string();
        let result = handler.process_state_transition(&job);
        assert!(result.is_err());
    }
}