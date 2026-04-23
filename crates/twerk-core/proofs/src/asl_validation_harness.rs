use indexmap::IndexMap;
use twerk_core::asl::machine::StateMachine;
use twerk_core::asl::state::{State, StateKind};
use twerk_core::asl::terminal::SucceedState;
use twerk_core::asl::transition::Transition;
use twerk_core::asl::types::StateName;
use twerk_core::asl::validation::analyze;

/// Helper: build a simple two-state machine (start -> end).
fn simple_machine() -> StateMachine {
    let start_name = StateName::new("Start").unwrap();
    let end_name = StateName::new("End").unwrap();

    let start = State::new(StateKind::Pass(twerk_core::asl::PassState::new(
        None,
        Transition::next(end_name.clone()),
    )));
    let end = State::new(StateKind::Succeed(SucceedState::new()));

    let mut states = IndexMap::new();
    states.insert(start_name.clone(), start);
    states.insert(end_name.clone(), end);

    StateMachine::new(start_name, states)
}

// ---------------------------------------------------------------------------
// Clean machine: no issues
// ---------------------------------------------------------------------------

#[kani::proof]
fn analyze_clean_for_valid_machine() {
    let machine = simple_machine();
    let report = analyze(&machine);
    assert!(report.is_clean(), "Simple valid machine should be clean");
    assert!(
        report.unreachable_states.is_empty(),
        "Should have no unreachable states"
    );
    assert!(report.cycles.is_empty(), "Should have no cycles");
    assert!(
        report.dead_end_states.is_empty(),
        "Should have no dead ends"
    );
}

// ---------------------------------------------------------------------------
// Unreachable state detection
// ---------------------------------------------------------------------------

#[kani::proof]
fn analyze_detects_unreachable() {
    let start_name = StateName::new("Start").unwrap();
    let end_name = StateName::new("End").unwrap();
    let orphan_name = StateName::new("Orphan").unwrap();

    let start = State::new(StateKind::Pass(twerk_core::asl::PassState::new(
        None,
        Transition::next(end_name.clone()),
    )));
    let end = State::new(StateKind::Succeed(SucceedState::new()));
    // Orphan is a terminal state that is never reached from Start
    let orphan = State::new(StateKind::Succeed(SucceedState::new()));

    let mut states = IndexMap::new();
    states.insert(start_name.clone(), start);
    states.insert(end_name.clone(), end);
    states.insert(orphan_name.clone(), orphan);

    let machine = StateMachine::new(start_name, states);
    let report = analyze(&machine);

    assert!(
        !report.is_clean(),
        "Machine with unreachable state should not be clean"
    );
    assert!(
        report
            .unreachable_states
            .iter()
            .any(|s| s.as_str() == "Orphan"),
        "Orphan should be reported as unreachable"
    );
}

// ---------------------------------------------------------------------------
// All states reachable in linear chain
// ---------------------------------------------------------------------------

#[kani::proof]
fn analyze_linear_chain_all_reachable() {
    let a_name = StateName::new("A").unwrap();
    let b_name = StateName::new("B").unwrap();
    let c_name = StateName::new("C").unwrap();

    let a = State::new(StateKind::Pass(twerk_core::asl::PassState::new(
        None,
        Transition::next(b_name.clone()),
    )));
    let b = State::new(StateKind::Pass(twerk_core::asl::PassState::new(
        None,
        Transition::next(c_name.clone()),
    )));
    let c = State::new(StateKind::Succeed(SucceedState::new()));

    let mut states = IndexMap::new();
    states.insert(a_name.clone(), a);
    states.insert(b_name.clone(), b);
    states.insert(c_name.clone(), c);

    let machine = StateMachine::new(a_name, states);
    let report = analyze(&machine);
    assert!(
        report.unreachable_states.is_empty(),
        "All states in linear chain should be reachable"
    );
}

// ---------------------------------------------------------------------------
// Single-state machine (start is terminal)
// ---------------------------------------------------------------------------

#[kani::proof]
fn analyze_single_succeed_state() {
    let name = StateName::new("Done").unwrap();
    let state = State::new(StateKind::Succeed(SucceedState::new()));

    let mut states = IndexMap::new();
    states.insert(name.clone(), state);

    let machine = StateMachine::new(name, states);
    let report = analyze(&machine);
    assert!(report.is_clean(), "Single Succeed state machine should be clean");
}
