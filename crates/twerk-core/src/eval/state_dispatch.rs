use std::collections::HashMap;

use indexmap::IndexMap;
use serde_json::Value;
use thiserror::Error;

use crate::asl::{
    ChoiceStateError, MapStateError, ParallelStateError, State, StateKind, StateMachine,
    StateMachineError, SwitchStateError, TaskStateError,
};

mod arms;
mod metadata;
#[cfg(test)]
mod tests;

use arms::{build_choice_arm, dispatch_task_from_state, eval_map_arm, eval_parallel_arm};
use metadata::{attach_machine_metadata, attach_state_metadata, validate_machine};

#[derive(Debug, Error, PartialEq)]
pub enum StateEvalError {
    #[error("task state invariant violated after ASL dispatch: {0}")]
    TaskState(#[from] TaskStateError),

    #[error("choice state invariant violated after ASL dispatch: {0}")]
    ChoiceState(#[from] ChoiceStateError),

    #[error("parallel state invariant violated after ASL dispatch: {0}")]
    ParallelState(#[from] ParallelStateError),

    #[error("map state invariant violated after ASL dispatch: {0}")]
    MapState(#[from] MapStateError),

    #[error("switch state invariant violated after ASL dispatch: {0}")]
    SwitchState(#[from] SwitchStateError),

    #[error("state machine invalid after recursive ASL dispatch: {0:?}")]
    StateMachine(Vec<StateMachineError>),
}

#[allow(clippy::implicit_hasher)]
pub fn evaluate_state(
    state: &State,
    context: &HashMap<String, Value>,
) -> Result<State, StateEvalError> {
    evaluate_state_kind(state.kind(), context)
        .map(State::new)
        .map(|evaluated_state| attach_state_metadata(evaluated_state, state))
}

#[allow(clippy::implicit_hasher)]
pub fn evaluate_state_machine(
    machine: &StateMachine,
    context: &HashMap<String, Value>,
) -> Result<StateMachine, StateEvalError> {
    machine
        .states()
        .iter()
        .map(|(name, state)| {
            evaluate_state(state, context).map(|evaluated_state| (name.clone(), evaluated_state))
        })
        .collect::<Result<IndexMap<_, _>, _>>()
        .map(|states| StateMachine::new(machine.start_at().clone(), states))
        .map(|evaluated_machine| attach_machine_metadata(evaluated_machine, machine))
        .and_then(validate_machine)
}

fn evaluate_state_kind(
    kind: &StateKind,
    context: &HashMap<String, Value>,
) -> Result<StateKind, StateEvalError> {
    match kind {
        StateKind::Task(task) => dispatch_task_from_state(task),
        StateKind::Pass(pass) => Ok(StateKind::Pass(pass.clone())),
        StateKind::Choice(choice) => {
            build_choice_arm(choice.choices().to_vec(), choice.default().cloned())
        }
        StateKind::Switch(switch) => {
            use crate::asl::SwitchState;
            SwitchState::new(switch.cases().to_vec(), switch.default().cloned())
                .map(StateKind::Switch)
                .map_err(StateEvalError::from)
        }
        StateKind::Wait(wait) => Ok(StateKind::Wait(wait.clone())),
        StateKind::Succeed(succeed) => Ok(StateKind::Succeed(*succeed)),
        StateKind::Fail(fail) => Ok(StateKind::Fail(fail.clone())),
        StateKind::Parallel(parallel) => eval_parallel_arm(parallel, context),
        StateKind::Map(map) => eval_map_arm(map, context),
    }
}
