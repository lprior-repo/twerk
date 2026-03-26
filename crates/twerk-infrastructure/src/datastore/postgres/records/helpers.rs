//! Helper functions for record conversions.

use twerk_core::task::TaskState;

/// Helper to convert a string slice to `TaskState`
#[must_use]
pub fn str_to_task_state(s: &str) -> TaskState {
    TaskState::from(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_to_task_state_converts_all_states() {
        let states = [
            "CREATED",
            "PENDING",
            "SCHEDULED",
            "RUNNING",
            "CANCELLED",
            "STOPPED",
            "COMPLETED",
            "FAILED",
            "SKIPPED",
        ];
        for state in &states {
            let converted = str_to_task_state(state);
            assert_eq!(converted.as_str(), *state);
        }
    }
}
