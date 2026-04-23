//! QueueName newtype wrapper.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated queue identifier.
///
/// Rules: 1-128 characters, lowercase ASCII alphanumeric, hyphens, underscores, dots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "QueueName should be used; it validates at construction"]
pub struct QueueName(String);

/// Errors that can arise when constructing a [`QueueName`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueueNameError {
    #[error("queue name length {0} is invalid (must be 1-128 chars)")]
    InvalidLength(usize),
    #[error("queue name contains invalid characters")]
    InvalidCharacter,
    #[error("queue name \"{0}\" is reserved")]
    Reserved(String),
}

impl QueueName {
    /// Create a new `QueueName`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`QueueNameError::InvalidLength`] if name is empty or > 128 chars.
    /// Returns [`QueueNameError::InvalidCharacter`] if name contains non-allowed chars.
    pub fn new(name: impl Into<String>) -> Result<Self, QueueNameError> {
        let s = name.into();
        if s.is_empty() || s.len() > 128 {
            return Err(QueueNameError::InvalidLength(s.len()));
        }
        if !s.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        }) {
            return Err(QueueNameError::InvalidCharacter);
        }
        Ok(Self(s))
    }

    /// View the queue name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QueueName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for QueueName {
    type Err = QueueNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for QueueName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for QueueName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_name_valid() {
        let q = QueueName::new("my-queue_01.2").unwrap();
        assert_eq!(q.as_str(), "my-queue_01.2");
    }

    #[test]
    fn queue_name_rejects_empty() {
        assert!(matches!(
            QueueName::new(""),
            Err(QueueNameError::InvalidLength(0))
        ));
    }

    #[test]
    fn queue_name_rejects_too_long() {
        let long = "a".repeat(129);
        assert!(matches!(
            QueueName::new(&long),
            Err(QueueNameError::InvalidLength(129))
        ));
    }

    #[test]
    fn queue_name_rejects_uppercase() {
        assert!(matches!(
            QueueName::new("MyQueue"),
            Err(QueueNameError::InvalidCharacter)
        ));
    }

    #[test]
    fn queue_name_from_str_roundtrip() {
        let q: QueueName = "hello.world".parse().unwrap();
        assert_eq!(q.to_string(), "hello.world");
    }

    #[test]
    fn queue_name_deref() {
        let q = QueueName::new("test").unwrap();
        assert!(q.contains("es"));
    }
}
