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

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn priority_new_accepts_0_to_9(v in 0i64..=9) {
            prop_assert!(Priority::new(v).is_ok());
        }

        #[test]
        fn priority_new_rejects_negative(v in -100i64..=-1) {
            prop_assert!(Priority::new(v).is_err());
        }

        #[test]
        fn priority_new_rejects_over_9(v in 10i64..=100) {
            prop_assert!(Priority::new(v).is_err());
        }

        #[test]
        fn priority_value_roundtrip(v in 0i64..=9) {
            let p = Priority::new(v).unwrap();
            prop_assert_eq!(p.value(), v);
        }

        #[test]
        fn priority_serde_roundtrip(v in 0i64..=9) {
            let p = Priority::new(v).unwrap();
            let json = serde_json::to_string(&p).unwrap();
            let back: Priority = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(p, back);
        }
    }
}
