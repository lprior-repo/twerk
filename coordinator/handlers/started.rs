//! Started handler for task start events.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use tork::task::{Task, TASK_STATE_RUNNING, TASK_STATE_SCHEDULED};

/// Started handler for processing task start events.
#[derive(Clone)]
pub struct StartedHandler {
    handler: TaskHandlerFunc,
}

impl std::fmt::Debug for StartedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartedHandler").finish()
    }
}

impl StartedHandler {
    /// Create a new started handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create a started handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a task start event.
    ///
    /// Updates the task state to running if it was previously scheduled.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        // If task was scheduled, mark it as running
        if task.state == *TASK_STATE_SCHEDULED {
            task.state = TASK_STATE_RUNNING.clone();
        }
        (self.handler)(ctx, TaskEventType::Started, task)
    }
}

impl Default for StartedHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_started_handler_scheduled_task() {
        let handler = StartedHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_SCHEDULED.clone();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }

    #[test]
    fn test_started_handler_already_running() {
        let handler = StartedHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_RUNNING.clone();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // State should remain running
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }
}