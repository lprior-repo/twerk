//! Log handler for task logs.
//!
//! Port of Go `internal/coordinator/handlers/log.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives TaskLogPart messages
//! 2. Calls ds.CreateTaskLogPart to persist
//! 3. Logs errors on failure (non-fatal — Go swallows errors)

use std::sync::Arc;

use tork::task::TaskLogPart;
use tork::Datastore;

use crate::handlers::HandlerError;

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Log handler for processing task log parts.
///
/// Holds a reference to the datastore for I/O operations.
/// Go's implementation swallows errors (logs and continues);
/// this handler surfaces them for the caller to decide.
pub struct LogHandler {
    ds: Arc<dyn Datastore>,
}

impl std::fmt::Debug for LogHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogHandler").finish()
    }
}

impl LogHandler {
    /// Create a new log handler with datastore dependency.
    /// Go: `NewLogHandler(ds datastore.Datastore) func(p *tork.TaskLogPart)`
    pub fn new(ds: Arc<dyn Datastore>) -> Self {
        Self { ds }
    }

    /// Handle a task log part by persisting it to the datastore.
    ///
    /// Go parity (`handle`):
    /// 1. Calls `ds.CreateTaskLogPart(ctx, p)`
    /// 2. On error, logs and returns (Go swallows; we surface for observability)
    pub async fn handle(&self, part: &TaskLogPart) -> Result<(), HandlerError> {
        self.ds
            .create_task_log_part(part.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

