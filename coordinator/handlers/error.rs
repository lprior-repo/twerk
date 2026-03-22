//! Error handler for task error events.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use tork::task::{Task, TASK_STATE_FAILED};

/// Error handler for processing task error events.
#[derive(Clone)]
pub struct ErrorHandler {
    handler: TaskHandlerFunc,
}

impl std::fmt::Debug for ErrorHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorHandler").finish()
    }
}

impl ErrorHandler {
    /// Create a new error handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create an error handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a task error event.
    ///
    /// Marks the task as failed with the error message.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        task.state = TASK_STATE_FAILED.clone();
        (self.handler)(ctx, TaskEventType::StateChange, task)
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_error_handler() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.error = Some("something went wrong".to_string());

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_FAILED);
        assert_eq!(task.error.as_deref(), Some("something went wrong"));
    }
}