//! Redelivered task handler.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
    MAX_REDELIVERIES,
};
use tork::task::{Task, TASK_STATE_FAILED};

/// Redelivered task handler for handling task redelivery.
pub struct RedeliveredHandler {
    handler: TaskHandlerFunc,
}

impl RedeliveredHandler {
    /// Create a new redelivered handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create a redelivered handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a redelivered task.
    ///
    /// If the task has been redelivered too many times, it will be marked as failed.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        if task.redelivered >= MAX_REDELIVERIES {
            task.state = TASK_STATE_FAILED.clone();
            task.error = Some("task redelivered too many times".to_string());
        }
        (self.handler)(ctx, TaskEventType::Redelivered, task)
    }
}

impl Default for RedeliveredHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RedeliveredHandler {
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
    use tork::task::TASK_STATE_RUNNING;

    #[test]
    fn test_redelivered_handler_under_limit() {
        let handler = RedeliveredHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_RUNNING.clone();
        task.redelivered = 3;

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Task should still be in running state
        assert_eq!(task.state, TASK_STATE_RUNNING.clone());
    }

    #[test]
    fn test_redelivered_handler_at_limit() {
        let handler = RedeliveredHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_RUNNING.clone();
        task.redelivered = MAX_REDELIVERIES;

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Task should be marked as failed
        assert_eq!(task.state, TASK_STATE_FAILED.clone());
        assert_eq!(task.error.as_deref(), Some("task redelivered too many times"));
    }
}