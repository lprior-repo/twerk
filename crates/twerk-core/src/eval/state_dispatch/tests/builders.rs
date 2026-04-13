use super::fixtures::*;
use super::*;

#[test]
fn build_task_arm_returns_task_state_error_when_timeout_is_zero() -> TestResult {
    let result = build_task_arm(task_arm_spec(task_env()?, Some(0), None)?);
    assert_eq!(
        result,
        Err(StateEvalError::TaskState(TaskStateError::TimeoutTooSmall(
            0
        )))
    );
    Ok(())
}

#[test]
fn build_task_arm_returns_task_state_error_when_heartbeat_is_zero() -> TestResult {
    let result = build_task_arm(task_arm_spec(task_env()?, Some(10), Some(0))?);
    assert_eq!(
        result,
        Err(StateEvalError::TaskState(
            TaskStateError::HeartbeatTooSmall(0)
        ))
    );
    Ok(())
}

#[test]
fn build_task_arm_returns_task_state_error_when_heartbeat_exceeds_timeout() -> TestResult {
    let result = build_task_arm(task_arm_spec(task_env()?, Some(5), Some(10))?);
    assert_eq!(
        result,
        Err(StateEvalError::TaskState(
            TaskStateError::HeartbeatExceedsTimeout {
                heartbeat: 10,
                timeout: 5,
            }
        )),
    );
    Ok(())
}

#[test]
fn build_task_arm_returns_task_state_error_when_heartbeat_equals_timeout() -> TestResult {
    let result = build_task_arm(task_arm_spec(task_env()?, Some(5), Some(5))?);
    assert_eq!(
        result,
        Err(StateEvalError::TaskState(
            TaskStateError::HeartbeatExceedsTimeout {
                heartbeat: 5,
                timeout: 5,
            }
        )),
    );
    Ok(())
}

#[test]
fn build_task_arm_returns_task_state_error_when_env_key_is_empty() -> TestResult {
    let env = HashMap::from([("".to_owned(), expr("7")?)]);
    let result = build_task_arm(task_arm_spec(env, Some(10), Some(9))?);
    assert_eq!(
        result,
        Err(StateEvalError::TaskState(TaskStateError::EmptyEnvKey))
    );
    Ok(())
}

#[test]
fn build_choice_arm_returns_choice_state_error_when_choices_is_empty() -> TestResult {
    let result = build_choice_arm(vec![], None);
    assert_eq!(
        result,
        Err(StateEvalError::ChoiceState(ChoiceStateError::EmptyChoices))
    );
    Ok(())
}

#[test]
fn build_parallel_arm_returns_parallel_state_error_when_branches_is_empty() -> TestResult {
    let result = build_parallel_arm(ParallelArmSpec {
        branches: vec![],
        transition: end_t(),
        fail_fast: ParallelArmFailFast::RuntimeDefault,
    });
    assert_eq!(
        result,
        Err(StateEvalError::ParallelState(
            ParallelStateError::EmptyBranches
        ))
    );
    Ok(())
}

#[test]
fn build_map_arm_returns_map_state_error_when_tolerance_is_out_of_range() -> TestResult {
    let result = build_map_arm(map_arm_spec(Some(101.0))?);
    assert_eq!(
        result,
        Err(StateEvalError::MapState(
            MapStateError::InvalidToleratedFailurePercentage(101.0)
        )),
    );
    Ok(())
}

#[test]
fn build_map_arm_returns_map_state_error_when_tolerance_is_below_range() -> TestResult {
    let result = build_map_arm(map_arm_spec(Some(-1.0))?);
    assert_eq!(
        result,
        Err(StateEvalError::MapState(
            MapStateError::InvalidToleratedFailurePercentage(-1.0)
        )),
    );
    Ok(())
}

#[test]
fn build_map_arm_returns_map_state_error_when_tolerance_is_not_finite() -> TestResult {
    let result = build_map_arm(map_arm_spec(Some(f64::NAN))?);
    assert_eq!(
        result,
        Err(StateEvalError::MapState(
            MapStateError::NonFiniteToleratedFailurePercentage
        )),
    );
    Ok(())
}

#[test]
fn build_map_arm_returns_map_state_error_when_tolerance_is_positive_infinity() -> TestResult {
    let result = build_map_arm(map_arm_spec(Some(f64::INFINITY))?);
    assert_eq!(
        result,
        Err(StateEvalError::MapState(
            MapStateError::NonFiniteToleratedFailurePercentage
        )),
    );
    Ok(())
}
