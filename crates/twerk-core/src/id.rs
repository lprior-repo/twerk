use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use thiserror::Error;

const MAX_ID_LENGTH: usize = 1000;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IdError {
    #[error("ID cannot be empty")]
    Empty,
    #[error("ID exceeds maximum length of {MAX_ID_LENGTH} characters: {0} characters")]
    TooLong(usize),
    #[error("ID contains invalid characters: only alphanumeric, dash, and underscore allowed")]
    InvalidCharacters,
}

fn validate_id(s: &str) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::Empty);
    }
    if s.len() > MAX_ID_LENGTH {
        return Err(IdError::TooLong(s.len()));
    }
    if !s
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(IdError::InvalidCharacters);
    }
    Ok(())
}

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a new ID from a string.
            ///
            /// # Errors
            ///
            /// Returns `IdError` if the string is empty, too long, or contains invalid characters.
            pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
                let s = id.into();
                validate_id(&s)?;
                Ok(Self(s))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }
    };
}

define_id!(JobId);
define_id!(TaskId);
define_id!(NodeId);
define_id!(ScheduledJobId);
define_id!(UserId);
define_id!(RoleId);
