use indexmap::IndexMap;
use twerk_core::asl::machine::{StateMachine, StateMachineError};
use twerk_core::asl::state::{State, StateKind};
use twerk_core::asl::terminal::{FailState, SucceedState};
use twerk_core::asl::transition::Transition;
use twerk_core::asl::types::StateName;

fn make_pass_state() -> StateKind {
    let transition = Transition::end();
    let pass = twerk_core::asl::pass::PassState::new(None, transition);
    StateKind::Pass(pass)
}

fn make_succeed_state() -> StateKind {
    StateKind::Succeed(SucceedState::new())
}

fn make_fail_state() -> StateKind {
    let fail = FailState::new(Some("error".to_string()), None);
    StateKind::Fail(fail)
}

fn make_simple_machine(start: &str, states: Vec<(&str, StateKind)>) -> StateMachine {
    let start_name = StateName::new(start).unwrap();
    let mut state_map = IndexMap::new();

    for (name, kind) in states {
        let sn = StateName::new(name).unwrap();
        let state = State::new(kind);
        state_map.insert(sn, state);
    }

    StateMachine::new(start_name, state_map)
}

#[kani::proof]
fn state_machine_rejects_empty_states() {
    let start = StateName::new("Start").unwrap();
    let empty_states = IndexMap::new();

    let machine = StateMachine::new(start, empty_states);
    let result = machine.validate();

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, StateMachineError::EmptyStates)));
}

#[kani::proof]
fn state_machine_rejects_missing_start_state() {
    let start_name = StateName::new("NonExistent").unwrap();
    let mut states = IndexMap::new();
    states.insert(
        StateName::new("ActualStart").unwrap(),
        State::new(make_succeed_state()),
    );

    let machine = StateMachine::new(start_name, states);
    let result = machine.validate();

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, StateMachineError::StartAtNotFound { .. })));
}

#[kani::proof]
fn state_machine_valid_with_terminal_state() {
    let machine = make_simple_machine(
        "Start",
        vec![("Start", make_pass_state()), ("End", make_succeed_state())],
    );

    let result = machine.validate();
    assert!(
        result.is_ok(),
        "Machine with terminal state should be valid: {:?}",
        result
    );
}

#[kani::proof]
fn state_machine_invalid_without_terminal_state() {
    let machine = make_simple_machine(
        "Start",
        vec![("Start", make_pass_state()), ("Middle", make_pass_state())],
    );

    let result = machine.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, StateMachineError::NoTerminalState)));
}

#[kani::proof]
fn state_machine_choice_rule_target_validation() {
    let choice_state = twerk_core::asl::choice::ChoiceState::new(
        vec![twerk_core::asl::choice::ChoiceRule::new(
            twerk_core::asl::types::Expression::new("$.value > 0").unwrap(),
            StateName::new("ValidTarget").unwrap(),
            None,
        )],
        Some(StateName::new("AlsoValid").unwrap()),
    )
    .unwrap();

    let machine = make_simple_machine(
        "Start",
        vec![
            ("Start", make_pass_state()),
            ("ValidTarget", make_succeed_state()),
            ("AlsoValid", make_succeed_state()),
            ("Choice", StateKind::Choice(choice_state)),
        ],
    );

    let result = machine.validate();
    assert!(
        result.is_ok(),
        "Machine with valid choice targets should be valid: {:?}",
        result
    );
}

#[kani::proof]
fn state_kind_is_terminal() {
    assert!(StateKind::Succeed(SucceedState::new()).is_terminal());

    let fail = FailState::new(Some("error".to_string()), None);
    assert!(StateKind::Fail(fail).is_terminal());

    assert!(!make_pass_state().is_terminal());
    assert!(!make_fail_state().is_terminal());
}

#[kani::proof]
fn state_machine_serialize_deserialize_roundtrip() {
    let machine = make_simple_machine(
        "Start",
        vec![("Start", make_pass_state()), ("End", make_succeed_state())],
    );

    let serialized = serde_json::to_string(&machine).unwrap();
    let deserialized: StateMachine = serde_json::from_str(&serialized).unwrap();

    assert_eq!(machine.start_at(), deserialized.start_at());
    assert_eq!(machine.states().len(), deserialized.states().len());
}

#[kani::proof]
fn state_machine_valid_with_fail_terminal_state() {
    let machine = make_simple_machine(
        "Start",
        vec![("Start", make_pass_state()), ("End", make_fail_state())],
    );

    let result = machine.validate();
    assert!(
        result.is_ok(),
        "Machine with fail terminal state should be valid: {:?}",
        result
    );
}
