//! Helper functions for record conversions.

use crate::datastore::Error as DatastoreError;
use twerk_core::task::TaskState;

/// Helper to convert a string slice to `TaskState` with a typed datastore error.
pub fn try_str_to_task_state(s: &str) -> Result<TaskState, DatastoreError> {
    s.parse()
        .map_err(|e| DatastoreError::Serialization(format!("task.state: {e}")))
}

/// Helper to convert a string slice to `TaskState`.
///
/// Returns `TaskState::Created` as a safe default for unknown values.
#[must_use]
pub fn str_to_task_state(s: &str) -> TaskState {
    try_str_to_task_state(s).unwrap_or(TaskState::Created)
}
