//! Shared test utilities for integration tests.

/// Creates a fixed-point timestamp for deterministic tests.
/// Returns `2026-03-22T12:00:00Z`.
#[allow(clippy::unwrap_used)]
pub fn fixed_now() -> time::OffsetDateTime {
    time::OffsetDateTime::new_utc(
        time::Date::from_calendar_date(2026, time::Month::March, 22).unwrap_or_else(|_| {
            time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
        }),
        time::Time::from_hms(12, 0, 0).unwrap_or(time::Time::MIDNIGHT),
    )
}
