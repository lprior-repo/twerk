//! Twerk Engine - State and Mode enums

use crate::engine::types::Config;
use std::sync::Arc;
use twerk_core::job::Job;

/// Engine execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Coordinator,
    Worker,
    #[default]
    Standalone,
}

/// Engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Idle,
    Running,
    Terminating,
    Terminated,
}

impl State {
    /// Returns true if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, State::Terminated)
    }

    /// Returns true if a transition to the given state is allowed
    pub fn can_transition_to(&self, new: State) -> bool {
        match (self, new) {
            (State::Idle, _) => true,
            (State::Running, State::Terminating) => true,
            (State::Terminating, State::Terminated) => true,
            _ => false,
        }
    }
}
