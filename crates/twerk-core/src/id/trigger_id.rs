use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use super::common::IdError;

/// Validated identifier for a trigger instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Default)]
#[serde(transparent)]
pub struct TriggerId(pub String);

impl<'de> Deserialize<'de> for TriggerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl utoipa::PartialSchema for TriggerId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        <String as utoipa::PartialSchema>::schema()
    }
}

impl utoipa::ToSchema for TriggerId {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("string")
    }
}

impl TriggerId {
    const MIN_LENGTH: usize = 3;
    const MAX_LENGTH: usize = 64;

    pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
        let value = id.into();
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() < Self::MIN_LENGTH {
            return Err(IdError::TooShort(value.len()));
        }
        if value.len() > Self::MAX_LENGTH {
            return Err(IdError::TooLong(value.len()));
        }
        if value
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
        {
            Ok(Self(value))
        } else {
            Err(IdError::InvalidCharacters)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TriggerId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TriggerId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl fmt::Display for TriggerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl AsRef<str> for TriggerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for TriggerId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for TriggerId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for TriggerId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
