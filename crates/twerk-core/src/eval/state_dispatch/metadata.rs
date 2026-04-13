use std::collections::HashMap;

use crate::asl::{State, StateMachine};

use super::StateEvalError;

pub(super) fn attach_state_metadata(state: State, original: &State) -> State {
    let with_comment = apply_state_comment(state, original.comment());
    let with_input_path = apply_state_input_path(with_comment, original.input_path());
    let with_output_path = apply_state_output_path(with_input_path, original.output_path());

    apply_state_assign(with_output_path, original.assign())
}

pub(super) fn attach_machine_metadata(
    machine: StateMachine,
    original: &StateMachine,
) -> StateMachine {
    let with_comment = apply_machine_comment(machine, original.comment());

    apply_machine_timeout(with_comment, original.timeout())
}

pub(super) fn validate_machine(machine: StateMachine) -> Result<StateMachine, StateEvalError> {
    machine
        .validate()
        .map(|()| machine)
        .map_err(StateEvalError::StateMachine)
}

fn apply_state_comment(state: State, comment: Option<&str>) -> State {
    if let Some(value) = comment {
        state.with_comment(value)
    } else {
        state
    }
}

fn apply_state_input_path(state: State, input_path: Option<&crate::asl::JsonPath>) -> State {
    if let Some(value) = input_path {
        state.with_input_path(value.clone())
    } else {
        state
    }
}

fn apply_state_output_path(state: State, output_path: Option<&crate::asl::JsonPath>) -> State {
    if let Some(value) = output_path {
        state.with_output_path(value.clone())
    } else {
        state
    }
}

fn apply_state_assign(
    state: State,
    assign: Option<&HashMap<crate::asl::VariableName, crate::asl::Expression>>,
) -> State {
    if let Some(value) = assign {
        state.with_assign(value.clone())
    } else {
        state
    }
}

fn apply_machine_comment(machine: StateMachine, comment: Option<&str>) -> StateMachine {
    if let Some(value) = comment {
        machine.with_comment(value)
    } else {
        machine
    }
}

fn apply_machine_timeout(machine: StateMachine, timeout: Option<u64>) -> StateMachine {
    if let Some(value) = timeout {
        machine.with_timeout(value)
    } else {
        machine
    }
}
