//! StateMachine: the top-level container for an ASL state machine definition.
//!
//! Holds an ordered map of named states with a designated start state.
//! Validation checks inter-state references via `validate()`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::state::{State, StateKind};
use super::transition::Transition;
use super::types::StateName;

// ---------------------------------------------------------------------------
// StateMachineError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateMachineError {
    #[error("states map is empty")]
    EmptyStates,

    #[error("start_at '{start_at}' does not reference a state")]
    StartAtNotFound { start_at: StateName },

    #[error("state '{from}' transitions to '{target}' which does not exist")]
    TransitionTargetNotFound { from: StateName, target: StateName },

    #[error("choice state '{from}' has rule targeting '{target}' which does not exist")]
    ChoiceTargetNotFound { from: StateName, target: StateName },

    #[error("choice state '{from}' has default '{target}' which does not exist")]
    DefaultTargetNotFound { from: StateName, target: StateName },

    #[error("no terminal state found (need at least one Succeed, Fail, or end: true)")]
    NoTerminalState,
}

// ---------------------------------------------------------------------------
// StateMachine
// ---------------------------------------------------------------------------

/// A named, ordered map of states with a designated start state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMachine {
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,

    start_at: StateName,

    states: IndexMap<StateName, State>,

    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
}

impl StateMachine {
    /// Creates a new StateMachine with required fields.
    #[must_use]
    pub fn new(start_at: StateName, states: IndexMap<StateName, State>) -> Self {
        Self {
            comment: None,
            start_at,
            states,
            timeout: None,
        }
    }

    /// Builder: attach a comment.
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Builder: set a timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    #[must_use]
    pub fn start_at(&self) -> &StateName {
        &self.start_at
    }

    #[must_use]
    pub fn states(&self) -> &IndexMap<StateName, State> {
        &self.states
    }

    #[must_use]
    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    /// Validates all 6 invariants (SM-1 through SM-6).
    /// Returns all errors, not just the first.
    pub fn validate(&self) -> Result<(), Vec<StateMachineError>> {
        if self.states.is_empty() {
            return Err(vec![StateMachineError::EmptyStates]);
        }

        let mut errors = Vec::new();

        self.check_start_at(&mut errors);
        self.check_transitions(&mut errors);
        self.check_choice_targets(&mut errors);

        if !self.has_terminal() {
            errors.push(StateMachineError::NoTerminalState);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Looks up a state by name.
    #[must_use]
    pub fn get_state(&self, name: &StateName) -> Option<&State> {
        self.states.get(name)
    }

    /// Returns the start state, if `start_at` references a valid key.
    #[must_use]
    pub fn start_state(&self) -> Option<&State> {
        self.states.get(&self.start_at)
    }

    // --- Private validation sub-functions ---

    fn check_start_at(&self, errors: &mut Vec<StateMachineError>) {
        if !self.states.contains_key(&self.start_at) {
            errors.push(StateMachineError::StartAtNotFound {
                start_at: self.start_at.clone(),
            });
        }
    }

    fn check_transitions(&self, errors: &mut Vec<StateMachineError>) {
        for (name, state) in &self.states {
            if let Some(Transition::Next(target)) = state.kind().transition() {
                if !self.states.contains_key(target) {
                    errors.push(StateMachineError::TransitionTargetNotFound {
                        from: name.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
    }

    fn check_choice_targets(&self, errors: &mut Vec<StateMachineError>) {
        for (name, state) in &self.states {
            if let StateKind::Choice(ref cs) = state.kind() {
                for rule in cs.choices() {
                    if !self.states.contains_key(rule.next()) {
                        errors.push(StateMachineError::ChoiceTargetNotFound {
                            from: name.clone(),
                            target: rule.next().clone(),
                        });
                    }
                }
                if let Some(default) = cs.default() {
                    if !self.states.contains_key(default) {
                        errors.push(StateMachineError::DefaultTargetNotFound {
                            from: name.clone(),
                            target: default.clone(),
                        });
                    }
                }
            }
        }
    }

    fn has_terminal(&self) -> bool {
        self.states.values().any(|state| {
            state.kind().is_terminal() || matches!(state.kind().transition(), Some(Transition::End))
        })
    }
}
