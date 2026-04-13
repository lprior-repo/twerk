#![allow(unused_imports)]

use super::*;

fn production_state_dispatch_source() -> &'static str {
    include_str!("../../state_dispatch.rs")
}

#[test]
fn public_dispatch_signatures_and_error_surface_never_reference_legacy_task_types() -> TestResult {
    fn assert_state_signature(
        _: fn(&State, &HashMap<String, Value>) -> Result<State, StateEvalError>,
    ) {
    }
    fn assert_machine_signature(
        _: fn(&StateMachine, &HashMap<String, Value>) -> Result<StateMachine, StateEvalError>,
    ) {
    }
    fn assert_kind_signature(
        _: fn(&StateKind, &HashMap<String, Value>) -> Result<StateKind, StateEvalError>,
    ) {
    }

    assert_state_signature(evaluate_state);
    assert_machine_signature(evaluate_state_machine);
    assert_kind_signature(evaluate_state_kind);
    let source = production_state_dispatch_source();
    assert!(!source.contains("crate::task::Task"));
    assert!(!source.contains("task::Task"));
    Ok(())
}

#[test]
fn evaluate_state_kind_matches_closed_union_without_unsupported_fallback() -> TestResult {
    let source = production_state_dispatch_source();

    assert!(source.contains("fn evaluate_state_kind("));
    assert!(source.contains("StateKind::Task(task) => dispatch_task_from_state(task)"));
    assert!(source.contains("StateKind::Pass(pass) => Ok(StateKind::Pass(pass.clone()))"));
    assert!(source.contains("StateKind::Choice(choice) =>"));
    assert!(source.contains("StateKind::Wait(wait) => Ok(StateKind::Wait(wait.clone()))"));
    assert!(source.contains("StateKind::Succeed(succeed) => Ok(StateKind::Succeed(*succeed))"));
    assert!(source.contains("StateKind::Fail(fail) => Ok(StateKind::Fail(fail.clone()))"));
    assert!(
        source.contains("StateKind::Parallel(parallel) => eval_parallel_arm(parallel, context)")
    );
    assert!(source.contains("StateKind::Map(map) => eval_map_arm(map, context)"));
    assert!(!source.contains("UnsupportedStateKind"));
    assert!(!source.contains("_ =>"));
    Ok(())
}
