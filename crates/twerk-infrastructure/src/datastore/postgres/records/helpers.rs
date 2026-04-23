//! Helper functions for record conversions.

use twerk_core::task::TaskState;

/// Helper to convert a string slice to `TaskState`
#[must_use]
pub fn str_to_task_state(s: &str) -> TaskState {
    s.parse().unwrap_or_default()
}
