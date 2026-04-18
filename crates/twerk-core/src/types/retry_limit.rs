//! Task retry limit types.
//!
//! Provides [`RetryLimit`] - a validated task retry limit (non-negative u32).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task retry limit (non-negative u32).
///
/// Unlike the Priority type in domain_types, this is a simple u32 wrapper
/// with no upper bound restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(transparent)]
#[must_use = "RetryLimit should be used; it validates at construction"]
pub struct RetryLimit(u32);

/// Errors that can arise when constructing a [`RetryLimit`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RetryLimitError {
    #[error("Optional retry limit must be present")]
    NoneNotAllowed,
}

impl RetryLimit {
    /// Create a new `RetryLimit` from a u32 value.
    ///
    /// This always succeeds since u32 is always non-negative.
    pub fn new(value: u32) -> Result<Self, RetryLimitError> {
        Ok(Self(value))
    }

    /// Create a new `RetryLimit` from an `Option<u32>`.
    ///
    /// # Errors
    /// Returns [`RetryLimitError::NoneNotAllowed`] if value is None.
    pub fn from_option(value: Option<u32>) -> Result<Self, RetryLimitError> {
        value.ok_or(RetryLimitError::NoneNotAllowed).map(Self)
    }

    /// Returns the raw retry limit value.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RetryLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for RetryLimit {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u32> for RetryLimit {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}

impl From<u32> for RetryLimit {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
