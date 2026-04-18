//! Authentication domain types.
//!
//! # Design Principles
//!
//! - **Parse, don't validate**: Raw input is parsed into domain types at
//!   boundary entry points. Core logic receives only validated types.
//! - **Make illegal states unrepresentable**: Newtypes encode business rules
//!   that cannot be violated after construction.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated username.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Username(String);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum UsernameError {
    #[error("username cannot be empty")]
    Empty,
    #[error("username must be 3-64 characters")]
    LengthOutOfRange,
    #[error("username must start with a letter and contain only alphanumeric characters, underscores, or hyphens")]
    InvalidCharacter,
}

impl Username {
    /// Create a new `Username` from a string.
    ///
    /// # Errors
    ///
    /// Returns [`UsernameError`] if the username is invalid.
    ///
    /// # Panics
    ///
    /// Panics if the internal invariant that username has at least 3 characters is violated.
    /// This invariant is guaranteed by the length check above.
    pub fn new(username: impl Into<String>) -> Result<Self, UsernameError> {
        let s = username.into();

        if s.is_empty() {
            return Err(UsernameError::Empty);
        }

        let len = s.len();
        if !(3..=64).contains(&len) {
            return Err(UsernameError::LengthOutOfRange);
        }

        let mut chars = s.chars();
        #[allow(clippy::unwrap_used)]
        let first = chars.next().unwrap();
        if !first.is_alphabetic() {
            return Err(UsernameError::InvalidCharacter);
        }

        for c in chars {
            if !c.is_alphanumeric() && c != '_' && c != '-' {
                return Err(UsernameError::InvalidCharacter);
            }
        }

        Ok(Self(s))
    }

    /// View the username as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Username {
    type Err = UsernameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Username {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Username {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A validated password (minimum requirements enforced).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Password(String);

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PasswordError {
    #[error("password cannot be empty")]
    Empty,
    #[error("password must be at least 8 characters")]
    TooShort,
}

impl Password {
    /// Create a new `Password` from a string.
    ///
    /// # Errors
    ///
    /// Returns [`PasswordError`] if the password is too short.
    pub fn new(password: impl Into<String>) -> Result<Self, PasswordError> {
        let s = password.into();

        if s.is_empty() {
            return Err(PasswordError::Empty);
        }

        if s.len() < 8 {
            return Err(PasswordError::TooShort);
        }

        Ok(Self(s))
    }

    /// View the password as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl AsRef<str> for Password {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_valid() {
        let u = Username::new("john_doe").unwrap();
        assert_eq!(u.as_str(), "john_doe");
    }

    #[test]
    fn username_rejects_short() {
        assert!(matches!(
            Username::new("ab"),
            Err(UsernameError::LengthOutOfRange)
        ));
    }

    #[test]
    fn username_rejects_invalid_start() {
        assert!(matches!(
            Username::new("123_user"),
            Err(UsernameError::InvalidCharacter)
        ));
    }

    #[test]
    fn password_valid() {
        let p = Password::new("secretpassword123").unwrap();
        assert_eq!(p.as_str(), "secretpassword123");
    }

    #[test]
    fn password_rejects_short() {
        assert!(matches!(
            Password::new("short"),
            Err(PasswordError::TooShort)
        ));
    }
}
