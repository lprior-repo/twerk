//! Task retry limit types.
//!
//! Provides [`RetryLimit`] - a validated task retry limit (1-10).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task retry limit (1-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
#[must_use = "RetryLimit should be used; it validates at construction"]
pub struct RetryLimit(u32);

const MIN_RETRY_LIMIT: u32 = 1;
const MAX_RETRY_LIMIT: u32 = 10;

/// Errors that can arise when constructing a [`RetryLimit`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OptionalRetryLimitError {
    #[error("Optional retry limit must be present")]
    NoneNotAllowed,
    #[error("retry limit {value} is out of range (must be {min}-{max})")]
    OutOfRange { value: u32, min: u32, max: u32 },
}

impl RetryLimit {
    /// Create a new `RetryLimit` from a u32 value.
    ///
    /// # Errors
    /// Returns [`OptionalRetryLimitError::OutOfRange`] when `value` is outside `1..=10`.
    pub fn new(value: u32) -> Result<Self, OptionalRetryLimitError> {
        if (MIN_RETRY_LIMIT..=MAX_RETRY_LIMIT).contains(&value) {
            Ok(Self(value))
        } else {
            Err(OptionalRetryLimitError::OutOfRange {
                value,
                min: MIN_RETRY_LIMIT,
                max: MAX_RETRY_LIMIT,
            })
        }
    }

    /// Create a new `RetryLimit` from an `Option<u32>`.
    ///
    /// # Errors
    /// Returns [`OptionalRetryLimitError::NoneNotAllowed`] if value is None.
    pub fn from_option(value: Option<u32>) -> Result<Self, OptionalRetryLimitError> {
        value
            .ok_or(OptionalRetryLimitError::NoneNotAllowed)
            .and_then(Self::new)
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

impl TryFrom<u32> for RetryLimit {
    type Error = OptionalRetryLimitError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for RetryLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}
