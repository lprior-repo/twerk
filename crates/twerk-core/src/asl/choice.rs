//! ChoiceRule and ChoiceState types for ASL branching logic.
//!
//! A ChoiceState evaluates rules against input and routes to the first
//! matching rule's target. Validation ensures at least one rule exists.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::{Expression, StateName, VariableName};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChoiceStateError {
    #[error("choice state must have at least one choice rule")]
    EmptyChoices,
}

// ---------------------------------------------------------------------------
// ChoiceRule
// ---------------------------------------------------------------------------

/// A single branch in a Choice state — pairs a boolean condition with a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceRule {
    condition: Expression,
    next: StateName,
    #[serde(skip_serializing_if = "Option::is_none")]
    assign: Option<HashMap<VariableName, Expression>>,
}

impl ChoiceRule {
    #[must_use]
    pub fn new(
        condition: Expression,
        next: StateName,
        assign: Option<HashMap<VariableName, Expression>>,
    ) -> Self {
        Self {
            condition,
            next,
            assign,
        }
    }

    #[must_use]
    pub fn condition(&self) -> &Expression {
        &self.condition
    }

    #[must_use]
    pub fn next(&self) -> &StateName {
        &self.next
    }

    #[must_use]
    pub fn assign(&self) -> Option<&HashMap<VariableName, Expression>> {
        self.assign.as_ref()
    }
}

// ---------------------------------------------------------------------------
// ChoiceState
// ---------------------------------------------------------------------------

/// Branching state that evaluates rules and routes to the first match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceState {
    choices: Vec<ChoiceRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<StateName>,
}

impl ChoiceState {
    pub fn new(
        choices: Vec<ChoiceRule>,
        default: Option<StateName>,
    ) -> Result<Self, ChoiceStateError> {
        if choices.is_empty() {
            return Err(ChoiceStateError::EmptyChoices);
        }
        Ok(Self { choices, default })
    }

    #[must_use]
    pub fn choices(&self) -> &[ChoiceRule] {
        &self.choices
    }

    #[must_use]
    pub fn default(&self) -> Option<&StateName> {
        self.default.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Serde: try_from for deserialization validation
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChoiceState {
    choices: Vec<ChoiceRule>,
    default: Option<StateName>,
}

impl TryFrom<RawChoiceState> for ChoiceState {
    type Error = ChoiceStateError;

    fn try_from(raw: RawChoiceState) -> Result<Self, Self::Error> {
        Self::new(raw.choices, raw.default)
    }
}

impl<'de> Deserialize<'de> for ChoiceState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawChoiceState::deserialize(deserializer)?;
        ChoiceState::try_from(raw).map_err(serde::de::Error::custom)
    }
}
