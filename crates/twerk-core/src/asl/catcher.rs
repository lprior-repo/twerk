//! Catcher type for ASL error catch-and-route policies.
//!
//! A Catcher matches specific errors after retries are exhausted
//! and routes execution to a recovery state.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::error_code::ErrorCode;
use super::types::{Expression, JsonPath, StateName, VariableName};

// ---------------------------------------------------------------------------
// CatcherError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatcherError {
    #[error("catcher error_equals must not be empty")]
    EmptyErrorEquals,
}

// ---------------------------------------------------------------------------
// Catcher
// ---------------------------------------------------------------------------

/// A fallback route attached to a state, matching specific errors
/// and routing to a recovery state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "RawCatcher")]
pub struct Catcher {
    error_equals: Vec<ErrorCode>,
    next: StateName,
    result_path: Option<JsonPath>,
    assign: Option<HashMap<VariableName, Expression>>,
}

impl Catcher {
    pub fn new(
        error_equals: Vec<ErrorCode>,
        next: StateName,
        result_path: Option<JsonPath>,
        assign: Option<HashMap<VariableName, Expression>>,
    ) -> Result<Self, CatcherError> {
        if error_equals.is_empty() {
            return Err(CatcherError::EmptyErrorEquals);
        }
        Ok(Self {
            error_equals,
            next,
            result_path,
            assign,
        })
    }

    #[must_use]
    pub fn error_equals(&self) -> &[ErrorCode] {
        &self.error_equals
    }

    #[must_use]
    pub fn next(&self) -> &StateName {
        &self.next
    }

    #[must_use]
    pub fn result_path(&self) -> Option<&JsonPath> {
        self.result_path.as_ref()
    }

    #[must_use]
    pub fn assign(&self) -> Option<&HashMap<VariableName, Expression>> {
        self.assign.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Serde raw helper for deserialization with validation
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCatcher {
    error_equals: Vec<ErrorCode>,
    next: StateName,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_path: Option<JsonPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assign: Option<HashMap<VariableName, Expression>>,
}

impl TryFrom<RawCatcher> for Catcher {
    type Error = CatcherError;

    fn try_from(raw: RawCatcher) -> Result<Self, Self::Error> {
        Self::new(raw.error_equals, raw.next, raw.result_path, raw.assign)
    }
}

impl From<Catcher> for RawCatcher {
    fn from(c: Catcher) -> Self {
        Self {
            error_equals: c.error_equals,
            next: c.next,
            result_path: c.result_path,
            assign: c.assign,
        }
    }
}

impl<'de> Deserialize<'de> for Catcher {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawCatcher::deserialize(deserializer)?;
        Catcher::try_from(raw).map_err(serde::de::Error::custom)
    }
}
