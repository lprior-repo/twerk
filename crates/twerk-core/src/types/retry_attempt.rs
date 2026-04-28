//! Task retry attempt counter types.
//!
//! Provides [`RetryAttempt`] - a validated task retry attempt counter (non-negative u32).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task retry attempt counter (non-negative u32).
///
/// This represents the current attempt number in a retry sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(transparent)]
#[must_use = "RetryAttempt should be used; it validates at construction"]
pub struct RetryAttempt(u32);

/// Errors that can arise when constructing a [`RetryAttempt`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RetryAttemptError {
    #[error("Invalid retry attempt")]
    Invalid,
}

impl RetryAttempt {
    /// Create a new `RetryAttempt` from a u32 value.
    ///
    /// This always succeeds since u32 is always non-negative.
    pub fn new(value: u32) -> Result<Self, RetryAttemptError> {
        Ok(Self(value))
    }

    /// Returns the raw retry attempt value.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RetryAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for RetryAttempt {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u32> for RetryAttempt {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}

impl From<u32> for RetryAttempt {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
