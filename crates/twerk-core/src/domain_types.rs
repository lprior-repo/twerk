//! Newtype wrappers for domain primitives.
//!
//! These types enforce validation at construction time, making illegal states
//! unrepresentable throughout the codebase.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// QueueName
// ---------------------------------------------------------------------------

/// A validated queue identifier.
///
/// Rules: 1-128 characters, lowercase ASCII alphanumeric, hyphens, underscores, dots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "QueueName should be used; it validates at construction"]
pub struct QueueName(String);

/// Errors that can arise when constructing a [`QueueName`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueueNameError {
    #[error("queue name length {0} is invalid (must be 1-128 chars)")]
    InvalidLength(usize),
    #[error("queue name contains invalid characters")]
    InvalidCharacter,
    #[error("queue name \"{0}\" is reserved")]
    Reserved(String),
}

impl QueueName {
    /// Create a new `QueueName`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`QueueNameError::InvalidLength`] if name is empty or > 128 chars.
    /// Returns [`QueueNameError::InvalidCharacter`] if name contains non-allowed chars.
    pub fn new(name: impl Into<String>) -> Result<Self, QueueNameError> {
        let s = name.into();
        if s.is_empty() || s.len() > 128 {
            return Err(QueueNameError::InvalidLength(s.len()));
        }
        if !s.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        }) {
            return Err(QueueNameError::InvalidCharacter);
        }
        Ok(Self(s))
    }

    /// View the queue name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QueueName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for QueueName {
    type Err = QueueNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for QueueName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for QueueName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// CronExpression
// ---------------------------------------------------------------------------

/// A validated cron expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "CronExpression should be used; it validates at construction"]
pub struct CronExpression(String);

/// Errors that can arise when constructing a [`CronExpression`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CronError {
    #[error("invalid cron expression: {0}")]
    InvalidExpression(String),
}

impl CronExpression {
    /// Create a new `CronExpression`, returning an error if the expression
    /// cannot be parsed by the `cron` crate.
    ///
    /// # Errors
    /// Returns [`CronError::InvalidExpression`] if the cron expression is malformed.
    pub fn new(expr: impl Into<String>) -> Result<Self, CronError> {
        let s = expr.into();
        cron::Schedule::from_str(&s).map_err(|e| CronError::InvalidExpression(e.to_string()))?;
        Ok(Self(s))
    }

    /// View the cron expression as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CronExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CronExpression {
    type Err = CronError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

// ---------------------------------------------------------------------------
// GoDuration
// ---------------------------------------------------------------------------

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
    /// this method is guaranteed to succeed.
    #[must_use]
    pub fn to_duration(&self) -> Duration {
        parse_go_duration(&self.0)
            .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
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
        total += Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX));
        i += unit_len;
    }

    Ok(total)
}

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

/// A validated job/Task priority value (0-9).
///
/// Lower values = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "Priority should be used; it validates at construction"]
pub struct Priority(i64);

/// Errors that can arise when constructing a [`Priority`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PriorityError {
    #[error("priority {0} is out of range (must be 0-9)")]
    OutOfRange(i64),
}

impl Priority {
    /// Create a new `Priority`, returning an error if outside 0..=9.
    ///
    /// # Errors
    /// Returns [`PriorityError::OutOfRange`] if value is not in 0..=9.
    pub fn new(value: i64) -> Result<Self, PriorityError> {
        if (0..=9).contains(&value) {
            Ok(Self(value))
        } else {
            Err(PriorityError::OutOfRange(value))
        }
    }

    /// Returns the raw priority value.
    #[must_use]
    pub fn value(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ---------------------------------------------------------------------------
// RetryLimit
// ---------------------------------------------------------------------------

/// A validated task retry limit (1-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "RetryLimit should be used; it validates at construction"]
pub struct RetryLimit(i64);

/// Errors that can arise when constructing a [`RetryLimit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RetryLimitError {
    #[error("retry limit {0} is out of range (must be 1-10)")]
    OutOfRange(i64),
}

impl RetryLimit {
    /// Create a new `RetryLimit`, returning an error if outside 1..=10.
    ///
    /// # Errors
    /// Returns [`RetryLimitError::OutOfRange`] if value is not in 1..=10.
    pub fn new(value: i64) -> Result<Self, RetryLimitError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(RetryLimitError::OutOfRange(value))
        }
    }

    /// Returns the raw retry limit value.
    #[must_use]
    pub fn value(self) -> i64 {
        self.0
    }
}

impl fmt::Display for RetryLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ---------------------------------------------------------------------------
// DomainParseError (aggregated error type)
// ---------------------------------------------------------------------------

/// Unified error type for domain parsing failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainParseError {
    #[error("invalid cron expression: {0}")]
    Cron(#[from] CronError),
    #[error("invalid duration: {0}")]
    Duration(#[from] GoDurationError),
    #[error("invalid queue name: {0}")]
    QueueName(#[from] QueueNameError),
    #[error("invalid priority: {0}")]
    Priority(#[from] PriorityError),
    #[error("invalid retry limit: {0}")]
    RetryLimit(#[from] RetryLimitError),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- QueueName ----------------------------------------------------------

    #[test]
    fn queue_name_valid() {
        let q = QueueName::new("my-queue_01.2").unwrap();
        assert_eq!(q.as_str(), "my-queue_01.2");
    }

    #[test]
    fn queue_name_rejects_empty() {
        assert!(matches!(
            QueueName::new(""),
            Err(QueueNameError::InvalidLength(0))
        ));
    }

    #[test]
    fn queue_name_rejects_too_long() {
        let long = "a".repeat(129);
        assert!(matches!(
            QueueName::new(&long),
            Err(QueueNameError::InvalidLength(129))
        ));
    }

    #[test]
    fn queue_name_rejects_uppercase() {
        assert!(matches!(
            QueueName::new("MyQueue"),
            Err(QueueNameError::InvalidCharacter)
        ));
    }

    #[test]
    fn queue_name_from_str_roundtrip() {
        let q: QueueName = "hello.world".parse().unwrap();
        assert_eq!(q.to_string(), "hello.world");
    }

    #[test]
    fn queue_name_deref() {
        let q = QueueName::new("test").unwrap();
        assert!(q.contains("es"));
    }

    // -- CronExpression -----------------------------------------------------

    #[test]
    fn cron_valid() {
        let c = CronExpression::new("0 0 * * * *").unwrap();
        assert_eq!(c.as_str(), "0 0 * * * *");
    }

    #[test]
    fn cron_invalid() {
        assert!(matches!(
            CronExpression::new("not-a-cron"),
            Err(CronError::InvalidExpression(_))
        ));
    }

    #[test]
    fn cron_from_str_roundtrip() {
        let c: CronExpression = "0 30 9 * * Mon-Fri".parse().unwrap();
        assert_eq!(c.to_string(), "0 30 9 * * Mon-Fri");
    }

    // -- GoDuration ---------------------------------------------------------

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
