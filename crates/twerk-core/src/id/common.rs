use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use thiserror::Error;

pub(crate) const MAX_ID_LENGTH: usize = 1000;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IdError {
    #[error("ID cannot be empty")]
    Empty,
    #[error("ID is too short: {0} characters (minimum 3)")]
    TooShort(usize),
    #[error("ID is too long: {0} characters (maximum {MAX_ID_LENGTH})")]
    TooLong(usize),
    #[error("ID contains invalid characters: only alphanumeric, dash, and underscore allowed")]
    InvalidCharacters,
    #[error("invalid job ID format: expected RFC 4122 UUID or 22-character base57 short ID")]
    InvalidJobIdFormat,
}

pub(crate) fn validate_id(value: &str) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > MAX_ID_LENGTH {
        return Err(IdError::TooLong(value.len()));
    }
    if value
        .chars()
        .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
    {
        Ok(())
    } else {
        Err(IdError::InvalidCharacters)
    }
}

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
                let value = id.into();
                validate_id(&value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                match Self::new(value) {
                    Ok(id) => id,
                    Err(e) => panic!("From<String> for {}: {e}", stringify!($name)),
                }
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match Self::new(value) {
                    Ok(id) => id,
                    Err(e) => panic!("From<&str> for {}: {e}", stringify!($name)),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
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

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

define_id!(TaskId);
define_id!(NodeId);
define_id!(ScheduledJobId);
define_id!(UserId);
define_id!(RoleId);

macro_rules! impl_partial_schema_for_id {
    ($name:ident) => {
        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                <String as utoipa::PartialSchema>::schema()
            }
        }

        impl utoipa::ToSchema for $name {
            fn name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed("string")
            }
        }
    };
}

impl_partial_schema_for_id!(TaskId);
impl_partial_schema_for_id!(NodeId);
impl_partial_schema_for_id!(ScheduledJobId);
impl_partial_schema_for_id!(UserId);
impl_partial_schema_for_id!(RoleId);
