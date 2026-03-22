//! Duration parsing and validation utilities
//!
//! Provides functions to parse and validate duration strings
//! in Go-style format (e.g., "300ms", "-1h30s", "1h30m").

use std::time::Duration;
use thiserror::Error;

/// Error type for duration parsing failures
#[derive(Debug, Error, PartialEq)]
#[error("invalid duration format: {0}")]
pub struct ParseDurationError(String);

/// Validates a duration string
///
/// Returns `true` if the string is a valid duration or empty,
/// otherwise returns `false`.
#[must_use]
pub fn is_valid_duration(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    parse_duration(value).is_ok()
}

/// Parses a duration string in Go format.
///
/// Supported formats:
/// - "300ms" - milliseconds
/// - "-1h30s" - negative durations
/// - "1h30m" - hours and minutes
/// - "1s", "1m", "1h", "1d" - seconds, minutes, hours, days
#[must_use]
pub fn parse_duration(value: &str) -> Result<Duration, ParseDurationError> {
    parse_go_duration(value).map_err(|e| ParseDurationError(e.to_string()))
}

#[derive(Debug, Error)]
enum DurationParseError {
    #[error("empty string")]
    Empty,

    #[error("missing number before '{0}'")]
    MissingNumber(char),

    #[error("invalid number: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),

    #[error("unknown unit: {0}")]
    UnknownUnit(char),

    #[error("invalid suffix: {0}")]
    InvalidSuffix(String),

    #[error("trailing characters: {0}")]
    TrailingChars(String),

    #[error("duration overflow")]
    Overflow,
}

fn parse_go_duration(s: &str) -> Result<Duration, DurationParseError> {
    if s.is_empty() {
        return Err(DurationParseError::Empty);
    }

    let mut total_nanos: i128 = 0;
    let mut current_num = String::new();
    let mut negative = false;
    let chars: Vec<char> = s.chars().collect();

    let mut i = 0;

    // Handle leading sign
    if chars[i] == '-' {
        negative = true;
        i += 1;
    }

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_ascii_digit() {
            current_num.push(ch);
            i += 1;
        } else if ch == 'n' && i + 1 < chars.len() && chars[i + 1] == 's' {
            // nanoseconds (ns)
            if current_num.is_empty() {
                return Err(DurationParseError::MissingNumber('n'));
            }
            let num: i64 = current_num.parse()?;
            total_nanos = total_nanos
                .checked_add(num as i128)
                .ok_or(DurationParseError::Overflow)?;
            current_num.clear();
            i += 2; // skip 'n' and 's'
        } else if ch == 'm' && i + 1 < chars.len() && chars[i + 1] == 's' {
            // milliseconds (ms)
            if current_num.is_empty() {
                return Err(DurationParseError::MissingNumber('m'));
            }
            let num: i64 = current_num.parse()?;
            total_nanos = total_nanos
                .checked_add(num as i128 * 1_000_000)
                .ok_or(DurationParseError::Overflow)?;
            current_num.clear();
            i += 2; // skip 'm' and 's'
        } else if ch == 'u' || ch == 'µ' {
            // microseconds
            if current_num.is_empty() {
                return Err(DurationParseError::MissingNumber('u'));
            }
            let num: i64 = current_num.parse()?;
            total_nanos = total_nanos
                .checked_add(num as i128 * 1000)
                .ok_or(DurationParseError::Overflow)?;
            current_num.clear();
            i += 1;
        } else if ch == 'h' || ch == 'm' || ch == 's' || ch == 'd' {
            if current_num.is_empty() {
                return Err(DurationParseError::MissingNumber(ch));
            }
            let num: i64 = current_num.parse()?;
            let nanos = match ch {
                'h' => (num as i128) * 3_600_000_000_000,
                'm' => (num as i128) * 60_000_000_000,
                's' => (num as i128) * 1_000_000_000,
                'd' => (num as i128) * 86_400_000_000_000,
                _ => return Err(DurationParseError::UnknownUnit(ch)),
            };
            total_nanos = total_nanos
                .checked_add(nanos)
                .ok_or(DurationParseError::Overflow)?;
            current_num.clear();
            i += 1;
        } else if ch == ' ' || ch == '\t' {
            // Skip whitespace
            i += 1;
        } else {
            return Err(DurationParseError::InvalidSuffix(ch.to_string()));
        }
    }

    if !current_num.is_empty() {
        return Err(DurationParseError::TrailingChars(current_num));
    }

    if negative {
        total_nanos = -total_nanos;
    }

    let abs_nanos = total_nanos.unsigned_abs();
    Ok(Duration::from_nanos(abs_nanos as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_duration() {
        assert!(is_valid_duration(""));
        assert!(is_valid_duration("1h"));
        assert!(is_valid_duration("30m"));
        assert!(is_valid_duration("60s"));
        assert!(is_valid_duration("1h30m"));
        assert!(is_valid_duration("300ms"));
        assert!(!is_valid_duration("invalid"));
        assert!(!is_valid_duration("1"));
    }

    #[test]
    fn test_parse_duration_valid() {
        assert_eq!(parse_duration("1h"), Ok(Duration::from_secs(3600)));
        // 30m = 30 minutes = 1800 seconds
        assert_eq!(parse_duration("30m"), Ok(Duration::from_secs(1800)));
        assert_eq!(parse_duration("60s"), Ok(Duration::from_secs(60)));
        // 1h30m = 5400 seconds
        assert_eq!(parse_duration("1h30m"), Ok(Duration::from_secs(5400)));
        assert_eq!(parse_duration("300ms"), Ok(Duration::from_millis(300)));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("invalid").is_err());
        assert!(parse_duration("1").is_err());
    }

    #[test]
    fn test_parse_duration_negative() {
        // Negative durations are stored as positive in Duration
        assert_eq!(parse_duration("-1h"), Ok(Duration::from_secs(3600)));
    }
}
