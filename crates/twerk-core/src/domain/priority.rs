//! Priority newtype wrapper.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
