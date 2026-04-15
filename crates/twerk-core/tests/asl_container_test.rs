//! Tests for ASL container types: TaskState, ParallelState, MapState.

use std::collections::HashMap;

use indexmap::IndexMap;
use twerk_core::asl::machine::StateMachine;
use twerk_core::asl::map::{MapState, MapStateError};
use twerk_core::asl::parallel::{ParallelState, ParallelStateError};
use twerk_core::asl::state::{State, StateKind};
use twerk_core::asl::task_state::{TaskState, TaskStateError};
use twerk_core::asl::terminal::SucceedState;
use twerk_core::asl::{
    BackoffRate, Catcher, ErrorCode, Expression, ImageRef, JitterStrategy, Retrier, ShellScript,
    StateName, Transition, VariableName,
};

// ---- helpers ----

fn img(s: &str) -> ImageRef {
    ImageRef::new(s).expect("test image")
}
fn script(s: &str) -> ShellScript {
    ShellScript::new(s).expect("test script")
}
fn expr(s: &str) -> Expression {
    Expression::new(s).expect("test expression")
}
fn var(s: &str) -> VariableName {
    VariableName::new(s).expect("test var")
}
fn sn(s: &str) -> StateName {
    StateName::new(s).expect("test state name")
}
fn next_t(s: &str) -> Transition {
    Transition::next(sn(s))
}
fn end_t() -> Transition {
    Transition::end()
}

fn sample_retrier() -> Retrier {
    Retrier::new(
        vec![ErrorCode::TaskFailed],
        1,
        3,
        BackoffRate::new(2.0).expect("test backoff"),
        None,
        JitterStrategy::None,
    )
    .expect("test retrier")
}

fn sample_catcher() -> Catcher {
    Catcher::new(vec![ErrorCode::All], sn("ErrorHandler"), None, None).expect("test catcher")
}

fn make_sub_machine() -> StateMachine {
    let mut states = IndexMap::new();
    states.insert(
        sn("S1"),
        State::new(StateKind::Succeed(SucceedState::new())),
    );
    StateMachine::new(sn("S1"), states)
}

fn make_sub_machine_named(name: &str) -> StateMachine {
    let mut states = IndexMap::new();
    states.insert(
        sn(name),
        State::new(StateKind::Succeed(SucceedState::new())),
    );
    StateMachine::new(sn(name), states)
}

// =========================================================================
// TaskState
// =========================================================================

// TS-1: Valid construction with all fields
#[test]
fn task_state_valid_all_fields() {
    let mut env = HashMap::new();
    env.insert("API_KEY".into(), expr("$.secrets.api_key"));

    let ts = TaskState::new(
        img("alpine:3.19"),
        script("echo hello"),
        env,
        Some(var("output")),
        Some(300),
        Some(60),
        vec![sample_retrier()],
        vec![sample_catcher()],
        next_t("NextStep"),
    )
    .expect("valid task state");

    assert_eq!(ts.image().as_str(), "alpine:3.19");
    assert_eq!(ts.run().as_str(), "echo hello");
    assert_eq!(ts.env().len(), 1);
    assert_eq!(
        ts.env().get("API_KEY").map(|e| e.as_str()),
        Some("$.secrets.api_key")
    );
    assert_eq!(ts.var().map(|v| v.as_str()), Some("output"));
    assert_eq!(ts.timeout(), Some(300));
    assert_eq!(ts.heartbeat(), Some(60));
    assert_eq!(ts.retry().len(), 1);
    assert_eq!(ts.catch().len(), 1);
    assert!(ts.transition().is_next());
}

// TS-2: Valid construction with minimal fields
#[test]
fn task_state_valid_minimal() {
    let ts = TaskState::new(
        img("ubuntu:22.04"),
        script("ls -la"),
        HashMap::new(),
        None,
        None,
        None,
        vec![],
        vec![],
        end_t(),
    )
    .expect("valid minimal task state");

    assert!(ts.env().is_empty());
    assert_eq!(ts.var(), None);
    assert_eq!(ts.timeout(), None);
    assert_eq!(ts.heartbeat(), None);
    assert!(ts.retry().is_empty());
    assert!(ts.catch().is_empty());
    assert!(ts.transition().is_end());
}

// TS-3: Reject timeout = 0
#[test]
fn task_state_reject_timeout_zero() {
    let err = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        Some(0),
        None,
        vec![],
        vec![],
        end_t(),
    )
    .expect_err("timeout 0 should fail");
    assert_eq!(err, TaskStateError::TimeoutTooSmall(0));
}

// TS-4: Reject heartbeat = 0
#[test]
fn task_state_reject_heartbeat_zero() {
    let err = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        None,
        Some(0),
        vec![],
        vec![],
        end_t(),
    )
    .expect_err("heartbeat 0 should fail");
    assert_eq!(err, TaskStateError::HeartbeatTooSmall(0));
}

// TS-5: Reject heartbeat == timeout
#[test]
fn task_state_reject_heartbeat_equals_timeout() {
    let err = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        Some(60),
        Some(60),
        vec![],
        vec![],
        end_t(),
    )
    .expect_err("heartbeat == timeout should fail");
    assert_eq!(
        err,
        TaskStateError::HeartbeatExceedsTimeout {
            heartbeat: 60,
            timeout: 60,
        }
    );
}

// TS-6: Reject heartbeat > timeout
#[test]
fn task_state_reject_heartbeat_exceeds_timeout() {
    let err = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        Some(30),
        Some(60),
        vec![],
        vec![],
        end_t(),
    )
    .expect_err("heartbeat > timeout should fail");
    assert_eq!(
        err,
        TaskStateError::HeartbeatExceedsTimeout {
            heartbeat: 60,
            timeout: 30,
        }
    );
}

// TS-7: Allow heartbeat without timeout
#[test]
fn task_state_heartbeat_without_timeout() {
    let ts = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        None,
        Some(30),
        vec![],
        vec![],
        end_t(),
    )
    .expect("heartbeat without timeout is valid");
    assert_eq!(ts.heartbeat(), Some(30));
    assert_eq!(ts.timeout(), None);
}

// TS-8: Reject empty env key
#[test]
fn task_state_reject_empty_env_key() {
    let mut env = HashMap::new();
    env.insert(String::new(), expr("value"));

    let err = TaskState::new(
        img("a"),
        script("b"),
        env,
        None,
        None,
        None,
        vec![],
        vec![],
        end_t(),
    )
    .expect_err("empty env key should fail");
    assert_eq!(err, TaskStateError::EmptyEnvKey);
}

// TS-9: Boundary -- timeout = 1 (minimum)
#[test]
fn task_state_timeout_minimum() {
    let ts = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        Some(1),
        None,
        vec![],
        vec![],
        end_t(),
    )
    .expect("timeout 1 is valid");
    assert_eq!(ts.timeout(), Some(1));
}

// TS-10: Boundary -- heartbeat just below timeout
#[test]
fn task_state_heartbeat_just_below_timeout() {
    let ts = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        Some(10),
        Some(9),
        vec![],
        vec![],
        end_t(),
    )
    .expect("heartbeat 9 < timeout 10");
    assert_eq!(ts.heartbeat(), Some(9));
    assert_eq!(ts.timeout(), Some(10));
}

// TS-11: Serde JSON roundtrip full
#[test]
fn task_state_serde_json_roundtrip_full() {
    let mut env = HashMap::new();
    env.insert("API_KEY".into(), expr("$.secrets.key"));

    let ts = TaskState::new(
        img("alpine:3.19"),
        script("echo hello"),
        env,
        Some(var("output")),
        Some(300),
        Some(60),
        vec![sample_retrier()],
        vec![sample_catcher()],
        next_t("NextStep"),
    )
    .expect("valid");

    let json = serde_json::to_string(&ts).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse value");

    // Flattened transition
    assert_eq!(parsed["next"], "NextStep");
    assert_eq!(parsed["image"], "alpine:3.19");
    assert_eq!(parsed["run"], "echo hello");

    // Roundtrip
    let ts2: TaskState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ts, ts2);
}

// TS-12: Serde omits default/empty fields
#[test]
fn task_state_serde_omits_defaults() {
    let ts = TaskState::new(
        img("a"),
        script("b"),
        HashMap::new(),
        None,
        None,
        None,
        vec![],
        vec![],
        end_t(),
    )
    .expect("valid");

    let json = serde_json::to_string(&ts).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed.get("env"), None);
    assert_eq!(parsed.get("var"), None);
    assert_eq!(parsed.get("timeout"), None);
    assert_eq!(parsed.get("heartbeat"), None);
    assert_eq!(parsed.get("retry"), None);
    assert_eq!(parsed.get("catch"), None);
    assert_eq!(parsed.get("image"), Some(&serde_json::json!("a")));
    assert_eq!(parsed.get("run"), Some(&serde_json::json!("b")));
}

// TS-13: Serde deserialize with defaults
#[test]
fn task_state_serde_deserialize_defaults() {
    let json = r#"{"image": "alpine:latest", "run": "ls", "end": true}"#;
    let ts: TaskState = serde_json::from_str(json).expect("deser");

    assert!(ts.env().is_empty());
    assert_eq!(ts.var(), None);
    assert_eq!(ts.timeout(), None);
    assert_eq!(ts.heartbeat(), None);
    assert!(ts.retry().is_empty());
    assert!(ts.catch().is_empty());
    assert!(ts.transition().is_end());
}

// TS-14: Serde rejects heartbeat >= timeout
#[test]
fn task_state_serde_reject_heartbeat_exceeds_timeout() {
    let json =
        r#"{"image": "alpine:latest", "run": "ls", "timeout": 10, "heartbeat": 10, "end": true}"#;
    let result = serde_json::from_str::<TaskState>(json);
    let err = result.unwrap_err();
    assert!(err.to_string().contains("heartbeat"), "{err}");
}

// TS-15: Serde env map with multiple entries
#[test]
fn task_state_serde_env_multiple() {
    let json = r#"{
        "image": "node:20",
        "run": "npm test",
        "env": {"NODE_ENV": "test", "CI": "true"},
        "end": true
    }"#;
    let ts: TaskState = serde_json::from_str(json).expect("deser");
    assert_eq!(ts.env().len(), 2);
    assert_eq!(ts.env().get("NODE_ENV").map(|e| e.as_str()), Some("test"));
    assert_eq!(ts.env().get("CI").map(|e| e.as_str()), Some("true"));
}

// TS-16: YAML deserialization
#[test]
fn task_state_yaml_deserialize() {
    let yaml = "image: \"alpine:3.19\"\nrun: \"echo hello\"\nenv:\n  GREETING: hello\ntimeout: 120\nnext: ProcessResult";
    let ts: TaskState = serde_saphyr::from_str(yaml).expect("yaml deser");
    assert_eq!(ts.image().as_str(), "alpine:3.19");
    assert_eq!(ts.env().get("GREETING").map(|e| e.as_str()), Some("hello"));
    assert_eq!(ts.timeout(), Some(120));
    assert!(ts.transition().is_next());
}

// =========================================================================
// ParallelState
// =========================================================================

// PS-1: Valid with multiple branches
#[test]
fn parallel_state_valid_multiple_branches() {
    let branches = vec![make_sub_machine_named("A"), make_sub_machine_named("B")];
    let ps =
        ParallelState::new(branches, next_t("MergeResults"), Some(true)).expect("valid parallel");
    assert_eq!(ps.branches().len(), 2);
    assert!(ps.transition().is_next());
    assert_eq!(ps.fail_fast(), Some(true));
}

// PS-2: Valid with single branch
#[test]
fn parallel_state_valid_single_branch() {
    let branches = vec![make_sub_machine()];
    let ps = ParallelState::new(branches, end_t(), None).expect("valid single branch");
    assert_eq!(ps.branches().len(), 1);
    assert_eq!(ps.fail_fast(), None);
}

// PS-3: Reject empty branches
#[test]
fn parallel_state_reject_empty_branches() {
    let err = ParallelState::new(vec![], end_t(), None).expect_err("empty branches");
    assert_eq!(err, ParallelStateError::EmptyBranches);
}

// PS-4: fail_fast explicit false
#[test]
fn parallel_state_fail_fast_false() {
    let branches = vec![make_sub_machine_named("A"), make_sub_machine_named("B")];
    let ps = ParallelState::new(branches, end_t(), Some(false)).expect("valid");
    assert_eq!(ps.fail_fast(), Some(false));
}

// PS-5: Serde roundtrip
#[test]
fn parallel_state_serde_json_roundtrip() {
    let branches = vec![make_sub_machine_named("S1"), make_sub_machine_named("S2")];
    let ps = ParallelState::new(branches, next_t("Merge"), Some(true)).expect("valid");

    let json = serde_json::to_string(&ps).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["next"], "Merge");
    assert_eq!(parsed["failFast"], true);

    let ps2: ParallelState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ps, ps2);
}

// PS-6: Serde omits None failFast
#[test]
fn parallel_state_serde_omits_none_fail_fast() {
    let branches = vec![make_sub_machine()];
    let ps = ParallelState::new(branches, end_t(), None).expect("valid");

    let json = serde_json::to_string(&ps).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(parsed.get("failFast"), None);
}

// PS-7: Serde rejects empty branches
#[test]
fn parallel_state_serde_reject_empty_branches() {
    let json = r#"{"branches": [], "end": true}"#;
    let result = serde_json::from_str::<ParallelState>(json);
    let err = result.unwrap_err();
    assert!(err.to_string().contains("branch"), "{err}");
}

// =========================================================================
// MapState
// =========================================================================

// MS-1: Valid with all fields
#[test]
fn map_state_valid_all_fields() {
    let ms = MapState::new(
        expr("$.items"),
        Box::new(make_sub_machine_named("Process")),
        Some(10),
        next_t("Aggregate"),
        vec![sample_retrier()],
        vec![sample_catcher()],
        Some(25.0),
    )
    .expect("valid map state");

    assert_eq!(ms.items_path().as_str(), "$.items");
    assert_eq!(ms.max_concurrency(), Some(10));
    assert!(ms.transition().is_next());
    assert_eq!(ms.retry().len(), 1);
    assert_eq!(ms.catch().len(), 1);
    assert_eq!(ms.tolerated_failure_percentage(), Some(25.0));
}

// MS-2: Valid minimal
#[test]
fn map_state_valid_minimal() {
    let ms = MapState::new(
        expr("$.data"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        None,
    )
    .expect("valid minimal map");

    assert_eq!(ms.max_concurrency(), None);
    assert!(ms.retry().is_empty());
    assert!(ms.catch().is_empty());
    assert_eq!(ms.tolerated_failure_percentage(), None);
}

// MS-3: Reject tolerated < 0
#[test]
fn map_state_reject_negative_tolerance() {
    let err = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(-1.0),
    )
    .expect_err("negative tolerance");
    assert_eq!(err, MapStateError::InvalidToleratedFailurePercentage(-1.0));
}

// MS-4: Reject tolerated > 100
#[test]
fn map_state_reject_over_hundred_tolerance() {
    let err = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(100.1),
    )
    .expect_err("over 100 tolerance");
    assert_eq!(err, MapStateError::InvalidToleratedFailurePercentage(100.1));
}

// MS-5: Reject NaN
#[test]
fn map_state_reject_nan_tolerance() {
    let err = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(f64::NAN),
    )
    .expect_err("NaN tolerance");
    assert_eq!(err, MapStateError::NonFiniteToleratedFailurePercentage);
}

// MS-6: Reject infinity
#[test]
fn map_state_reject_infinity_tolerance() {
    let err = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(f64::INFINITY),
    )
    .expect_err("infinity tolerance");
    assert_eq!(err, MapStateError::NonFiniteToleratedFailurePercentage);
}

// MS-7: Boundary 0.0
#[test]
fn map_state_tolerance_zero() {
    let ms = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(0.0),
    )
    .expect("zero tolerance");
    assert_eq!(ms.tolerated_failure_percentage(), Some(0.0));
}

// MS-8: Boundary 100.0
#[test]
fn map_state_tolerance_hundred() {
    let ms = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(100.0),
    )
    .expect("100% tolerance");
    assert_eq!(ms.tolerated_failure_percentage(), Some(100.0));
}

// MS-9: max_concurrency = 0
#[test]
fn map_state_max_concurrency_zero() {
    let ms = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        Some(0),
        end_t(),
        vec![],
        vec![],
        None,
    )
    .expect("concurrency 0");
    assert_eq!(ms.max_concurrency(), Some(0));
}

// MS-10: Serde roundtrip
#[test]
fn map_state_serde_json_roundtrip() {
    let ms = MapState::new(
        expr("$.items"),
        Box::new(make_sub_machine_named("P")),
        Some(5),
        next_t("Done"),
        vec![sample_retrier()],
        vec![sample_catcher()],
        Some(10.0),
    )
    .expect("valid");

    let json = serde_json::to_string(&ms).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed["itemsPath"], "$.items");
    assert_eq!(parsed["next"], "Done");
    assert_eq!(parsed["maxConcurrency"], 5);
    assert_eq!(parsed["toleratedFailurePercentage"], 10.0);

    let ms2: MapState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ms, ms2);
}

// MS-11: Serde omits defaults
#[test]
fn map_state_serde_omits_defaults() {
    let ms = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        None,
    )
    .expect("valid");

    let json = serde_json::to_string(&ms).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert_eq!(parsed.get("maxConcurrency"), None);
    assert_eq!(parsed.get("retry"), None);
    assert_eq!(parsed.get("catch"), None);
    assert_eq!(parsed.get("toleratedFailurePercentage"), None);
    assert!(
        parsed.get("itemsPath").is_some(),
        "itemsPath must be present"
    );
    assert!(
        parsed.get("itemProcessor").is_some(),
        "itemProcessor must be present"
    );
}

// MS-12: Serde rejects invalid tolerance
#[test]
fn map_state_serde_reject_invalid_tolerance() {
    let json = r#"{"itemsPath": "$.items", "itemProcessor": {"startAt": "S", "states": {"S": {"type": "succeed"}}}, "toleratedFailurePercentage": 150.0, "end": true}"#;
    let result = serde_json::from_str::<MapState>(json);
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("tolerated") || err.to_string().contains("150"),
        "{err}"
    );
}

// MS-14: Reject neg infinity
#[test]
fn map_state_reject_neg_infinity_tolerance() {
    let err = MapState::new(
        expr("$.x"),
        Box::new(make_sub_machine()),
        None,
        end_t(),
        vec![],
        vec![],
        Some(f64::NEG_INFINITY),
    )
    .expect_err("neg infinity");
    assert_eq!(err, MapStateError::NonFiniteToleratedFailurePercentage);
}

// =========================================================================
// StateMachine YAML Parsing
// =========================================================================

#[test]
fn state_machine_parses_asl_yaml() {
    let yaml = r#"
comment: "Simple two-step ASL workflow"
startAt: Hello
states:
  Hello:
    type: pass
    result: "Hello, World!"
    next: Goodbye
  Goodbye:
    type: pass
    result: "Goodbye!"
    end: true
"#;
    let sm: StateMachine = serde_saphyr::from_str(yaml).expect("yaml deser");
    assert_eq!(sm.start_at().as_str(), "Hello");
    assert_eq!(sm.states().len(), 2);
    assert!(sm.validate().is_ok());
}

#[test]
fn state_machine_parses_asl_yaml_with_task() {
    let yaml = r#"
startAt: RunTask
states:
  RunTask:
    type: task
    image: alpine:3.19
    run: echo hello
    next: Done
  Done:
    type: succeed
"#;
    let sm: StateMachine = serde_saphyr::from_str(yaml).expect("yaml deser");
    assert_eq!(sm.states().len(), 2);
    assert!(sm.validate().is_ok());
}

#[test]
fn state_machine_parses_real_yaml_file() {
    use std::fs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("could not resolve workspace root");
    let asl_yaml = workspace_root.join("examples/asl-hello.yaml");

    let content = fs::read_to_string(&asl_yaml)
        .unwrap_or_else(|_| panic!("Failed to read {}", asl_yaml.display()));

    let sm: StateMachine = serde_saphyr::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", asl_yaml.display(), e));

    assert_eq!(sm.start_at().as_str(), "Hello");
    assert_eq!(sm.states().len(), 2);
    assert!(sm.validate().is_ok());
}

#[test]
fn state_machine_parses_task_with_retry_yaml() {
    use std::fs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("could not resolve workspace root");
    let yaml_path = workspace_root.join("examples/asl-task-retry.yaml");

    let content = fs::read_to_string(&yaml_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", yaml_path.display()));

    let sm: StateMachine = serde_saphyr::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", yaml_path.display(), e));

    assert_eq!(sm.start_at().as_str(), "Build");
    assert_eq!(sm.states().len(), 3);
    assert!(sm.validate().is_ok());

    let build_state = sm.get_state(&sn("Build")).expect("Build state");
    let task_kind = build_state.kind();
    assert!(matches!(task_kind, StateKind::Task(_)));
}
