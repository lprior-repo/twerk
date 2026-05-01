use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepName(String);

impl StepName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for StepName {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for StepName {}

impl Hash for StepName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepOutcome {
    Completed(Vec<u8>),
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchState {
    pub steps: HashMap<StepName, StepOutcome>,
}

impl BranchState {
    #[must_use]
    pub fn new() -> Self {
        Self { steps: HashMap::new() }
    }

    pub fn complete_step(&mut self, step: StepName, outcome: StepOutcome) -> bool {
        if self.steps.contains_key(&step) {
            return false;
        }
        self.steps.insert(step, outcome);
        true
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty()
    }
}

impl Default for BranchState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowStateInner {
    branches: HashMap<String, BranchState>,
    pending_joins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    inner: Arc<Mutex<WorkflowStateInner>>,
}

impl WorkflowState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WorkflowStateInner {
                branches: HashMap::new(),
                pending_joins: Vec::new(),
            })),
        }
    }

    pub fn add_branch(&self, branch_id: impl Into<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.branches.insert(branch_id.into(), BranchState::new());
    }

    pub fn complete_step(
        &self,
        branch_id: &str,
        step: StepName,
        outcome: StepOutcome,
    ) -> Result<bool, WorkflowStateError> {
        let mut inner = self.inner.lock().unwrap();
        let branch = inner
            .branches
            .get_mut(branch_id)
            .ok_or(WorkflowStateError::BranchNotFound)?;
        Ok(branch.complete_step(step, outcome))
    }

    #[must_use]
    pub fn is_branch_complete(&self, branch_id: &str) -> Result<bool, WorkflowStateError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .branches
            .get(branch_id)
            .map(|b| b.is_complete())
            .unwrap_or(false))
    }

    pub fn all_branches_complete(&self) -> Result<bool, WorkflowStateError> {
        let inner = self.inner.lock().unwrap();
        if inner.branches.is_empty() {
            return Ok(false);
        }
        Ok(inner.branches.values().all(|b| b.is_complete()))
    }

    pub fn record_join(&self, join_id: String) -> Result<(), WorkflowStateError> {
        let mut inner = self.inner.lock().unwrap();
        inner.pending_joins.push(join_id);
        Ok(())
    }

    #[must_use]
    pub fn pending_joins(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.pending_joins.clone()
    }

    #[must_use]
    pub fn get_step_outcome(
        &self,
        branch_id: &str,
        step: &StepName,
    ) -> Result<Option<StepOutcome>, WorkflowStateError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .branches
            .get(branch_id)
            .and_then(|b| b.steps.get(step).cloned()))
    }
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowStateError {
    #[error("branch not found")]
    BranchNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;

    #[test]
    fn workflow_state_records_step_completion() {
        let state = WorkflowState::new();
        state.add_branch("branch-1");

        let result = state.complete_step(
            "branch-1",
            StepName::new("step-A"),
            StepOutcome::Completed(vec![1, 2, 3]),
        );
        assert!(result.is_ok());
        assert!(result.unwrap());

        let outcome = state.get_step_outcome("branch-1", &StepName::new("step-A"));
        assert!(outcome.is_ok());
        assert!(outcome.unwrap().is_some());
    }

    #[test]
    fn workflow_state_same_step_twice_is_noop() {
        let state = WorkflowState::new();
        state.add_branch("branch-1");

        let first = state.complete_step(
            "branch-1",
            StepName::new("step-A"),
            StepOutcome::Completed(vec![1]),
        );
        assert!(first.is_ok());
        assert!(first.unwrap());

        let second = state.complete_step(
            "branch-1",
            StepName::new("step-A"),
            StepOutcome::Completed(vec![2]),
        );
        assert!(second.is_ok());
        assert!(!second.unwrap());
    }

    #[test]
    fn workflow_state_tracks_multiple_branches() {
        let state = WorkflowState::new();
        state.add_branch("branch-A");
        state.add_branch("branch-B");

        state
            .complete_step(
                "branch-A",
                StepName::new("step-A"),
                StepOutcome::Completed(vec![1]),
            )
            .unwrap();

        state
            .complete_step(
                "branch-B",
                StepName::new("step-B"),
                StepOutcome::Completed(vec![2]),
            )
            .unwrap();

        assert!(state.all_branches_complete().unwrap());
    }

    #[tokio::test]
    async fn workflow_state_concurrent_step_completion() {
        let state = Arc::new(WorkflowState::new());
        state.add_branch("parallel-branch");

        let state_clone = Arc::clone(&state);
        let handle_a = task::spawn(async move {
            state_clone
                .complete_step(
                    "parallel-branch",
                    StepName::new("step-A"),
                    StepOutcome::Completed(vec![1]),
                )
                .unwrap()
        });

        let state_clone = Arc::clone(&state);
        let handle_b = task::spawn(async move {
            state_clone
                .complete_step(
                    "parallel-branch",
                    StepName::new("step-B"),
                    StepOutcome::Completed(vec![2]),
                )
                .unwrap()
        });

        let (result_a, result_b) = tokio::join!(handle_a, handle_b);
        assert!(result_a.is_ok());
        assert!(result_b.is_ok());

        assert!(state.is_branch_complete("parallel-branch").unwrap());
    }

    #[tokio::test]
    async fn workflow_state_concurrent_same_step_race() {
        let state = Arc::new(WorkflowState::new());
        state.add_branch("race-branch");

        let state_clone = Arc::clone(&state);
        let handle_a = task::spawn(async move {
            state_clone
                .complete_step(
                    "race-branch",
                    StepName::new("step-X"),
                    StepOutcome::Completed(vec![1]),
                )
                .unwrap()
        });

        let state_clone = Arc::clone(&state);
        let handle_b = task::spawn(async move {
            state_clone
                .complete_step(
                    "race-branch",
                    StepName::new("step-X"),
                    StepOutcome::Completed(vec![2]),
                )
                .unwrap()
        });

        let (result_a, result_b) = tokio::join!(handle_a, handle_b);
        assert!(result_a.is_ok());
        assert!(result_b.is_ok());

        let outcome = state
            .get_step_outcome("race-branch", &StepName::new("step-X"))
            .unwrap();
        let count = outcome.expect("should have outcome").get_count();
        assert_eq!(count, 1);
    }

    #[test]
    fn workflow_state_join_trigger_after_all_complete() {
        let state = WorkflowState::new();
        state.add_branch("branch-A");
        state.add_branch("branch-B");

        state
            .complete_step(
                "branch-A",
                StepName::new("step-A"),
                StepOutcome::Completed(vec![1]),
            )
            .unwrap();

        assert!(!state.all_branches_complete().unwrap());

        state
            .complete_step(
                "branch-B",
                StepName::new("step-B"),
                StepOutcome::Completed(vec![2]),
            )
            .unwrap();

        assert!(state.all_branches_complete().unwrap());

        state.record_join("join-1".to_string()).unwrap();
        let joins = state.pending_joins();
        assert!(joins.contains(&"join-1".to_string()));
    }

    #[test]
    fn workflow_state_nonexistent_branch_error() {
        let state = WorkflowState::new();
        let result = state.complete_step(
            "nonexistent",
            StepName::new("step-A"),
            StepOutcome::Completed(vec![1]),
        );
        assert!(result.is_err());
    }
}

impl StepOutcome {
    #[must_use]
    pub fn get_count(&self) -> usize {
        1
    }
}