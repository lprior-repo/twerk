//! Tests for ASL state variant types: Choice, Wait, Pass, Terminal.
//! Covers all Given-When-Then scenarios from the twerk-snj contract.

use std::collections::HashMap;

use serde_json::json;
use twerk_core::asl::choice::{ChoiceRule, ChoiceState, ChoiceStateError};
use twerk_core::asl::pass::PassState;
use twerk_core::asl::terminal::{FailState, SucceedState};
use twerk_core::asl::wait::{WaitDuration, WaitState};
use twerk_core::asl::{Expression, JsonPath, StateName, Transition, VariableName};

// =========================================================================
// ChoiceRule
// =========================================================================

/// CR-1: Construct valid ChoiceRule without assign
#[test]
fn choice_rule_new_without_assign() {
    let cond = Expression::new("$.value > 10").unwrap();
    let next = StateName::new("HighValuePath").unwrap();
    let rule = ChoiceRule::new(cond.clone(), next.clone(), None);
    assert_eq!(rule.condition(), &cond);
    assert_eq!(rule.next(), &next);
    assert!(rule.assign().is_none());
}

/// CR-2: Construct valid ChoiceRule with assign
#[test]
fn choice_rule_new_with_assign() {
    let cond = Expression::new("$.ready == true").unwrap();
    let next = StateName::new("Process").unwrap();
    let mut map = HashMap::new();
    map.insert(
        VariableName::new("result").unwrap(),
        Expression::new("$.output").unwrap(),
    );
    let rule = ChoiceRule::new(cond, next, Some(map));
    let assign = rule.assign().expect("assign should be Some");
    assert_eq!(assign.len(), 1);
}

/// CR-3: Serialize ChoiceRule roundtrip (no assign)
#[test]
fn choice_rule_serde_roundtrip_no_assign() {
    let cond = Expression::new("$.x > 0").unwrap();
    let next = StateName::new("Positive").unwrap();
    let rule = ChoiceRule::new(cond, next, None);

    let json = serde_json::to_string(&rule).unwrap();
    assert!(json.contains("\"condition\""));
    assert!(json.contains("\"next\""));
    assert!(!json.contains("\"assign\""));

    let deser: ChoiceRule = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, rule);
}

/// CR-4: Serialize ChoiceRule with assign roundtrip
#[test]
fn choice_rule_serde_roundtrip_with_assign() {
    let cond = Expression::new("$.x > 0").unwrap();
    let next = StateName::new("Pos").unwrap();
    let mut map = HashMap::new();
    map.insert(
        VariableName::new("out").unwrap(),
        Expression::new("$.result").unwrap(),
    );
    let rule = ChoiceRule::new(cond, next, Some(map));

    let json = serde_json::to_string(&rule).unwrap();
    assert!(json.contains("\"assign\""));

    let deser: ChoiceRule = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, rule);
}

// =========================================================================
// ChoiceState
// =========================================================================

fn make_rule(cond: &str, next: &str) -> ChoiceRule {
    ChoiceRule::new(
        Expression::new(cond).unwrap(),
        StateName::new(next).unwrap(),
        None,
    )
}

/// CS-1: Construct valid ChoiceState with one rule, no default
#[test]
fn choice_state_one_rule_no_default() {
    let cs = ChoiceState::new(vec![make_rule("$.a", "A")], None).unwrap();
    assert_eq!(cs.choices().len(), 1);
    assert!(cs.default().is_none());
}

/// CS-2: Construct valid ChoiceState with multiple rules and default
#[test]
fn choice_state_multiple_rules_with_default() {
    let rules = vec![
        make_rule("$.a", "A"),
        make_rule("$.b", "B"),
        make_rule("$.c", "C"),
    ];
    let default = StateName::new("Fallback").unwrap();
    let cs = ChoiceState::new(rules, Some(default.clone())).unwrap();
    assert_eq!(cs.choices().len(), 3);
    assert_eq!(cs.default(), Some(&default));
}

/// CS-3: Reject empty choices
#[test]
fn choice_state_rejects_empty_choices() {
    let err = ChoiceState::new(vec![], None).unwrap_err();
    assert_eq!(err, ChoiceStateError::EmptyChoices);
}

/// CS-4: Deserialize valid ChoiceState
#[test]
fn choice_state_deserialize_valid() {
    let json = r#"{"choices": [{"condition": "$.x > 0", "next": "Pos"}], "default": "Neg"}"#;
    let cs: ChoiceState = serde_json::from_str(json).unwrap();
    assert_eq!(cs.choices().len(), 1);
    assert_eq!(cs.default().unwrap().as_str(), "Neg");
}

/// CS-5: Reject deserialization with empty choices array
#[test]
fn choice_state_deserialize_rejects_empty_choices() {
    let json = r#"{"choices": []}"#;
    let err = serde_json::from_str::<ChoiceState>(json).unwrap_err();
    assert!(err.to_string().contains("at least one"));
}

/// CS-6: Serialize ChoiceState roundtrip
#[test]
fn choice_state_serde_roundtrip() {
    let rules = vec![make_rule("$.a", "A"), make_rule("$.b", "B")];
    let default = StateName::new("Fallback").unwrap();
    let cs = ChoiceState::new(rules, Some(default)).unwrap();

    let json = serde_json::to_string(&cs).unwrap();
    let deser: ChoiceState = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, cs);
}

// =========================================================================
// WaitDuration
// =========================================================================

/// WD-1: Seconds variant
#[test]
fn wait_duration_seconds() {
    let wd = WaitDuration::Seconds(30);
    assert!(wd.is_seconds());
    assert!(!wd.is_timestamp());
    assert!(!wd.is_seconds_path());
    assert!(!wd.is_timestamp_path());
}

/// WD-2: Timestamp variant
#[test]
fn wait_duration_timestamp() {
    let wd = WaitDuration::Timestamp("2024-12-31T23:59:59Z".into());
    assert!(wd.is_timestamp());
    assert!(!wd.is_seconds());
    assert!(!wd.is_seconds_path());
    assert!(!wd.is_timestamp_path());
}

/// WD-3: SecondsPath variant
#[test]
fn wait_duration_seconds_path() {
    let path = JsonPath::new("$.config.delay").unwrap();
    let wd = WaitDuration::SecondsPath(path);
    assert!(wd.is_seconds_path());
    assert!(!wd.is_seconds());
    assert!(!wd.is_timestamp());
    assert!(!wd.is_timestamp_path());
}

/// WD-4: TimestampPath variant
#[test]
fn wait_duration_timestamp_path() {
    let path = JsonPath::new("$.schedule.when").unwrap();
    let wd = WaitDuration::TimestampPath(path);
    assert!(wd.is_timestamp_path());
    assert!(!wd.is_seconds());
    assert!(!wd.is_timestamp());
    assert!(!wd.is_seconds_path());
}

/// WD-5: Deserialize seconds
#[test]
fn wait_duration_deser_seconds() {
    let json = r#"{"seconds": 60}"#;
    let wd: WaitDuration = serde_json::from_str(json).unwrap();
    assert_eq!(wd, WaitDuration::Seconds(60));
}

/// WD-6: Deserialize timestamp
#[test]
fn wait_duration_deser_timestamp() {
    let json = r#"{"timestamp": "2024-01-01T00:00:00Z"}"#;
    let wd: WaitDuration = serde_json::from_str(json).unwrap();
    assert_eq!(wd, WaitDuration::Timestamp("2024-01-01T00:00:00Z".into()));
}

/// WD-7: Deserialize seconds_path
#[test]
fn wait_duration_deser_seconds_path() {
    let json = r#"{"seconds_path": "$.delay"}"#;
    let wd: WaitDuration = serde_json::from_str(json).unwrap();
    assert_eq!(
        wd,
        WaitDuration::SecondsPath(JsonPath::new("$.delay").unwrap())
    );
}

/// WD-8: Deserialize timestamp_path
#[test]
fn wait_duration_deser_timestamp_path() {
    let json = r#"{"timestamp_path": "$.when"}"#;
    let wd: WaitDuration = serde_json::from_str(json).unwrap();
    assert_eq!(
        wd,
        WaitDuration::TimestampPath(JsonPath::new("$.when").unwrap())
    );
}

/// WD-9: Reject no duration field
#[test]
fn wait_duration_rejects_no_field() {
    let json = r#"{}"#;
    let err = serde_json::from_str::<WaitDuration>(json).unwrap_err();
    assert!(err.to_string().contains("exactly one"));
}

/// WD-10: Reject multiple duration fields
#[test]
fn wait_duration_rejects_multiple_fields() {
    let json = r#"{"seconds": 10, "timestamp": "2024-01-01T00:00:00Z"}"#;
    let err = serde_json::from_str::<WaitDuration>(json).unwrap_err();
    assert!(err.to_string().contains("multiple fields"));
}

/// WD-11: Reject empty timestamp string
#[test]
fn wait_duration_rejects_empty_timestamp() {
    let json = r#"{"timestamp": ""}"#;
    let err = serde_json::from_str::<WaitDuration>(json).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("empty"));
}

/// WD-12: Reject invalid JsonPath in seconds_path
#[test]
fn wait_duration_rejects_invalid_json_path() {
    let json = r#"{"seconds_path": "no-dollar"}"#;
    let err = serde_json::from_str::<WaitDuration>(json).unwrap_err();
    assert!(err.to_string().contains("$"));
}

/// WD-13: Serialize seconds roundtrip
#[test]
fn wait_duration_serde_roundtrip_seconds() {
    let wd = WaitDuration::Seconds(10);
    let json = serde_json::to_string(&wd).unwrap();
    let deser: WaitDuration = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, WaitDuration::Seconds(10));
}

/// WD-14: Serialize timestamp_path roundtrip
#[test]
fn wait_duration_serde_roundtrip_timestamp_path() {
    let path = JsonPath::new("$.schedule").unwrap();
    let wd = WaitDuration::TimestampPath(path);
    let json = serde_json::to_string(&wd).unwrap();
    let deser: WaitDuration = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, wd);
}

/// WD-15: Display formatting
#[test]
fn wait_duration_display() {
    assert_eq!(format!("{}", WaitDuration::Seconds(30)), "seconds: 30");
    assert_eq!(
        format!("{}", WaitDuration::Timestamp("2024-01-01T00:00:00Z".into())),
        "timestamp: 2024-01-01T00:00:00Z"
    );
    assert_eq!(
        format!(
            "{}",
            WaitDuration::SecondsPath(JsonPath::new("$.delay").unwrap())
        ),
        "seconds_path: $.delay"
    );
    assert_eq!(
        format!(
            "{}",
            WaitDuration::TimestampPath(JsonPath::new("$.when").unwrap())
        ),
        "timestamp_path: $.when"
    );
}

// =========================================================================
// WaitState
// =========================================================================

/// WS-1: Construct WaitState with seconds and next
#[test]
fn wait_state_seconds_and_next() {
    let dur = WaitDuration::Seconds(10);
    let trans = Transition::next(StateName::new("ProcessResult").unwrap());
    let ws = WaitState::new(dur.clone(), trans.clone());
    assert_eq!(ws.duration(), &dur);
    assert_eq!(ws.transition(), &trans);
}

/// WS-2: Construct WaitState with timestamp and end
#[test]
fn wait_state_timestamp_and_end() {
    let dur = WaitDuration::Timestamp("2024-12-31T23:59:59Z".into());
    let ws = WaitState::new(dur.clone(), Transition::end());
    assert_eq!(ws.duration(), &dur);
    assert_eq!(ws.transition(), &Transition::End);
}

/// WS-3: Deserialize flattened WaitState with seconds + next
#[test]
fn wait_state_deser_seconds_next() {
    let json = r#"{"seconds": 10, "next": "Step2"}"#;
    let ws: WaitState = serde_json::from_str(json).unwrap();
    assert_eq!(ws.duration(), &WaitDuration::Seconds(10));
    assert_eq!(
        ws.transition(),
        &Transition::next(StateName::new("Step2").unwrap())
    );
}

/// WS-4: Deserialize flattened WaitState with timestamp_path + end
#[test]
fn wait_state_deser_timestamp_path_end() {
    let json = r#"{"timestamp_path": "$.when", "end": true}"#;
    let ws: WaitState = serde_json::from_str(json).unwrap();
    assert_eq!(
        ws.duration(),
        &WaitDuration::TimestampPath(JsonPath::new("$.when").unwrap())
    );
    assert_eq!(ws.transition(), &Transition::End);
}

/// WS-5: Reject WaitState with no duration fields
#[test]
fn wait_state_rejects_no_duration() {
    let json = r#"{"next": "Step2"}"#;
    let err = serde_json::from_str::<WaitState>(json).unwrap_err();
    assert!(err.to_string().contains("exactly one"));
}

/// WS-6: Reject WaitState with no transition fields
#[test]
fn wait_state_rejects_no_transition() {
    let json = r#"{"seconds": 10}"#;
    let err = serde_json::from_str::<WaitState>(json).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("next") || msg.contains("end") || msg.contains("transition"));
}

/// WS-7: Serialize WaitState roundtrip
#[test]
fn wait_state_serde_roundtrip() {
    let ws = WaitState::new(
        WaitDuration::Seconds(5),
        Transition::next(StateName::new("Done").unwrap()),
    );
    let json = serde_json::to_string(&ws).unwrap();
    let deser: WaitState = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, ws);
    assert!(json.contains("\"seconds\":5"));
    assert!(json.contains("\"next\":\"Done\""));
}

// =========================================================================
// PassState
// =========================================================================

/// PS-1: Construct PassState with result and next
#[test]
fn pass_state_with_result_and_next() {
    let result = Some(json!({"key": "value"}));
    let trans = Transition::next(StateName::new("Process").unwrap());
    let ps = PassState::new(result.clone(), trans.clone());
    assert_eq!(ps.result(), result.as_ref());
    assert_eq!(ps.transition(), &trans);
}

/// PS-2: Construct PassState with no result and end
#[test]
fn pass_state_no_result_end() {
    let ps = PassState::new(None, Transition::end());
    assert!(ps.result().is_none());
    assert_eq!(ps.transition(), &Transition::End);
}

/// PS-3: Deserialize PassState with result
#[test]
fn pass_state_deser_with_result() {
    let json = r#"{"result": {"output": 42}, "next": "Step2"}"#;
    let ps: PassState = serde_json::from_str(json).unwrap();
    assert_eq!(ps.result(), Some(&json!({"output": 42})));
    assert_eq!(
        ps.transition(),
        &Transition::next(StateName::new("Step2").unwrap())
    );
}

/// PS-4: Deserialize PassState without result
#[test]
fn pass_state_deser_without_result() {
    let json = r#"{"end": true}"#;
    let ps: PassState = serde_json::from_str(json).unwrap();
    assert!(ps.result().is_none());
    assert_eq!(ps.transition(), &Transition::End);
}

/// PS-5: Serialize PassState roundtrip (with result)
#[test]
fn pass_state_serde_roundtrip_with_result() {
    let ps = PassState::new(
        Some(json!("hello")),
        Transition::next(StateName::new("Done").unwrap()),
    );
    let json = serde_json::to_string(&ps).unwrap();
    assert!(json.contains("\"result\""));
    let deser: PassState = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, ps);
}

/// PS-6: Serialize PassState roundtrip (without result)
#[test]
fn pass_state_serde_roundtrip_without_result() {
    let ps = PassState::new(None, Transition::end());
    let json = serde_json::to_string(&ps).unwrap();
    assert!(!json.contains("\"result\""));
    assert_eq!(json, r#"{"end":true}"#);
}

// =========================================================================
// SucceedState
// =========================================================================

/// SS-1: Construct SucceedState
#[test]
fn succeed_state_construct() {
    let _s = SucceedState::new();
    let _d = SucceedState;
}

/// SS-2: Serialize SucceedState
#[test]
fn succeed_state_serialize() {
    let json = serde_json::to_string(&SucceedState::new()).unwrap();
    assert_eq!(json, "{}");
}

/// SS-3: Deserialize SucceedState
#[test]
fn succeed_state_deserialize() {
    let ss: SucceedState = serde_json::from_str("{}").unwrap();
    assert_eq!(ss, SucceedState::new());
}

/// SS-4: SucceedState equality
#[test]
fn succeed_state_equality() {
    let s1 = SucceedState::new();
    let s2 = SucceedState::new();
    assert_eq!(s1, s2);
}

/// SS-5: SucceedState is Copy
#[test]
fn succeed_state_is_copy() {
    let s1 = SucceedState::new();
    let s2 = s1; // copy
    assert_eq!(s1, s2); // s1 still usable
}

// =========================================================================
// FailState
// =========================================================================

/// FS-1: Construct FailState with both error and cause
#[test]
fn fail_state_both_fields() {
    let fs = FailState::new(Some("TaskFailed".into()), Some("Lambda timed out".into()));
    assert_eq!(fs.error(), Some("TaskFailed"));
    assert_eq!(fs.cause(), Some("Lambda timed out"));
}

/// FS-2: Construct FailState with only error
#[test]
fn fail_state_only_error() {
    let fs = FailState::new(Some("InternalError".into()), None);
    assert_eq!(fs.error(), Some("InternalError"));
    assert!(fs.cause().is_none());
}

/// FS-3: Construct FailState with only cause
#[test]
fn fail_state_only_cause() {
    let fs = FailState::new(None, Some("unexpected EOF".into()));
    assert!(fs.error().is_none());
    assert_eq!(fs.cause(), Some("unexpected EOF"));
}

/// FS-4: Construct empty FailState
#[test]
fn fail_state_empty() {
    let fs = FailState::new(None, None);
    assert!(fs.error().is_none());
    assert!(fs.cause().is_none());
}

/// FS-5: Deserialize FailState with both fields
#[test]
fn fail_state_deser_both() {
    let json = r#"{"error": "MyError", "cause": "something broke"}"#;
    let fs: FailState = serde_json::from_str(json).unwrap();
    assert_eq!(fs.error(), Some("MyError"));
    assert_eq!(fs.cause(), Some("something broke"));
}

/// FS-6: Deserialize empty FailState
#[test]
fn fail_state_deser_empty() {
    let fs: FailState = serde_json::from_str("{}").unwrap();
    assert!(fs.error().is_none());
    assert!(fs.cause().is_none());
}

/// FS-7: Serialize FailState skips None fields
#[test]
fn fail_state_serialize_skips_none() {
    let fs = FailState::new(Some("E".into()), None);
    let json = serde_json::to_string(&fs).unwrap();
    assert_eq!(json, r#"{"error":"E"}"#);
    assert!(!json.contains("cause"));
}

/// FS-8: Serialize FailState roundtrip
#[test]
fn fail_state_serde_roundtrip() {
    let fs = FailState::new(Some("TaskFailed".into()), Some("timeout".into()));
    let json = serde_json::to_string(&fs).unwrap();
    let deser: FailState = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, fs);
}

/// FS-9: Display formatting
#[test]
fn fail_state_display() {
    assert_eq!(
        format!(
            "{}",
            FailState::new(Some("TaskFailed".into()), Some("timeout".into()))
        ),
        "FAIL: TaskFailed (timeout)"
    );
    assert_eq!(
        format!("{}", FailState::new(Some("E".into()), None)),
        "FAIL: E"
    );
    assert_eq!(
        format!("{}", FailState::new(None, Some("oops".into()))),
        "FAIL: (oops)"
    );
    assert_eq!(format!("{}", FailState::new(None, None)), "FAIL");
}

/// FS-10: FailState equality
#[test]
fn fail_state_equality() {
    let f1 = FailState::new(Some("E".into()), Some("C".into()));
    let f2 = FailState::new(Some("E".into()), Some("C".into()));
    assert_eq!(f1, f2);

    let f3 = FailState::new(Some("E1".into()), None);
    let f4 = FailState::new(Some("E2".into()), None);
    assert_ne!(f3, f4);
}
