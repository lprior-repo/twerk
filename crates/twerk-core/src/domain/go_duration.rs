//! GoDuration newtype wrapper for Go-style duration strings.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A parsed Go-style duration string (e.g. `"30s"`, `"1h30m"`, `"2d"`).
///
/// The original string representation is preserved for serialisation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "GoDuration should be used; it validates at construction"]
pub struct GoDuration(String);

/// Errors that can arise when constructing a [`GoDuration`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoDurationError {
    #[error("empty duration string")]
    Empty,
    #[error("no unit specified in duration segment near index {0}")]
    NoUnit(usize),
    #[error("unknown duration unit: '{0}'")]
    UnknownUnit(char),
    #[error("failed to parse duration number near index {0}")]
    ParseNumber(usize),
    #[error("zero duration is not allowed (duration must be positive)")]
    ZeroDuration,
}

impl GoDuration {
    /// Create a new `GoDuration`, returning an error if parsing fails.
    ///
    /// # Errors
    /// Returns [`GoDurationError::Empty`] if the string is empty.
    /// Returns [`GoDurationError::NoUnit`] if a duration segment lacks a unit.
    /// Returns [`GoDurationError::UnknownUnit`] if an unknown unit is found.
    /// Returns [`GoDurationError::ParseNumber`] if a number can't be parsed.
    pub fn new(s: impl Into<String>) -> Result<Self, GoDurationError> {
        let original = s.into();
        if original.is_empty() {
            return Err(GoDurationError::Empty);
        }
        // Validate by performing a full parse.
        parse_go_duration(&original)?;
        Ok(Self(original))
    }

    /// Convert to a `std::time::Duration`.
    ///
    /// Since `Self` is only constructible via `new()` which validates the format,
    /// this method is guaranteed to succeed for all valid instances.
    #[must_use]
    pub fn to_duration(&self) -> Duration {
        match parse_go_duration(&self.0) {
            Ok(d) => d,
            Err(_) => Duration::ZERO, // unreachable for valid GoDuration
        }
    }

    /// View the original Go-style duration string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GoDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for GoDuration {
    type Err = GoDurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Parse a Go-style duration string into a [`Duration`].
///
/// Supported units: `ns`, `us`/`µs`, `ms`, `s`, `m`, `h`, `d`.
/// Segments can be combined, e.g. `"1h30m"`, `"2d12h30m"`.
fn parse_go_duration(s: &str) -> Result<Duration, GoDurationError> {
    if s.is_empty() {
        return Err(GoDurationError::Empty);
    }

    let mut total = Duration::ZERO;
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Read the numeric part.
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        if i == start {
            // No digits found.
            return Err(GoDurationError::ParseNumber(start));
        }
        let num_str = &s[start..i];
        let is_float = num_str.contains('.');

        // Determine the unit.
        if i >= bytes.len() {
            return Err(GoDurationError::NoUnit(start));
        }

        let (unit_len, multiplier): (usize, u128) = if s[i..].starts_with("ns") {
            (2, 1)
        } else if s[i..].starts_with("us") {
            (2, 1_000)
        } else if s[i..].starts_with("\u{00b5}s") {
            // µs (micro sign U+00B5)
            ("\u{00b5}s".len(), 1_000)
        } else if s[i..].starts_with("ms") {
            (2, 1_000_000)
        } else if s[i..].starts_with('s') {
            (1, 1_000_000_000)
        } else if s[i..].starts_with('m') {
            (1, 60_000_000_000)
        } else if s[i..].starts_with('h') {
            (1, 3_600_000_000_000)
        } else if s[i..].starts_with('d') {
            (1, 86_400_000_000_000)
        } else {
            return Err(GoDurationError::UnknownUnit(bytes[i] as char));
        };

        let nanos = if is_float {
            let val: f64 = num_str
                .parse()
                .map_err(|_| GoDurationError::ParseNumber(start))?;
            // Safe: multiplier fits in f64 without precision loss for reasonable durations
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let result = (val * multiplier as f64) as u128;
            result
        } else {
            let val: u64 = num_str
                .parse()
                .map_err(|_| GoDurationError::ParseNumber(start))?;
            u128::from(val) * multiplier
        };

        // Safe: nanos won't exceed u64::MAX for any reasonable duration string
        total += Duration::from_nanos(u64::try_from(nanos).map_or(u64::MAX, |n| n));
        i += unit_len;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn go_duration_seconds() {
        let d = GoDuration::new("30s").unwrap();
        assert_eq!(d.to_duration(), Duration::from_secs(30));
    }

    #[test]
    fn go_duration_combo() {
        let d = GoDuration::new("1h30m").unwrap();
        assert_eq!(d.to_duration(), Duration::from_secs(90 * 60));
    }

    #[test]
    fn go_duration_days() {
        let d = GoDuration::new("2d").unwrap();
        assert_eq!(d.to_duration(), Duration::from_secs(2 * 24 * 3600));
    }

    #[test]
    fn go_duration_millis() {
        let d = GoDuration::new("500ms").unwrap();
        assert_eq!(d.to_duration(), Duration::from_millis(500));
    }

    #[test]
    fn go_duration_micros() {
        let d = GoDuration::new("100us").unwrap();
        assert_eq!(d.to_duration(), Duration::from_micros(100));
    }

    #[test]
    fn go_duration_nanos() {
        let d = GoDuration::new("100ns").unwrap();
        assert_eq!(d.to_duration(), Duration::from_nanos(100));
    }

    #[test]
    fn go_duration_complex() {
        let d = GoDuration::new("2d12h30m15s500ms").unwrap();
        let expected =
            Duration::from_secs(2 * 86400 + 12 * 3600 + 30 * 60 + 15) + Duration::from_millis(500);
        assert_eq!(d.to_duration(), expected);
    }

    #[test]
    fn go_duration_empty_rejected() {
        assert!(matches!(GoDuration::new(""), Err(GoDurationError::Empty)));
    }

    #[test]
    fn go_duration_from_str_roundtrip() {
        let d: GoDuration = "1h30m".parse().unwrap();
        assert_eq!(d.to_string(), "1h30m");
    }
}
