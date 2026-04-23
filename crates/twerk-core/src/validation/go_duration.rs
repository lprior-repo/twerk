//! Raw Go-duration parser returning `std::time::Duration`.
//!
//! Supports: `s` (seconds), `m` (minutes), `h` (hours), `d` (days).

use std::time::Duration as StdDuration;

/// Parse a Go-style duration string into `std::time::Duration`.
///
/// Supports: `s` (seconds), `m` (minutes), `h` (hours), `d` (days).
///
/// # Errors
/// Returns a descriptive `String` on empty input, invalid characters, or overflow.
pub fn parse_go_duration(s: &str) -> Result<StdDuration, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let (total_secs, trailing) = accumulate_duration_units(trimmed)?;
    let total_secs = total_secs + trailing;
    to_unsigned_duration(total_secs)
}

/// Accumulate named time units (`s`/`m`/`h`/`d`) and return total seconds
/// plus any trailing (unlabelled) numeric remainder.
fn accumulate_duration_units(s: &str) -> Result<(i64, i64), String> {
    s.chars().try_fold((0i64, 0i64), |(total, num), c| match c {
        '0'..='9' => Ok((total, num * 10 + i64::from(c as u32 - '0' as u32))),
        's' => Ok((total + num, 0)),
        'm' => Ok((total + num * 60, 0)),
        'h' => Ok((total + num * 3600, 0)),
        'd' => Ok((total + num * 86400, 0)),
        _ => Err(format!("invalid duration character: {c}")),
    })
}

/// Convert signed seconds into an unsigned `Duration`, rejecting negatives.
fn to_unsigned_duration(total_secs: i64) -> Result<StdDuration, String> {
    usize::try_from(total_secs)
        .map(|s| StdDuration::from_secs(s as u64))
        .map_err(|_| "duration overflow".into())
}
