//! Terminal state types for ASL: SucceedState and FailState.
//!
//! Terminal states have no outgoing transition — execution ends here.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

// ---------------------------------------------------------------------------
// SucceedState
// ---------------------------------------------------------------------------

/// Terminal state indicating successful execution. No fields, no transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SucceedState;

impl SucceedState {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Serialize for SucceedState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for SucceedState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SucceedVisitor;

        impl<'de> Visitor<'de> for SucceedVisitor {
            type Value = SucceedState;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an empty map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                // Drain any unknown fields
                while map.next_key::<de::IgnoredAny>().ok().flatten().is_some() {
                    let _: de::IgnoredAny = map.next_value()?;
                }
                Ok(SucceedState)
            }
        }

        deserializer.deserialize_map(SucceedVisitor)
    }
}

// ---------------------------------------------------------------------------
// FailState
// ---------------------------------------------------------------------------

/// Terminal state indicating failed execution with optional error and cause.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailState {
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<String>,
}

impl FailState {
    #[must_use]
    pub fn new(error: Option<String>, cause: Option<String>) -> Self {
        Self { error, cause }
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[must_use]
    pub fn cause(&self) -> Option<&str> {
        self.cause.as_deref()
    }
}

impl fmt::Display for FailState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.error, &self.cause) {
            (Some(e), Some(c)) => write!(f, "FAIL: {e} ({c})"),
            (Some(e), None) => write!(f, "FAIL: {e}"),
            (None, Some(c)) => write!(f, "FAIL: ({c})"),
            (None, None) => f.write_str("FAIL"),
        }
    }
}
