use super::fixtures::*;
use super::machine_fixtures::dense_all_variant_machine;
use super::*;
use rstest::rstest;

fn definition_time_map_state(transition: Transition) -> TestResult<MapState> {
    fixture(MapState::new(
        expr("$.input.items")?,
        Box::new(dense_all_variant_machine()?),
        Some(3),
        transition,
        vec![],
        vec![],
        Some(50.0),
    ))
}

fn assert_round_trip(state: State) {
    assert_eq!(evaluate_state(&state, &context()), Ok(state));
}

#[test]
fn evaluate_state_preserves_task_wrapper_fields_when_task_state_is_valid() -> TestResult {
    let state = wrapped_state(StateKind::Task(valid_task_state(
        Some(10),
        Some(9),
        next_t("Done")?,
    )?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_task_payload_fields_when_task_state_is_valid() -> TestResult {
    let state = State::new(StateKind::Task(valid_task_state(
        Some(10),
        Some(9),
        next_t("Done")?,
    )?));
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_task_timeout_of_one_when_timeout_is_minimum_valid() -> TestResult {
    let state = wrapped_state(StateKind::Task(valid_task_state(
        Some(1),
        None,
        next_t("Done")?,
    )?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_task_heartbeat_of_one_when_timeout_is_two() -> TestResult {
    let state = wrapped_state(StateKind::Task(valid_task_state(
        Some(2),
        Some(1),
        next_t("Done")?,
    )?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_task_heartbeat_when_heartbeat_is_one_below_timeout() -> TestResult {
    let state = wrapped_state(StateKind::Task(valid_task_state(
        Some(10),
        Some(9),
        next_t("Done")?,
    )?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_pass_result_none_when_pass_result_is_absent() -> TestResult {
    let state = wrapped_state(StateKind::Pass(PassState::new(None, next_t("Done")?)))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_pass_result_some_when_pass_result_is_present() -> TestResult {
    let state = wrapped_state(StateKind::Pass(PassState::new(
        Some(json!({"answer": 42})),
        next_t("Done")?,
    )))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_pass_next_transition_when_transition_is_next() -> TestResult {
    let state = wrapped_state(StateKind::Pass(PassState::new(
        Some(json!({"answer": 42})),
        next_t("Done")?,
    )))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_pass_end_transition_when_transition_is_end() -> TestResult {
    let state = wrapped_state(StateKind::Pass(PassState::new(
        Some(json!({"answer": 42})),
        end_t(),
    )))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_choice_rule_order_when_choice_state_is_valid() -> TestResult {
    let state = wrapped_state(StateKind::Choice(valid_choice_state()?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_choice_rule_payloads_and_default_when_choice_state_is_valid(
) -> TestResult {
    let state = wrapped_state(StateKind::Choice(valid_choice_state()?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_does_not_select_a_branch_when_choice_state_is_dispatched() -> TestResult {
    let state = wrapped_state(StateKind::Choice(valid_choice_state()?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_wait_seconds_when_wait_duration_is_seconds() -> TestResult {
    assert_wait_state_preserved(WaitDuration::Seconds(30))
}

#[test]
fn evaluate_state_preserves_wait_timestamp_when_wait_duration_is_timestamp() -> TestResult {
    assert_wait_state_preserved(WaitDuration::Timestamp("2026-04-13T00:00:00Z".to_owned()))
}

#[test]
fn evaluate_state_preserves_wait_seconds_path_when_wait_duration_is_seconds_path() -> TestResult {
    assert_wait_state_preserved(WaitDuration::SecondsPath(jp("$.input.seconds")?))
}

#[test]
fn evaluate_state_preserves_wait_timestamp_path_when_wait_duration_is_timestamp_path() -> TestResult
{
    assert_wait_state_preserved(WaitDuration::TimestampPath(jp("$.input.timestamp")?))
}

#[test]
fn evaluate_state_preserves_succeed_terminality_when_succeed_state_is_valid() -> TestResult {
    let state = wrapped_state(StateKind::Succeed(SucceedState::new()))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_fail_without_error_or_cause_when_both_literals_are_absent() -> TestResult
{
    assert_fail_state_preserved(None, None)
}

#[test]
fn evaluate_state_preserves_fail_error_when_only_error_literal_is_present() -> TestResult {
    assert_fail_state_preserved(Some("Boom"), None)
}

#[test]
fn evaluate_state_preserves_fail_cause_when_only_cause_literal_is_present() -> TestResult {
    assert_fail_state_preserved(None, Some("Because reasons"))
}

#[test]
fn evaluate_state_preserves_fail_error_and_cause_when_both_literals_are_present() -> TestResult {
    assert_fail_state_preserved(Some("Boom"), Some("Because reasons"))
}

#[test]
fn evaluate_state_preserves_parallel_branch_order_when_parallel_state_is_valid() -> TestResult {
    let state = wrapped_state(StateKind::Parallel(valid_parallel_state(next_t("Done")?)?))?;
    assert_round_trip(state);
    Ok(())
}

#[rstest]
#[case(None)]
#[case(Some(0.0))]
#[case(Some(100.0))]
fn evaluate_state_preserves_map_tolerated_failure_percentage_when_value_is_valid(
    #[case] tolerance: Option<f64>,
) -> TestResult {
    let state = wrapped_state(StateKind::Map(valid_map_state(tolerance, next_t("Done")?)?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_validated_newtypes_when_asl_state_contains_them() -> TestResult {
    let state = wrapped_state(StateKind::Task(valid_task_state(
        Some(10),
        Some(9),
        next_t("Done")?,
    )?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_remains_definition_time_only_when_runtime_shaped_fields_are_present() -> TestResult
{
    let state = wrapped_state(StateKind::Map(definition_time_map_state(end_t())?))?;
    assert_round_trip(state);
    Ok(())
}

#[test]
fn evaluate_state_preserves_invalid_task_env_expression_without_eager_evaluation() -> TestResult {
    let invalid_task = invalid_task_state(next_t("Done")?)?;
    let state = wrapped_state(StateKind::Task(invalid_task))?;
    assert_round_trip(state);
    Ok(())
}
