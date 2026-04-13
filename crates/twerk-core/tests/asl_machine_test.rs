//! Tests for ASL State, StateKind, StateMachine, and StateMachineError.

use std::collections::HashMap;

use indexmap::IndexMap;
use twerk_core::asl::choice::{ChoiceRule, ChoiceState};
use twerk_core::asl::machine::{StateMachine, StateMachineError};
use twerk_core::asl::map::MapState;
use twerk_core::asl::parallel::ParallelState;
use twerk_core::asl::pass::PassState;
use twerk_core::asl::state::{State, StateKind};
use twerk_core::asl::task_state::TaskState;
use twerk_core::asl::terminal::{FailState, SucceedState};
use twerk_core::asl::wait::{WaitDuration, WaitState};
use twerk_core::asl::{
    Expression, ImageRef, JsonPath, ShellScript, StateName, Transition, VariableName,
};

// ---- helpers ----

fn sn(s: &str) -> StateName {
    StateName::new(s).expect("test state name")
}
fn expr(s: &str) -> Expression {
    Expression::new(s).expect("test expression")
}
fn next_t(s: &str) -> Transition {
    Transition::next(sn(s))
}
fn end_t() -> Transition {
    Transition::end()
}

fn make_task_state(transition: Transition) -> TaskState {
    TaskState::new(
        ImageRef::new("alpine:3.18").expect("img"),
        ShellScript::new("echo hello").expect("script"),
        HashMap::new(),
        None,
        None,
        None,
        vec![],
        vec![],
        transition,
    )
    .expect("task state")
}

fn make_pass_next(target: &str) -> State {
    State::new(StateKind::Pass(PassState::new(None, next_t(target))))
}

fn make_pass_end() -> State {
    State::new(StateKind::Pass(PassState::new(None, end_t())))
}

fn make_succeed() -> State {
    State::new(StateKind::Succeed(SucceedState::new()))
}

fn make_fail() -> State {
    State::new(StateKind::Fail(FailState::new(
        Some("Err".to_owned()),
        Some("cause".to_owned()),
    )))
}

fn make_choice(targets: &[&str], default: Option<&str>) -> State {
    let rules: Vec<ChoiceRule> = targets
        .iter()
        .map(|t| ChoiceRule::new(expr("$.x > 0"), sn(t), None))
        .collect();
    State::new(StateKind::Choice(
        ChoiceState::new(rules, default.map(sn)).expect("choice"),
    ))
}

fn simple_machine() -> StateMachine {
    let mut states = IndexMap::new();
    states.insert(sn("Init"), make_pass_next("Done"));
    states.insert(sn("Done"), make_succeed());
    StateMachine::new(sn("Init"), states)
}

// =========================================================================
// StateKind discrimination tests
// =========================================================================

#[test]
fn statekind_task_is_not_terminal() {
    let sk = StateKind::Task(make_task_state(next_t("Step2")));
    assert!(!sk.is_terminal());
}

#[test]
fn statekind_task_has_transition() {
    let sk = StateKind::Task(make_task_state(next_t("Step2")));
    let t = sk.transition().expect("task should have transition");
    assert_eq!(t, &next_t("Step2"));
}

#[test]
fn statekind_succeed_is_terminal() {
    let sk = StateKind::Succeed(SucceedState::new());
    assert!(sk.is_terminal());
}

#[test]
fn statekind_succeed_has_no_transition() {
    let sk = StateKind::Succeed(SucceedState::new());
    assert!(sk.transition().is_none());
}

#[test]
fn statekind_fail_is_terminal() {
    let sk = StateKind::Fail(FailState::new(Some("Err".into()), None));
    assert!(sk.is_terminal());
}

#[test]
fn statekind_fail_has_no_transition() {
    let sk = StateKind::Fail(FailState::new(None, None));
    assert!(sk.transition().is_none());
}

#[test]
fn statekind_choice_is_not_terminal() {
    let cs = ChoiceState::new(vec![ChoiceRule::new(expr("$.x > 0"), sn("A"), None)], None)
        .expect("choice");
    let sk = StateKind::Choice(cs);
    assert!(!sk.is_terminal());
}

#[test]
fn statekind_choice_has_no_transition() {
    let cs = ChoiceState::new(vec![ChoiceRule::new(expr("$.x > 0"), sn("A"), None)], None)
        .expect("choice");
    let sk = StateKind::Choice(cs);
    assert!(sk.transition().is_none());
}

#[test]
fn statekind_pass_is_not_terminal_and_has_transition() {
    let sk = StateKind::Pass(PassState::new(None, end_t()));
    assert!(!sk.is_terminal());
    assert_eq!(sk.transition(), Some(&end_t()));
}

#[test]
fn statekind_wait_is_not_terminal_and_has_transition() {
    let ws = WaitState::new(WaitDuration::Seconds(10), end_t());
    let sk = StateKind::Wait(ws);
    assert!(!sk.is_terminal());
    assert_eq!(sk.transition(), Some(&end_t()));
}

#[test]
fn statekind_parallel_is_not_terminal_and_has_transition() {
    let inner = simple_machine();
    let ps = ParallelState::new(vec![inner], end_t(), None).expect("parallel");
    let sk = StateKind::Parallel(ps);
    assert!(!sk.is_terminal());
    assert_eq!(sk.transition(), Some(&end_t()));
}

#[test]
fn statekind_map_is_not_terminal_and_has_transition() {
    let inner = simple_machine();
    let ms = MapState::new(
        expr("$.items"),
        Box::new(inner),
        None,
        end_t(),
        vec![],
        vec![],
        None,
    )
    .expect("map");
    let sk = StateKind::Map(ms);
    assert!(!sk.is_terminal());
    assert_eq!(sk.transition(), Some(&end_t()));
}

// =========================================================================
// State construction tests
// =========================================================================

#[test]
fn state_with_all_shared_fields() {
    let state = State::new(StateKind::Task(make_task_state(next_t("Verify"))))
        .with_comment("Execute the build")
        .with_input_path(JsonPath::new("$.build").expect("jp"))
        .with_output_path(JsonPath::new("$.result").expect("jp"))
        .with_assign({
            let mut m = HashMap::new();
            m.insert(
                VariableName::new("build_id").expect("vn"),
                expr("$.context.id"),
            );
            m
        });
    assert_eq!(state.comment(), Some("Execute the build"));
    assert_eq!(state.input_path().map(|p| p.as_str()), Some("$.build"));
    assert_eq!(state.output_path().map(|p| p.as_str()), Some("$.result"));
    assert!(state.assign().is_some());
    assert!(matches!(state.kind(), StateKind::Task(_)));
}

#[test]
fn state_with_minimal_fields() {
    let state = State::new(StateKind::Succeed(SucceedState::new()));
    assert_eq!(state.comment(), None);
    assert_eq!(state.input_path(), None);
    assert_eq!(state.output_path(), None);
    assert_eq!(state.assign(), None);
    assert!(matches!(state.kind(), StateKind::Succeed(_)));
}

// =========================================================================
// StateMachine validation tests
// =========================================================================

#[test]
fn validate_valid_machine_ok() {
    let m = simple_machine();
    assert_eq!(m.validate(), Ok(()));
}

#[test]
fn validate_empty_states() {
    let m = StateMachine::new(sn("Init"), IndexMap::new());
    let errs = m.validate().unwrap_err();
    assert_eq!(errs.len(), 1);
    assert!(matches!(errs[0], StateMachineError::EmptyStates));
}

#[test]
fn validate_start_at_not_found() {
    let mut states = IndexMap::new();
    states.insert(sn("Init"), make_pass_next("Done"));
    states.insert(sn("Done"), make_succeed());
    let m = StateMachine::new(sn("Missing"), states);
    let errs = m.validate().unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        StateMachineError::StartAtNotFound { start_at } if start_at.as_str() == "Missing"
    )));
}

#[test]
fn validate_transition_target_not_found() {
    let mut states = IndexMap::new();
    states.insert(sn("Init"), make_pass_next("Ghost"));
    states.insert(sn("End"), make_succeed());
    let m = StateMachine::new(sn("Init"), states);
    let errs = m.validate().unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        StateMachineError::TransitionTargetNotFound { from, target }
            if from.as_str() == "Init" && target.as_str() == "Ghost"
    )));
}

#[test]
fn validate_choice_target_not_found() {
    let mut states = IndexMap::new();
    states.insert(sn("Router"), make_choice(&["Phantom"], None));
    states.insert(sn("End"), make_succeed());
    let m = StateMachine::new(sn("Router"), states);
    let errs = m.validate().unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        StateMachineError::ChoiceTargetNotFound { from, target }
            if from.as_str() == "Router" && target.as_str() == "Phantom"
    )));
}

#[test]
fn validate_default_target_not_found() {
    let mut states = IndexMap::new();
    states.insert(sn("Router"), make_choice(&["End"], Some("Ghost")));
    states.insert(sn("End"), make_succeed());
    let m = StateMachine::new(sn("Router"), states);
    let errs = m.validate().unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        StateMachineError::DefaultTargetNotFound { from, target }
            if from.as_str() == "Router" && target.as_str() == "Ghost"
    )));
}

#[test]
fn validate_no_terminal_state() {
    let mut states = IndexMap::new();
    states.insert(sn("A"), make_pass_next("B"));
    states.insert(sn("B"), make_pass_next("A"));
    let m = StateMachine::new(sn("A"), states);
    let errs = m.validate().unwrap_err();
    assert!(errs
        .iter()
        .any(|e| matches!(e, StateMachineError::NoTerminalState)));
}

#[test]
fn validate_multiple_errors_collected() {
    let mut states = IndexMap::new();
    states.insert(sn("A"), make_pass_next("Ghost1"));
    states.insert(sn("B"), make_pass_next("Ghost2"));
    let m = StateMachine::new(sn("Missing"), states);
    let errs = m.validate().unwrap_err();
    assert_eq!(errs.len(), 4);
    assert!(errs
        .iter()
        .any(|e| matches!(e, StateMachineError::StartAtNotFound { .. })));
    assert!(errs.iter().any(
        |e| matches!(e, StateMachineError::TransitionTargetNotFound { from, target }
                if from.as_str() == "A" && target.as_str() == "Ghost1"
        )
    ));
    assert!(errs.iter().any(
        |e| matches!(e, StateMachineError::TransitionTargetNotFound { from, target }
                if from.as_str() == "B" && target.as_str() == "Ghost2"
        )
    ));
    assert!(errs
        .iter()
        .any(|e| matches!(e, StateMachineError::NoTerminalState)));
}

#[test]
fn validate_transition_end_counts_as_terminal() {
    let mut states = IndexMap::new();
    states.insert(sn("Init"), make_pass_end());
    let m = StateMachine::new(sn("Init"), states);
    assert_eq!(m.validate(), Ok(()));
}

#[test]
fn validate_fail_state_is_terminal() {
    let mut states = IndexMap::new();
    states.insert(sn("Init"), make_pass_next("Boom"));
    states.insert(sn("Boom"), make_fail());
    let m = StateMachine::new(sn("Init"), states);
    assert_eq!(m.validate(), Ok(()));
}

// =========================================================================
// get_state / start_state tests
// =========================================================================

#[test]
fn get_state_found() {
    let m = simple_machine();
    let state = m.get_state(&sn("Init"));
    assert!(state.is_some());
    assert!(matches!(state.unwrap().kind(), StateKind::Pass(_)));
}

#[test]
fn get_state_not_found() {
    let m = simple_machine();
    assert_eq!(m.get_state(&sn("Unknown")), None);
}

#[test]
fn start_state_returns_first() {
    let m = simple_machine();
    let s = m.start_state().expect("start state must exist");
    assert!(matches!(s.kind(), StateKind::Pass(_)));
}

// =========================================================================
// Serde round-trip tests
// =========================================================================

#[test]
fn state_serde_roundtrip_succeed() {
    let json = r#"{"type":"succeed"}"#;
    let state: State = serde_json::from_str(json).expect("deser");
    assert!(matches!(state.kind(), StateKind::Succeed(_)));
    assert_eq!(state.comment(), None);
    let out = serde_json::to_string(&state).expect("ser");
    let state2: State = serde_json::from_str(&out).expect("roundtrip deser");
    assert_eq!(state, state2);
}

#[test]
fn state_serde_roundtrip_fail() {
    let json = r#"{"type":"fail","error":"Boom","cause":"bad"}"#;
    let state: State = serde_json::from_str(json).expect("deser");
    assert!(matches!(state.kind(), StateKind::Fail(_)));
    let out = serde_json::to_string(&state).expect("ser");
    let state2: State = serde_json::from_str(&out).expect("roundtrip deser");
    assert_eq!(state, state2);
}

#[test]
fn state_serde_roundtrip_pass_with_comment() {
    let json = r#"{"type":"pass","comment":"hello","next":"Done"}"#;
    let state: State = serde_json::from_str(json).expect("deser");
    assert_eq!(state.comment(), Some("hello"));
    assert!(matches!(state.kind(), StateKind::Pass(_)));
}

#[test]
fn state_serde_unknown_type_rejected() {
    let json = r#"{"type":"unknown_state_type"}"#;
    let result = serde_json::from_str::<State>(json);
    assert!(
        result.is_err(),
        "unknown state type should be rejected: {result:?}"
    );
}

#[test]
fn statemachine_serde_roundtrip_preserves_order() {
    let json = r#"{
        "startAt": "First",
        "states": {
            "First": { "type": "pass", "next": "Second" },
            "Second": { "type": "pass", "next": "Third" },
            "Third": { "type": "succeed" }
        }
    }"#;
    let m: StateMachine = serde_json::from_str(json).expect("deser");
    assert_eq!(m.validate(), Ok(()));
    let keys: Vec<&str> = m.states().keys().map(|k| k.as_str()).collect();
    assert_eq!(keys, vec!["First", "Second", "Third"]);

    let out = serde_json::to_string(&m).expect("ser");
    let m2: StateMachine = serde_json::from_str(&out).expect("roundtrip deser");
    let keys2: Vec<&str> = m2.states().keys().map(|k| k.as_str()).collect();
    assert_eq!(keys2, vec!["First", "Second", "Third"]);
    assert_eq!(m, m2);
}

#[test]
fn statemachine_serde_with_parallel_branches() {
    let json = r#"{
        "startAt": "Fork",
        "states": {
            "Fork": {
                "type": "parallel",
                "branches": [
                    { "startAt": "SubA", "states": { "SubA": { "type": "succeed" } } },
                    { "startAt": "SubB", "states": { "SubB": { "type": "succeed" } } }
                ],
                "next": "Join"
            },
            "Join": { "type": "succeed" }
        }
    }"#;
    let m: StateMachine = serde_json::from_str(json).expect("deser");
    assert_eq!(m.validate(), Ok(()));
    // Sub-machine validation is independent
    if let StateKind::Parallel(ref ps) = m.get_state(&sn("Fork")).unwrap().kind() {
        assert_eq!(ps.branches().len(), 2);
        assert_eq!(ps.branches()[0].validate(), Ok(()));
        assert_eq!(ps.branches()[1].validate(), Ok(()));
    } else {
        panic!("expected ParallelState");
    }
}

#[test]
fn statemachine_serde_with_map_item_processor() {
    let json = r#"{
        "startAt": "MapIt",
        "states": {
            "MapIt": {
                "type": "map",
                "itemsPath": "$.items",
                "itemProcessor": {
                    "startAt": "Process",
                    "states": { "Process": { "type": "succeed" } }
                },
                "end": true
            }
        }
    }"#;
    let m: StateMachine = serde_json::from_str(json).expect("deser");
    assert_eq!(m.validate(), Ok(()));
    if let StateKind::Map(ref ms) = m.get_state(&sn("MapIt")).unwrap().kind() {
        assert_eq!(ms.item_processor().validate(), Ok(()));
    } else {
        panic!("expected MapState");
    }
}

#[test]
fn statemachine_serde_with_timeout() {
    let json = r#"{
        "startAt": "Go",
        "states": { "Go": { "type": "succeed" } },
        "timeout": 3600
    }"#;
    let m: StateMachine = serde_json::from_str(json).expect("deser");
    assert_eq!(m.timeout(), Some(3600));
}
