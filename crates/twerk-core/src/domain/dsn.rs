//! `Dsn` newtype wrapper for PostgreSQL connection strings.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A PostgreSQL connection string (DSN).
///
/// The DSN format is validated for basic structure but actual
/// connection validation is performed by the PostgreSQL driver.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use = "Dsn should be used; it validates at construction"]
pub struct Dsn(String);

/// Errors that can arise when constructing a [`Dsn`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DsnError {
    #[error("DSN cannot be empty")]
    Empty,
    #[error("DSN must contain key=value pairs separated by spaces or ampersands")]
    InvalidFormat,
}

impl Dsn {
    /// Create a new `Dsn`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`DsnError::Empty`] if the DSN is empty.
    /// Returns [`DsnError::InvalidFormat`] if the DSN doesn't contain key=value pairs.
    pub fn new(dsn: impl Into<String>) -> Result<Self, DsnError> {
        let s = dsn.into();
        if s.trim().is_empty() {
            return Err(DsnError::Empty);
        }
        if !s.contains('=') {
            return Err(DsnError::InvalidFormat);
        }
        Ok(Self(s))
    }

    /// Create a new `Dsn` without validation (use with caution).
    ///
    /// # Safety
    /// The caller must ensure the string is a valid PostgreSQL DSN.
    pub fn new_unchecked(dsn: impl Into<String>) -> Self {
        Self(dsn.into())
    }

    /// View the DSN as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Dsn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Dsn {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Dsn {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Serialize for Dsn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Dsn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_dsn() {
        let dsn = Dsn::new("host=localhost user=foo dbname=bar port=5432").unwrap();
        assert!(dsn.as_str().contains("host=localhost"));
    }

    #[test]
    fn empty_dsn_rejected() {
        let result = Dsn::new("");
        assert!(matches!(result, Err(DsnError::Empty)));
    }

    #[test]
    fn whitespace_only_dsn_rejected() {
        let result = Dsn::new("   ");
        assert!(matches!(result, Err(DsnError::Empty)));
    }

    #[test]
    fn invalid_format_rejected() {
        let result = Dsn::new("not-a-valid-dsn");
        assert!(matches!(result, Err(DsnError::InvalidFormat)));
    }

    #[test]
    fn display_trait() {
        let dsn = Dsn::new("host=localhost").unwrap();
        assert_eq!(format!("{}", dsn), "host=localhost");
    }
}
