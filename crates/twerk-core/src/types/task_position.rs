//! Task position types.
//!
//! Provides [`TaskPosition`] - a validated task position (any i64 including negative).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task position (any i64 including negative).
///
/// TaskPosition supports negative values for relative offsets from the end
/// (e.g., -1 for the last task, -2 for second to last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "TaskPosition should be used; it validates at construction"]
pub struct TaskPosition(i64);

/// Errors that can arise when constructing a [`TaskPosition`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskPositionError {
    #[error("Invalid task position")]
    Invalid,
}

impl TaskPosition {
    /// Create a new `TaskPosition` from an i64 value.
    ///
    /// This always succeeds since i64 has no range restriction.
    pub fn new(value: i64) -> Result<Self, TaskPositionError> {
        Ok(Self(value))
    }

    /// Returns the raw task position value.
    #[must_use]
    pub fn value(self) -> i64 {
        self.0
    }
}

impl fmt::Display for TaskPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for TaskPosition {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<i64> for TaskPosition {
    fn as_ref(&self) -> &i64 {
        &self.0
    }
}

impl From<i64> for TaskPosition {
    fn from(value: i64) -> Self {
        Self(value)
    }
}
