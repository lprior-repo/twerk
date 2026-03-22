//! Pending handler for pending task events.

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use tork::task::{Task, TASK_STATE_PENDING, TASK_STATE_SKIPPED};

/// Pending handler for processing pending task events.
#[derive(Clone)]
pub struct PendingHandler {
    handler: TaskHandlerFunc,
}

impl std::fmt::Debug for PendingHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingHandler").finish()
    }
}

impl PendingHandler {
    /// Create a new pending handler.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
        }
    }

    /// Create a pending handler with a custom handler function.
    pub fn with_handler(handler: TaskHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a pending task event.
    ///
    /// If the task has a conditional that evaluates to false, it will be skipped.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
    ) -> Result<(), HandlerError> {
        // Check if task should be skipped due to conditional
        if let Some(ref r#if) = task.r#if {
            let cond = r#if.trim();
            if cond == "false" {
                task.state = TASK_STATE_SKIPPED.clone();
                return (self.handler)(ctx, TaskEventType::StateChange, task);
            }
        }
        task.state = TASK_STATE_PENDING.clone();
        (self.handler)(ctx, TaskEventType::StateChange, task)
    }
}

impl Default for PendingHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_pending_handler_normal() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_PENDING);
    }

    #[test]
    fn test_pending_handler_with_false_condition() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.r#if = Some("false".to_string());

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
    }

    #[test]
    fn test_pending_handler_with_whitespace_false_condition() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.r#if = Some("  false  ".to_string());

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
    }

    #[test]
    fn test_pending_handler_with_true_condition() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();
        task.r#if = Some("true".to_string());

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        // Should be pending, not skipped
        assert_eq!(task.state, *TASK_STATE_PENDING);
    }
}