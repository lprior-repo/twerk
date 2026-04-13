use super::fixtures::*;
use super::machine_fixtures::*;
use super::*;
use crate::asl::StateMachineError;
use rstest::rstest;

#[test]
fn evaluate_state_machine_returns_empty_states_error_when_machine_is_empty() -> TestResult {
    let machine = StateMachine::new(sn("Init")?, IndexMap::new());
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::EmptyStates
        ]))
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_start_at_not_found_when_input_machine_is_invalid() -> TestResult {
    let machine = machine_missing_start_at()?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::StartAtNotFound {
                start_at: sn("Missing")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_transition_target_not_found_when_input_machine_is_invalid(
) -> TestResult {
    let machine = machine_missing_transition_target()?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::TransitionTargetNotFound {
                from: sn("Init")?,
                target: sn("Ghost")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_choice_target_not_found_when_choice_rule_target_is_invalid(
) -> TestResult {
    let machine = machine_missing_choice_target()?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::ChoiceTargetNotFound {
                from: sn("Choose")?,
                target: sn("Ghost")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_default_target_not_found_when_choice_default_target_is_invalid(
) -> TestResult {
    let machine = machine_missing_default_target()?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::DefaultTargetNotFound {
                from: sn("Choose")?,
                target: sn("Ghost")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_no_terminal_state_when_machine_has_no_terminal_state(
) -> TestResult {
    let machine = machine_without_terminal_state()?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::NoTerminalState
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_minimum_valid_machine_when_single_terminal_state_is_present(
) -> TestResult {
    assert_machine_dispatch(terminal_machine("Done")?);
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_machine_comment_when_comment_is_present() -> TestResult {
    let machine = terminal_machine("Done")?.with_comment("dispatch this machine");
    let result = evaluate_state_machine(&machine, &context())?;
    assert_eq!(result.comment(), Some("dispatch this machine"));
    assert_eq!(result, machine);
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_dense_all_variant_machine_when_timeout_is_u64_max() -> TestResult
{
    assert_machine_dispatch(dense_all_variant_machine()?);
    Ok(())
}

#[rstest]
#[case(None)]
#[case(Some(0))]
#[case(Some(1))]
#[case(Some(u64::MAX))]
fn evaluate_state_machine_preserves_timeout_when_machine_timeout_is_valid(
    #[case] timeout: Option<u64>,
) -> TestResult {
    let machine = terminal_machine("Done")?;
    let machine = if let Some(timeout) = timeout {
        machine.with_timeout(timeout)
    } else {
        machine
    };
    assert_machine_dispatch(machine);
    Ok(())
}

#[test]
fn evaluate_state_machine_recurses_into_parallel_branches_when_parallel_states_are_present(
) -> TestResult {
    let machine = machine(
        "Fork",
        [
            (
                "Fork",
                State::new(StateKind::Parallel(valid_parallel_state(next_t("Done")?)?)),
            ),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )?;
    assert_machine_dispatch(machine);
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_state_machine_error_when_parallel_branch_machine_is_invalid(
) -> TestResult {
    let machine = machine(
        "Fork",
        [
            (
                "Fork",
                State::new(StateKind::Parallel(parallel_state_with_invalid_branch(
                    next_t("Done")?,
                )?)),
            ),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::StartAtNotFound {
                start_at: sn("Missing")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_recurses_into_map_item_processor_when_map_states_are_present(
) -> TestResult {
    let machine = machine(
        "MapIt",
        [
            (
                "MapIt",
                State::new(StateKind::Map(valid_map_state(
                    Some(25.0),
                    next_t("Done")?,
                )?)),
            ),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )?;
    assert_machine_dispatch(machine);
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_state_machine_error_when_map_item_processor_is_invalid(
) -> TestResult {
    let machine = machine(
        "MapIt",
        [(
            "MapIt",
            State::new(StateKind::Map(map_state_with_invalid_item_processor(
                end_t(),
            )?)),
        )],
    )?;
    let result = evaluate_state_machine(&machine, &context());
    assert_eq!(
        result,
        Err(StateEvalError::StateMachine(vec![
            StateMachineError::TransitionTargetNotFound {
                from: sn("Init")?,
                target: sn("Ghost")?,
            }
        ])),
    );
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_nested_invalid_task_env_expression_without_eager_evaluation(
) -> TestResult {
    let nested = machine(
        "NestedTask",
        [(
            "NestedTask",
            State::new(StateKind::Task(invalid_task_state(end_t())?)),
        )],
    )?;
    let state = fixture(MapState::new(
        expr("$.input.items")?,
        Box::new(nested),
        Some(2),
        end_t(),
        vec![],
        vec![],
        Some(25.0),
    ))?;
    let machine = machine("MapIt", [("MapIt", State::new(StateKind::Map(state)))])?;
    assert_machine_dispatch(machine);
    Ok(())
}

#[test]
fn evaluate_state_machine_remains_definition_time_only_when_nested_runtime_shaped_states_are_present(
) -> TestResult {
    assert_machine_dispatch(dense_all_variant_machine()?);
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_parallel_inside_map_topology_when_nested_machine_is_valid(
) -> TestResult {
    assert_machine_dispatch(parallel_inside_map_machine()?);
    Ok(())
}

#[test]
fn evaluate_state_machine_preserves_map_inside_parallel_topology_when_nested_machine_is_valid(
) -> TestResult {
    assert_machine_dispatch(map_inside_parallel_machine()?);
    Ok(())
}

#[test]
fn evaluate_state_machine_returns_only_validated_machine_when_recursive_dispatch_succeeds(
) -> TestResult {
    let machine = dense_all_variant_machine()?;
    let result = evaluate_state_machine(&machine, &context())?;
    assert_eq!(result.validate(), Ok(()));
    assert_eq!(result, machine);
    Ok(())
}
