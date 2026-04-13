use super::fixtures::*;
use super::*;

fn assert_kind_round_trip(kind: StateKind) {
    assert_eq!(evaluate_state_kind(&kind, &context()), Ok(kind));
}

#[test]
fn evaluate_state_kind_dispatches_task_arm_when_task_kind_is_valid() -> TestResult {
    assert_kind_round_trip(StateKind::Task(valid_task_state(
        Some(10),
        Some(9),
        next_t("Done")?,
    )?));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_pass_arm_when_pass_kind_is_valid() -> TestResult {
    assert_kind_round_trip(StateKind::Pass(PassState::new(
        Some(json!({"answer": 42})),
        next_t("Done")?,
    )));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_choice_arm_without_selecting_a_branch() -> TestResult {
    assert_kind_round_trip(StateKind::Choice(valid_choice_state()?));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_wait_arm_preserving_duration_discriminant() -> TestResult {
    assert_kind_round_trip(StateKind::Wait(valid_wait_state(
        WaitDuration::Seconds(5),
        next_t("Done")?,
    )));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_succeed_arm_without_adding_transition() -> TestResult {
    assert_kind_round_trip(StateKind::Succeed(SucceedState::new()));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_fail_arm_without_adding_transition() -> TestResult {
    assert_kind_round_trip(StateKind::Fail(FailState::new(
        Some("boom".to_owned()),
        Some("cause".to_owned()),
    )));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_parallel_arm_preserving_branch_order() -> TestResult {
    assert_kind_round_trip(StateKind::Parallel(valid_parallel_state(next_t("Done")?)?));
    Ok(())
}

#[test]
fn evaluate_state_kind_dispatches_map_arm_preserving_item_processor_and_tolerance() -> TestResult {
    assert_kind_round_trip(StateKind::Map(valid_map_state(
        Some(25.0),
        next_t("Done")?,
    )?));
    Ok(())
}
