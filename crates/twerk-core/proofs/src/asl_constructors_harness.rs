use indexmap::IndexMap;
use twerk_core::asl::catcher::{Catcher, CatcherError};
use twerk_core::asl::error_code::ErrorCode;
use twerk_core::asl::machine::StateMachine;
use twerk_core::asl::map::{MapState, MapStateError};
use twerk_core::asl::parallel::{ParallelState, ParallelStateError};
use twerk_core::asl::state::{State, StateKind};
use twerk_core::asl::terminal::SucceedState;
use twerk_core::asl::transition::Transition;
use twerk_core::asl::types::{Expression, StateName};

/// Helper: create a minimal valid StateMachine for use as item_processor / branch.
fn minimal_machine() -> StateMachine {
    let name = StateName::new("Done").unwrap();
    let state = State::new(StateKind::Succeed(SucceedState::new()));
    let mut states = IndexMap::new();
    states.insert(name.clone(), state);
    StateMachine::new(name, states)
}

// ===========================================================================
// MapState
// ===========================================================================

fn valid_map_params() -> (
    Expression,
    Box<StateMachine>,
    Option<u32>,
    Transition,
    Vec<twerk_core::asl::Retrier>,
    Vec<Catcher>,
    Option<f64>,
) {
    let items_path = Expression::new("$.items").unwrap();
    let processor = Box::new(minimal_machine());
    let transition = Transition::next(StateName::new("Next").unwrap());
    (items_path, processor, None, transition, vec![], vec![], None)
}

#[kani::proof]
fn map_state_accepts_none_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, tolerated_failure_percentage) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        tolerated_failure_percentage,
    );
    assert!(result.is_ok(), "None tolerance should be accepted");
}

#[kani::proof]
fn map_state_accepts_valid_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(50.0),
    );
    assert!(result.is_ok(), "50% tolerance should be accepted");
}

#[kani::proof]
fn map_state_accepts_zero_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(0.0),
    );
    assert!(result.is_ok(), "0% tolerance should be accepted");
}

#[kani::proof]
fn map_state_accepts_100_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(100.0),
    );
    assert!(result.is_ok(), "100% tolerance should be accepted");
}

#[kani::proof]
fn map_state_rejects_nan_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(f64::NAN),
    );
    assert!(result.is_err(), "NaN tolerance should be rejected");
    match result {
        Err(MapStateError::NonFiniteToleratedFailurePercentage) => {}
        other => panic!("Expected NonFiniteToleratedFailurePercentage, got {:?}", other),
    }
}

#[kani::proof]
fn map_state_rejects_infinity_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(f64::INFINITY),
    );
    assert!(result.is_err(), "Infinity tolerance should be rejected");
}

#[kani::proof]
fn map_state_rejects_negative_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(-1.0),
    );
    assert!(result.is_err(), "Negative tolerance should be rejected");
}

#[kani::proof]
fn map_state_rejects_over_100_tolerance() {
    let (items_path, processor, max_concurrency, transition, retry, catchers, _) =
        valid_map_params();
    let result = MapState::new(
        items_path,
        processor,
        max_concurrency,
        transition,
        retry,
        catchers,
        Some(100.1),
    );
    assert!(result.is_err(), "100.1% tolerance should be rejected");
    match result {
        Err(MapStateError::InvalidToleratedFailurePercentage(pct)) => {
            assert!((pct - 100.1).abs() < f64::EPSILON);
        }
        other => panic!(
            "Expected InvalidToleratedFailurePercentage, got {:?}",
            other
        ),
    }
}

// ===========================================================================
// Catcher
// ===========================================================================

#[kani::proof]
fn catcher_accepts_non_empty_error_equals() {
    let result = Catcher::new(
        vec![ErrorCode::Timeout],
        StateName::new("Fallback").unwrap(),
        None,
        None,
    );
    assert!(result.is_ok(), "Non-empty error_equals should be accepted");
}

#[kani::proof]
fn catcher_rejects_empty_error_equals() {
    let result = Catcher::new(
        vec![],
        StateName::new("Fallback").unwrap(),
        None,
        None,
    );
    assert!(result.is_err(), "Empty error_equals should be rejected");
    match result {
        Err(CatcherError::EmptyErrorEquals) => {}
        other => panic!("Expected EmptyErrorEquals, got {:?}", other),
    }
}

#[kani::proof]
fn catcher_accepts_multiple_error_equals() {
    let result = Catcher::new(
        vec![ErrorCode::Timeout, ErrorCode::TaskFailed],
        StateName::new("Fallback").unwrap(),
        None,
        None,
    );
    assert!(
        result.is_ok(),
        "Multiple error_equals should be accepted"
    );
}

// ===========================================================================
// ParallelState
// ===========================================================================

#[kani::proof]
fn parallel_accepts_non_empty_branches() {
    let branch = minimal_machine();
    let transition = Transition::next(StateName::new("Next").unwrap());
    let result = ParallelState::new(vec![branch], transition, None);
    assert!(result.is_ok(), "Non-empty branches should be accepted");
}

#[kani::proof]
fn parallel_rejects_empty_branches() {
    let transition = Transition::next(StateName::new("Next").unwrap());
    let result = ParallelState::new(vec![], transition, None);
    assert!(result.is_err(), "Empty branches should be rejected");
    match result {
        Err(ParallelStateError::EmptyBranches) => {}
        other => panic!("Expected EmptyBranches, got {:?}", other),
    }
}

#[kani::proof]
fn parallel_accepts_multiple_branches() {
    let branch1 = minimal_machine();
    let branch2 = minimal_machine();
    let transition = Transition::next(StateName::new("Next").unwrap());
    let result = ParallelState::new(vec![branch1, branch2], transition, None);
    assert!(result.is_ok(), "Multiple branches should be accepted");
}
