//! ParallelState: concurrent branch execution.
//!
//! Runs multiple independent branches concurrently, with optional fail-fast.
//! Enforces INV-PS1 (non-empty branches) at construction time.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::machine::StateMachine;
use super::transition::Transition;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParallelStateError {
    #[error("parallel state must have at least one branch")]
    EmptyBranches,
}

// ---------------------------------------------------------------------------
// ParallelState
// ---------------------------------------------------------------------------

/// Runs multiple independent branches concurrently.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "RawParallelState")]
pub struct ParallelState {
    branches: Vec<StateMachine>,
    transition: Transition,
    fail_fast: Option<bool>,
}

impl ParallelState {
    /// Validated constructor.
    ///
    /// Returns `Err(EmptyBranches)` if `branches` is empty (INV-PS1).
    pub fn new(
        branches: Vec<StateMachine>,
        transition: Transition,
        fail_fast: Option<bool>,
    ) -> Result<Self, ParallelStateError> {
        if branches.is_empty() {
            return Err(ParallelStateError::EmptyBranches);
        }
        Ok(Self {
            branches,
            transition,
            fail_fast,
        })
    }

    #[must_use]
    pub fn branches(&self) -> &[StateMachine] {
        &self.branches
    }

    #[must_use]
    pub fn transition(&self) -> &Transition {
        &self.transition
    }

    #[must_use]
    pub fn fail_fast(&self) -> Option<bool> {
        self.fail_fast
    }
}

// ---------------------------------------------------------------------------
// Serde raw helper for deserialization with validation
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawParallelState {
    branches: Vec<StateMachine>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    fail_fast: Option<bool>,
    #[serde(flatten)]
    transition: Transition,
}

impl TryFrom<RawParallelState> for ParallelState {
    type Error = ParallelStateError;

    fn try_from(raw: RawParallelState) -> Result<Self, Self::Error> {
        Self::new(raw.branches, raw.transition, raw.fail_fast)
    }
}

impl From<ParallelState> for RawParallelState {
    fn from(ps: ParallelState) -> Self {
        Self {
            branches: ps.branches,
            fail_fast: ps.fail_fast,
            transition: ps.transition,
        }
    }
}

impl<'de> Deserialize<'de> for ParallelState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawParallelState::deserialize(deserializer)?;
        ParallelState::try_from(raw).map_err(serde::de::Error::custom)
    }
}
