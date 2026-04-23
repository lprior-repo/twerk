//! Task progress percentage types.
//!
//! Provides [`Progress`] - a validated task progress percentage (0.0-100.0).

use core::fmt;
use core::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated task progress percentage (0.0-100.0).
///
/// Progress values must be in the percentage range [0.0, 100.0] and not NaN.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
#[must_use = "Progress should be used; it validates at construction"]
pub struct Progress(f64);

/// Errors that can arise when constructing a [`Progress`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ProgressError {
    #[error("Progress {value} out of valid range {min}..={max}")]
    OutOfRange { value: f64, min: f64, max: f64 },
    #[error("Progress NaN is not a valid progress")]
    NaN,
}

impl Progress {
    /// Create a new `Progress`, returning an error if outside valid range.
    ///
    /// # Errors
    /// Returns [`ProgressError::OutOfRange`] if value is < 0.0 or > 100.0.
    /// Returns [`ProgressError::NaN`] if value is NaN.
    pub fn new(value: f64) -> Result<Self, ProgressError> {
        if value.is_nan() {
            Err(ProgressError::NaN)
        } else if !(0.0..=100.0).contains(&value) {
            Err(ProgressError::OutOfRange {
                value,
                min: 0.0,
                max: 100.0,
            })
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the raw progress value.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for Progress {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<f64> for Progress {
    fn as_ref(&self) -> &f64 {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Progress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Progress::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn progress_new_accepts_valid_range(v in 0.0f64..=100.0) {
            prop_assert!(Progress::new(v).is_ok());
        }

        #[test]
        fn progress_new_rejects_negative(v in -1000.0f64..-0.001) {
            prop_assert!(Progress::new(v).is_err());
        }

        #[test]
        fn progress_new_rejects_over_100(v in 100.001f64..10000.0) {
            prop_assert!(Progress::new(v).is_err());
        }

        #[test]
        fn progress_serde_roundtrip(v in 0.0f64..=100.0) {
            let p = Progress::new(v).unwrap();
            let json = serde_json::to_string(&p).unwrap();
            let back: Progress = serde_json::from_str(&json).unwrap();
            prop_assert!((p.value() - back.value()).abs() <= f64::EPSILON * p.value().abs().max(1.0));
        }
    }
}
