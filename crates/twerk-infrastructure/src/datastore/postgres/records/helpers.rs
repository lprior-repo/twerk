//! Helper functions for record conversions.

use twerk_core::task::TaskState;

/// Helper to convert a string slice to `TaskState`
#[must_use]
pub fn str_to_task_state(s: &str) -> TaskState {
    s.parse().unwrap_or_default()
}

/// Creates a fixed-point timestamp for deterministic tests.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub fn fixed_now() -> time::OffsetDateTime {
    time::OffsetDateTime::new_utc(
        time::Date::from_calendar_date(2026, time::Month::March, 22).unwrap_or_else(|_| {
            time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
        }),
        time::Time::from_hms(12, 0, 0).unwrap_or(time::Time::MIDNIGHT),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic)]
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
            assert_eq!(converted.to_string(), *state);
        }
    }
}
