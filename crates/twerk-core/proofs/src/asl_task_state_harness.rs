use std::collections::HashMap;
use twerk_core::asl::catcher::Catcher;
use twerk_core::asl::error_code::ErrorCode;
use twerk_core::asl::retrier::{JitterStrategy, Retrier};
use twerk_core::asl::task_state::{TaskState, TaskStateError};
use twerk_core::asl::transition::Transition;
use twerk_core::asl::types::{BackoffRate, ImageRef, ShellScript, StateName};

/// Helper: valid default params for constructing a TaskState.
fn valid_params() -> (
    ImageRef,
    ShellScript,
    HashMap<String, twerk_core::asl::types::Expression>,
    Option<twerk_core::asl::types::VariableName>,
    Option<u64>,
    Option<u64>,
    Vec<Retrier>,
    Vec<Catcher>,
    Transition,
) {
    let image = ImageRef::new("ubuntu:latest").unwrap();
    let run = ShellScript::new("echo hello").unwrap();
    let env = HashMap::new();
    let var = None;
    let timeout = None;
    let heartbeat = None;
    let retry = vec![];
    let catchers = vec![];
    let transition = Transition::next(StateName::new("NextState").unwrap());
    (image, run, env, var, timeout, heartbeat, retry, catchers, transition)
}

// ---------------------------------------------------------------------------
// Accepts valid parameters
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_accepts_valid() {
    let (image, run, env, var, timeout, heartbeat, retry, catchers, transition) = valid_params();
    let result = TaskState::new(image, run, env, var, timeout, heartbeat, retry, catchers, transition);
    assert!(result.is_ok(), "Valid parameters should create TaskState");
}

// ---------------------------------------------------------------------------
// Rejects zero timeout
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_rejects_zero_timeout() {
    let (image, run, env, var, _, heartbeat, retry, catchers, transition) = valid_params();
    let result = TaskState::new(image, run, env, var, Some(0), heartbeat, retry, catchers, transition);
    assert!(result.is_err(), "timeout=0 should be rejected");
    match result {
        Err(TaskStateError::TimeoutTooSmall(0)) => {}
        other => panic!("Expected TimeoutTooSmall(0), got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Rejects zero heartbeat
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_rejects_zero_heartbeat() {
    let (image, run, env, var, timeout, _, retry, catchers, transition) = valid_params();
    let result = TaskState::new(image, run, env, var, timeout, Some(0), retry, catchers, transition);
    assert!(result.is_err(), "heartbeat=0 should be rejected");
    match result {
        Err(TaskStateError::HeartbeatTooSmall(0)) => {}
        other => panic!("Expected HeartbeatTooSmall(0), got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Rejects heartbeat >= timeout
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_rejects_heartbeat_exceeds_timeout() {
    let (image, run, env, var, _, _, retry, catchers, transition) = valid_params();
    let result = TaskState::new(
        image, run, env, var,
        Some(5),
        Some(5),
        retry, catchers, transition,
    );
    assert!(result.is_err(), "heartbeat == timeout should be rejected");
}

#[kani::proof]
fn asl_task_state_rejects_heartbeat_greater_than_timeout() {
    let (image, run, env, var, _, _, retry, catchers, transition) = valid_params();
    let result = TaskState::new(
        image, run, env, var,
        Some(5),
        Some(10),
        retry, catchers, transition,
    );
    assert!(result.is_err(), "heartbeat > timeout should be rejected");
}

// ---------------------------------------------------------------------------
// Accepts heartbeat < timeout
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_accepts_heartbeat_less_than_timeout() {
    let (image, run, env, var, _, _, retry, catchers, transition) = valid_params();
    let result = TaskState::new(
        image, run, env, var,
        Some(10),
        Some(5),
        retry, catchers, transition,
    );
    assert!(result.is_ok(), "heartbeat < timeout should be accepted");
}

// ---------------------------------------------------------------------------
// Rejects empty env key
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_rejects_empty_env_key() {
    let (image, run, _, var, timeout, heartbeat, retry, catchers, transition) = valid_params();
    let mut env = HashMap::new();
    env.insert(
        String::new(),
        twerk_core::asl::types::Expression::new("value").unwrap(),
    );
    let result = TaskState::new(image, run, env, var, timeout, heartbeat, retry, catchers, transition);
    assert!(result.is_err(), "Empty env key should be rejected");
    match result {
        Err(TaskStateError::EmptyEnvKey) => {}
        other => panic!("Expected EmptyEnvKey, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Accepts valid timeout >= 1
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_accepts_valid_timeout() {
    let (image, run, env, var, _, heartbeat, retry, catchers, transition) = valid_params();
    let result = TaskState::new(image, run, env, var, Some(1), heartbeat, retry, catchers, transition);
    assert!(result.is_ok(), "timeout=1 should be accepted");
}

// ---------------------------------------------------------------------------
// Accepts valid heartbeat >= 1
// ---------------------------------------------------------------------------

#[kani::proof]
fn asl_task_state_accepts_valid_heartbeat() {
    let (image, run, env, var, timeout, _, retry, catchers, transition) = valid_params();
    let result = TaskState::new(image, run, env, var, timeout, Some(1), retry, catchers, transition);
    assert!(result.is_ok(), "heartbeat=1 should be accepted");
}
