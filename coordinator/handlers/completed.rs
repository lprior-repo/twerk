//! Completed handler for task completion events.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use tork::task::{Task, TASK_STATE_COMPLETED, TASK_STATE_SKIPPED};

/// Completed handler for processing task completion events.
#[derive(Clone)]
pub struct CompletedHandler {
    handler: TaskHandlerFunc,
}

impl std::fmt::Debug for CompletedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedHandler").finish()
    }
}

impl CompletedHandler {
    /// Create a new completed handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create a completed handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a task completion event.
    ///
    /// Marks the task as completed (or skipped if already in that state).
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        match &*task.state {
            s if *s == *TASK_STATE_COMPLETED || *s == *TASK_STATE_SKIPPED => {
                (self.handler)(ctx, TaskEventType::StateChange, task)
            }
            _ => {
                task.state = TASK_STATE_COMPLETED.clone();
                (self.handler)(ctx, TaskEventType::StateChange, task)
            }
        }
    }
}

impl Default for CompletedHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_completed_handler_normal() {
        let handler = CompletedHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_COMPLETED);
    }

    #[test]
    fn test_completed_handler_already_completed() {
        let handler = CompletedHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_COMPLETED.clone();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_COMPLETED);
    }

    #[test]
    fn test_completed_handler_skipped() {
        let handler = CompletedHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.state = TASK_STATE_SKIPPED.clone();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Skipped state should be preserved
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
    }
}