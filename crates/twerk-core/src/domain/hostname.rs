//! Hostname newtype wrapper.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated DNS hostname with RFC 1123 compliance.
///
/// Validation rules:
/// - Each label: 1-63 characters
/// - Labels: alphanumeric ASCII, hyphens allowed (but not at start or end)
/// - Total length: 1-253 characters
/// - No port number (reject `:` character explicitly)
/// - Case-insensitive but preserved as-is
/// - Labels cannot be all-numeric (to avoid ambiguity with IP addresses)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[must_use = "Hostname should be used; it validates at construction"]
pub struct Hostname(String);

/// Errors that can arise when constructing a [`Hostname`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HostnameError {
    #[error("hostname cannot be empty")]
    Empty,
    #[error("hostname exceeds maximum length of 253 characters: {0}")]
    TooLong(usize),
    #[error("hostname contains invalid character: {0}")]
    InvalidCharacter(char),
    #[error("hostname label \"{0}\" is invalid: {1}")]
    InvalidLabel(String, String),
    #[error("hostname label \"{0}\" exceeds 63 characters: {1}")]
    LabelTooLong(String, usize),
}

impl Hostname {
    /// Create a new `Hostname`, returning an error if validation fails.
    ///
    /// # Errors
    /// Returns [`HostnameError::Empty`] if the string is empty.
    /// Returns [`HostnameError::TooLong`] if length exceeds 253 characters.
    /// Returns [`HostnameError::InvalidCharacter`] if contains disallowed characters like `:`.
    /// Returns [`HostnameError::InvalidLabel`] if a label is all-numeric or otherwise invalid.
    /// Returns [`HostnameError::LabelTooLong`] if a label exceeds 63 characters.
    pub fn new(hostname: impl Into<String>) -> Result<Self, HostnameError> {
        let s = hostname.into();

        validate_not_empty(&s)?;
        validate_length(&s)?;
        validate_no_colon(&s)?;
        validate_labels(&s)?;

        Ok(Self(s))
    }

    /// View the hostname as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Private validation helpers
// ---------------------------------------------------------------------------

/// Validates that the hostname string is not empty.
fn validate_not_empty(s: &str) -> Result<(), HostnameError> {
    if s.is_empty() {
        Err(HostnameError::Empty)
    } else {
        Ok(())
    }
}

/// Validates that the hostname length is within 1-253 characters.
fn validate_length(s: &str) -> Result<(), HostnameError> {
    if s.len() > 253 {
        Err(HostnameError::TooLong(s.len()))
    } else {
        Ok(())
    }
}

/// Validates that the hostname does not contain a colon character.
fn validate_no_colon(s: &str) -> Result<(), HostnameError> {
    s.find(':')
        .map_or(Ok(()), |_| Err(HostnameError::InvalidCharacter(':')))
}

/// Validates all labels in the hostname against RFC 1123 rules.
fn validate_labels(s: &str) -> Result<(), HostnameError> {
    s.split('.').try_for_each(validate_single_label)
}

/// Validates a single hostname label against RFC 1123 rules.
fn validate_single_label(label: &str) -> Result<(), HostnameError> {
    let label_len = label.len();

    // LabelTooLong check (PC1 for individual labels)
    if label_len > 63 {
        return Err(HostnameError::LabelTooLong(label.to_string(), label_len));
    }

    // Empty label check
    if label.is_empty() {
        return Err(HostnameError::InvalidLabel(
            label.to_string(),
            "empty".to_string(),
        ));
    }

    // First character must be alphanumeric
    let first_char = label.chars().next();
    if let Some(c) = first_char {
        if !c.is_ascii_alphanumeric() {
            return Err(HostnameError::InvalidLabel(
                label.to_string(),
                "must start with alphanumeric".to_string(),
            ));
        }
    }

    // Last character must be alphanumeric
    let last_char = label.chars().last();
    if let Some(c) = last_char {
        if !c.is_ascii_alphanumeric() {
            return Err(HostnameError::InvalidLabel(
                label.to_string(),
                "must end with alphanumeric".to_string(),
            ));
        }
    }

    // Middle characters: alphanumeric or hyphen
    for (i, c) in label.chars().enumerate() {
        if i == 0 || i == label_len - 1 {
            continue; // Already checked first and last
        }
        if !c.is_ascii_alphanumeric() && c != '-' {
            return Err(HostnameError::InvalidCharacter(c));
        }
    }

    // PC5: Labels cannot be all-numeric
    if label.chars().all(|c| c.is_ascii_digit()) {
        return Err(HostnameError::InvalidLabel(
            label.to_string(),
            "all_numeric".to_string(),
        ));
    }

    Ok(())
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Hostname {
    type Err = HostnameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Hostname {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Hostname {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[cfg(test)]
mod tests;
