//! Task count types.
//!
//! Provides [`TaskCount`] - a validated task count (non-negative u32).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task count (non-negative u32).
///
/// This represents the total number of tasks in a batch or queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(transparent)]
#[must_use = "TaskCount should be used; it validates at construction"]
pub struct TaskCount(u32);

/// Errors that can arise when constructing a [`TaskCount`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskCountError {
    #[error("Optional task count must be present")]
    NoneNotAllowed,
}

impl TaskCount {
    /// Create a new `TaskCount` from a u32 value.
    ///
    /// This always succeeds since u32 is always non-negative.
    pub fn new(value: u32) -> Result<Self, TaskCountError> {
        Ok(Self(value))
    }

    /// Create a new `TaskCount` from an `Option<u32>`.
    ///
    /// # Errors
    /// Returns [`TaskCountError::NoneNotAllowed`] if value is None.
    pub fn from_option(value: Option<u32>) -> Result<Self, TaskCountError> {
        value.ok_or(TaskCountError::NoneNotAllowed).map(Self)
    }

    /// Returns the raw task count value.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for TaskCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for TaskCount {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u32> for TaskCount {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}

impl From<u32> for TaskCount {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
