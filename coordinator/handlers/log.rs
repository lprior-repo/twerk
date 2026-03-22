//! Log handler for task logs.

use crate::handlers::{noop_log_handler, HandlerContext, HandlerError, LogHandlerFunc};
use tork::task::TaskLogPart;

/// Log handler for processing task log parts.
pub struct LogHandler {
    handler: LogHandlerFunc,
}

impl LogHandler {
    /// Create a new log handler.
    pub fn new() -> Self {
        Self {
            handler: noop_log_handler(),
        }
    }

    /// Create a log handler with a custom handler function.
    pub fn with_handler(handler: LogHandlerFunc) -> Self {
        Self { handler }
    }

    /// Handle a batch of log parts.
    pub fn handle(
        &self,
        ctx: HandlerContext,
        logs: &[TaskLogPart],
    ) -> Result<(), HandlerError> {
        (self.handler)(ctx, logs)
    }
}

impl Default for LogHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LogHandler {
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
    fn test_log_handler_default() {
        let handler = LogHandler::new();
        let ctx = Arc::new(());
        let logs: Vec<TaskLogPart> = vec![];
        assert!(handler.handle(ctx, &logs).is_ok());
    }

    #[test]
    fn test_log_handler_with_logs() {
        let handler = LogHandler::new();
        let ctx = Arc::new(());
        let logs = vec![TaskLogPart {
            id: Some("log-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("test log content".to_string()),
            created_at: None,
        }];
        assert!(handler.handle(ctx, &logs).is_ok());
    }
}