//! StateKind enum and State wrapper for ASL state machine states.
//!
//! StateKind is the 8-variant discriminated union replacing the Go Task god object.
//! State wraps StateKind with shared fields (comment, input_path, output_path, assign).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::choice::ChoiceState;
use super::map::MapState;
use super::parallel::ParallelState;
use super::pass::PassState;
use super::switch::SwitchState;
use super::task_state::TaskState;
use super::terminal::{FailState, SucceedState};
use super::transition::Transition;
use super::types::{Expression, JsonPath, VariableName};
use super::wait::WaitState;

// ---------------------------------------------------------------------------
// StateKind
// ---------------------------------------------------------------------------

/// The 9-variant discriminated union for ASL state types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StateKind {
    Task(TaskState),
    Pass(PassState),
    Choice(ChoiceState),
    Switch(SwitchState),
    Wait(WaitState),
    Succeed(SucceedState),
    Fail(FailState),
    Parallel(ParallelState),
    Map(MapState),
}

impl StateKind {
    /// Returns `true` for `Succeed` and `Fail`; `false` for all others.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeed(_) | Self::Fail(_))
    }

    /// Returns the `Transition` for states that have one.
    /// `None` for `Choice` and `Switch` (uses cases/default) and terminals (`Succeed`, `Fail`).
    #[must_use]
    pub fn transition(&self) -> Option<&Transition> {
        match self {
            Self::Task(s) => Some(s.transition()),
            Self::Pass(s) => Some(s.transition()),
            Self::Wait(s) => Some(s.transition()),
            Self::Parallel(s) => Some(s.transition()),
            Self::Map(s) => Some(s.transition()),
            Self::Choice(_) | Self::Switch(_) | Self::Succeed(_) | Self::Fail(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Wrapper combining shared fields with a StateKind variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    input_path: Option<JsonPath>,

    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<JsonPath>,

    #[serde(skip_serializing_if = "Option::is_none")]
    assign: Option<HashMap<VariableName, Expression>>,

    #[serde(flatten)]
    kind: StateKind,
}

impl State {
    /// Creates a new State with the given kind and no optional fields.
    #[must_use]
    pub fn new(kind: StateKind) -> Self {
        Self {
            comment: None,
            input_path: None,
            output_path: None,
            assign: None,
            kind,
        }
    }

    /// Builder: attach a comment.
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Builder: attach an input path.
    #[must_use]
    pub fn with_input_path(mut self, path: JsonPath) -> Self {
        self.input_path = Some(path);
        self
    }

    /// Builder: attach an output path.
    #[must_use]
    pub fn with_output_path(mut self, path: JsonPath) -> Self {
        self.output_path = Some(path);
        self
    }

    /// Builder: attach assignments.
    #[must_use]
    pub fn with_assign(mut self, assign: HashMap<VariableName, Expression>) -> Self {
        self.assign = Some(assign);
        self
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    #[must_use]
    pub fn input_path(&self) -> Option<&JsonPath> {
        self.input_path.as_ref()
    }

    #[must_use]
    pub fn output_path(&self) -> Option<&JsonPath> {
        self.output_path.as_ref()
    }

    #[must_use]
    pub fn assign(&self) -> Option<&HashMap<VariableName, Expression>> {
        self.assign.as_ref()
    }

    #[must_use]
    pub fn kind(&self) -> &StateKind {
        &self.kind
    }
}
