//! `CronExpression` newtype wrapper.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated cron schedule expression (5-field or 6-field format).
///
/// Validation rules:
/// - 5-field format: `second? minute hour day_of_month month day_of_week`
/// - 6-field format: `second minute hour day_of_month month day_of_week`
/// - Supported special characters: `*`, `?`, `-`, `,`, `/`
/// - Supported day names: `MON-SUN` (case-insensitive)
/// - Supported month names: `JAN-DEC` (case-insensitive)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "CronExpression should be used; it validates at construction"]
pub struct CronExpression(String);

/// Errors that can arise when constructing a [`CronExpression`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CronExpressionError {
    #[error("cron expression cannot be empty")]
    Empty,
    #[error("invalid cron expression: {0}")]
    ParseError(String),
    #[error("invalid field count: {0} (must be 5 or 6)")]
    InvalidFieldCount(usize),
}

// ---------------------------------------------------------------------------
// Private validation helpers
// ---------------------------------------------------------------------------

fn validate_not_empty(s: &str) -> Result<(), CronExpressionError> {
    if s.is_empty() {
        Err(CronExpressionError::Empty)
    } else {
        Ok(())
    }
}

fn validate_field_count(s: &str) -> Result<usize, CronExpressionError> {
    let count = s.split_whitespace().count();
    if count == 5 || count == 6 {
        Ok(count)
    } else {
        Err(CronExpressionError::InvalidFieldCount(count))
    }
}

fn normalize_for_cron(s: &str, field_count: usize) -> String {
    let normalized = s.to_uppercase();
    if field_count == 5 {
        format!("0 {normalized}")
    } else {
        normalized
    }
}

fn validate_cron_parse(parse_expr: &str) -> Result<(), CronExpressionError> {
    cron::Schedule::from_str(parse_expr)
        .map_err(|e| CronExpressionError::ParseError(e.to_string()))?;
    Ok(())
}

impl CronExpression {
    /// Create a new `CronExpression`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`CronExpressionError::Empty`] if the string is empty.
    /// Returns [`CronExpressionError::ParseError`] if the cron expression is malformed.
    /// Returns [`CronExpressionError::InvalidFieldCount`] if the field count is not 5 or 6.
    pub fn new(expr: impl Into<String>) -> Result<Self, CronExpressionError> {
        let s = expr.into();
        validate_not_empty(&s)?;
        let field_count = validate_field_count(&s)?;
        let parse_expr = normalize_for_cron(&s, field_count);
        validate_cron_parse(&parse_expr)?;
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
        write!(f, "{}", self.0)
    }
}

impl FromStr for CronExpression {
    type Err = CronExpressionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for CronExpression {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for CronExpression {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[cfg(test)]
mod tests;
