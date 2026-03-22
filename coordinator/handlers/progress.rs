//! Progress handler for task progress updates.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use tork::task::Task;

/// Progress handler for processing task progress updates.
pub struct ProgressHandler {
    handler: TaskHandlerFunc,
}

impl ProgressHandler {
    /// Create a new progress handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create a progress handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a progress update for a task.
    ///
    /// Clamps progress to the range [0, 100].
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        // Clamp progress to valid range
        if task.progress < 0.0 {
            task.progress = 0.0;
        } else if task.progress > 100.0 {
            task.progress = 100.0;
        }
        (self.handler)(ctx, TaskEventType::Progress, task)
    }
}

impl Default for ProgressHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProgressHandler {
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
    fn test_progress_handler_normal() {
        let handler = ProgressHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.progress = 50.0;

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.progress, 50.0);
    }

    #[test]
    fn test_progress_handler_negative() {
        let handler = ProgressHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.progress = -10.0;

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Progress should be clamped to 0
        assert_eq!(task.progress, 0.0);
    }

    #[test]
    fn test_progress_handler_over_100() {
        let handler = ProgressHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.progress = 150.0;

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Progress should be clamped to 100
        assert_eq!(task.progress, 100.0);
    }
}