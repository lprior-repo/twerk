//! `PassState` type for ASL no-op states.
//!
//! A `PassState` optionally injects a result value, then transitions.

use serde::{Deserialize, Serialize};

use super::transition::Transition;

// ---------------------------------------------------------------------------
// PassState
// ---------------------------------------------------------------------------

/// A no-op state that optionally injects a result value, then transitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassState {
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(flatten)]
    transition: Transition,
}

impl PassState {
    #[must_use]
    pub fn new(result: Option<serde_json::Value>, transition: Transition) -> Self {
        Self { result, transition }
    }

    #[must_use]
    pub fn result(&self) -> Option<&serde_json::Value> {
        self.result.as_ref()
    }

    #[must_use]
    pub fn transition(&self) -> &Transition {
        &self.transition
    }
}
