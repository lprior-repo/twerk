//! Exhaustive tests for ASL Transition, Retrier, and Catcher types (twerk-9xv).

use std::collections::HashMap;

use twerk_core::asl::catcher::{Catcher, CatcherError};
use twerk_core::asl::error_code::ErrorCode;
use twerk_core::asl::retrier::{JitterStrategy, Retrier, RetrierError};
use twerk_core::asl::transition::Transition;
use twerk_core::asl::types::{BackoffRate, Expression, JsonPath, StateName, VariableName};

// ===========================================================================
// Transition
// ===========================================================================

mod transition_tests {
    use super::*;

    // -- TR-1: Construct Next variant ------------------------------------

    #[test]
    fn tr1_construct_next() {
        let name = StateName::new("ProcessOrder").expect("valid name");
        let t = Transition::next(name.clone());
        assert!(t.is_next());
        assert!(!t.is_end());
        assert_eq!(t.target_state(), Some(&name));
    }

    // -- TR-2: Construct End variant ------------------------------------

    #[test]
    fn tr2_construct_end() {
        let t = Transition::end();
        assert!(!t.is_next());
        assert!(t.is_end());
        assert_eq!(t.target_state(), None);
    }

    // -- TR-3: Deserialize next transition (YAML) -----------------------

    #[test]
    fn tr3_deserialize_next_yaml() {
        let yaml = "next: ProcessOrder";
        let t: Transition = serde_saphyr::from_str(yaml).expect("deser next");
        let expected = Transition::next(StateName::new("ProcessOrder").unwrap());
        assert_eq!(t, expected);
    }

    // -- TR-4: Deserialize end transition (YAML) ------------------------

    #[test]
    fn tr4_deserialize_end_yaml() {
        let yaml = "end: true";
        let t: Transition = serde_saphyr::from_str(yaml).expect("deser end");
        assert_eq!(t, Transition::end());
    }

    // -- TR-5: Reject both next and end ---------------------------------

    #[test]
    fn tr5_reject_both_next_and_end() {
        let json = r#"{"next": "Foo", "end": true}"#;
        let err = serde_json::from_str::<Transition>(json).unwrap_err();
        assert!(
            err.to_string().contains("both"),
            "expected 'both' in error: {err}"
        );
    }

    // -- TR-6: Reject neither next nor end ------------------------------

    #[test]
    fn tr6_reject_neither() {
        let json = "{}";
        let err = serde_json::from_str::<Transition>(json).unwrap_err();
        assert!(
            err.to_string().contains("neither"),
            "expected 'neither' in error: {err}"
        );
    }

    // -- TR-7: Reject end: false ----------------------------------------

    #[test]
    fn tr7_reject_end_false() {
        let json = r#"{"end": false}"#;
        let err = serde_json::from_str::<Transition>(json).unwrap_err();
        assert!(
            err.to_string().contains("must be true"),
            "expected 'must be true' in error: {err}"
        );
    }

    // -- TR-8: Reject invalid state name in next ------------------------

    #[test]
    fn tr8_reject_invalid_state_name() {
        let json = r#"{"next": ""}"#;
        let err = serde_json::from_str::<Transition>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("empty") || msg.contains("state name"),
            "expected state name error: {msg}"
        );
    }

    // -- TR-9: Serialize Next roundtrip ---------------------------------

    #[test]
    fn tr9_serialize_next_roundtrip() {
        let t = Transition::next(StateName::new("Foo").unwrap());
        let json = serde_json::to_string(&t).expect("ser");
        assert_eq!(json, r#"{"next":"Foo"}"#);
        let back: Transition = serde_json::from_str(&json).expect("deser");
        assert_eq!(back, t);
    }

    // -- TR-10: Serialize End roundtrip ---------------------------------

    #[test]
    fn tr10_serialize_end_roundtrip() {
        let t = Transition::end();
        let json = serde_json::to_string(&t).expect("ser");
        assert_eq!(json, r#"{"end":true}"#);
        let back: Transition = serde_json::from_str(&json).expect("deser");
        assert_eq!(back, Transition::end());
    }

    // -- TR-11: Display formatting --------------------------------------

    #[test]
    fn tr11_display() {
        let next = Transition::next(StateName::new("Step2").unwrap());
        assert_eq!(format!("{next}"), "-> Step2");

        let end = Transition::end();
        assert_eq!(format!("{end}"), "END");
    }

    // -- TR-12: Equality ------------------------------------------------

    #[test]
    fn tr12_equality() {
        let end1 = Transition::end();
        let end2 = Transition::end();
        assert_eq!(end1, end2);

        let name = StateName::new("A").unwrap();
        let next1 = Transition::next(name.clone());
        let next2 = Transition::next(name);
        assert_eq!(next1, next2);

        assert_ne!(next1, end1);
    }
}

// ===========================================================================
// JitterStrategy
// ===========================================================================

mod jitter_strategy_tests {
    use super::*;

    // -- JS-1: Default is None ------------------------------------------

    #[test]
    fn js1_default_is_none() {
        assert_eq!(JitterStrategy::default(), JitterStrategy::None);
    }

    // -- JS-2: Serialize Full -------------------------------------------

    #[test]
    fn js2_serialize_full() {
        let json = serde_json::to_string(&JitterStrategy::Full).expect("ser");
        assert_eq!(json, r#""FULL""#);
    }

    // -- JS-3: Serialize None -------------------------------------------

    #[test]
    fn js3_serialize_none() {
        let json = serde_json::to_string(&JitterStrategy::None).expect("ser");
        assert_eq!(json, r#""NONE""#);
    }

    // -- JS-4: Deserialize FULL -----------------------------------------

    #[test]
    fn js4_deserialize_full() {
        let js: JitterStrategy = serde_json::from_str(r#""FULL""#).expect("deser");
        assert_eq!(js, JitterStrategy::Full);
    }

    // -- JS-5: Deserialize NONE -----------------------------------------

    #[test]
    fn js5_deserialize_none() {
        let js: JitterStrategy = serde_json::from_str(r#""NONE""#).expect("deser");
        assert_eq!(js, JitterStrategy::None);
    }

    // -- JS-6: Reject unknown string ------------------------------------

    #[test]
    fn js6_reject_unknown() {
        let result = serde_json::from_str::<JitterStrategy>(r#""HALF""#);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown variant"), "{err}");
    }

    // -- JS-7: Reject lowercase (case-sensitive) ------------------------

    #[test]
    fn js7_reject_lowercase() {
        let result = serde_json::from_str::<JitterStrategy>(r#""full""#);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown variant"), "{err}");
    }

    // -- JS-8: Display --------------------------------------------------

    #[test]
    fn js8_display() {
        assert_eq!(format!("{}", JitterStrategy::Full), "FULL");
        assert_eq!(format!("{}", JitterStrategy::None), "NONE");
    }

    // -- JS-9: Serde roundtrip ------------------------------------------

    #[test]
    fn js9_roundtrip() {
        for js in [JitterStrategy::Full, JitterStrategy::None] {
            let json = serde_json::to_string(&js).unwrap();
            let back: JitterStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(back, js);
        }
    }
}

// ===========================================================================
// Retrier
// ===========================================================================

mod retrier_tests {
    use super::*;

    fn default_backoff() -> BackoffRate {
        BackoffRate::new(2.0).unwrap()
    }

    fn default_errors() -> Vec<ErrorCode> {
        vec![ErrorCode::Timeout]
    }

    // -- RT-1: Valid construction with all fields -----------------------

    #[test]
    fn rt1_valid_all_fields() {
        let r = Retrier::new(
            vec![ErrorCode::Timeout, ErrorCode::TaskFailed],
            2,
            3,
            BackoffRate::new(2.0).unwrap(),
            Some(30),
            JitterStrategy::Full,
        )
        .expect("valid retrier");

        assert_eq!(
            r.error_equals(),
            &[ErrorCode::Timeout, ErrorCode::TaskFailed]
        );
        assert_eq!(r.interval_seconds(), 2);
        assert_eq!(r.max_attempts(), 3);
        assert_eq!(r.backoff_rate().value(), 2.0);
        assert_eq!(r.max_delay_seconds(), Some(30));
        assert_eq!(r.jitter_strategy(), JitterStrategy::Full);
    }

    // -- RT-2: Valid construction without optional fields ----------------

    #[test]
    fn rt2_valid_minimal() {
        let r = Retrier::new(
            vec![ErrorCode::All],
            1,
            1,
            BackoffRate::new(1.0).unwrap(),
            None,
            JitterStrategy::None,
        )
        .expect("valid retrier");

        assert_eq!(r.max_delay_seconds(), None);
        assert_eq!(r.jitter_strategy(), JitterStrategy::None);
    }

    // -- RT-3: Reject empty error_equals --------------------------------

    #[test]
    fn rt3_reject_empty_errors() {
        let err =
            Retrier::new(vec![], 1, 1, default_backoff(), None, JitterStrategy::None).unwrap_err();
        assert_eq!(err, RetrierError::EmptyErrorEquals);
    }

    // -- RT-4: Reject interval_seconds = 0 ------------------------------

    #[test]
    fn rt4_reject_interval_zero() {
        let err = Retrier::new(
            default_errors(),
            0,
            3,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .unwrap_err();
        assert_eq!(err, RetrierError::IntervalTooSmall(0));
    }

    // -- RT-5: Reject max_attempts = 0 ----------------------------------

    #[test]
    fn rt5_reject_max_attempts_zero() {
        let err = Retrier::new(
            default_errors(),
            1,
            0,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .unwrap_err();
        assert_eq!(err, RetrierError::MaxAttemptsTooSmall(0));
    }

    // -- RT-6: Reject max_delay_seconds == interval_seconds --------------

    #[test]
    fn rt6_reject_max_delay_equal_interval() {
        let err = Retrier::new(
            default_errors(),
            5,
            3,
            default_backoff(),
            Some(5),
            JitterStrategy::None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            RetrierError::MaxDelayNotGreaterThanInterval {
                max_delay: 5,
                interval: 5
            }
        );
    }

    // -- RT-7: Reject max_delay_seconds < interval_seconds ---------------

    #[test]
    fn rt7_reject_max_delay_less_than_interval() {
        let err = Retrier::new(
            default_errors(),
            10,
            3,
            default_backoff(),
            Some(3),
            JitterStrategy::None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            RetrierError::MaxDelayNotGreaterThanInterval {
                max_delay: 3,
                interval: 10
            }
        );
    }

    // -- RT-8: Boundary -- interval_seconds = 1 -------------------------

    #[test]
    fn rt8_boundary_interval_one() {
        let r = Retrier::new(
            default_errors(),
            1,
            3,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .expect("valid");
        assert_eq!(r.interval_seconds(), 1);
    }

    // -- RT-9: Boundary -- max_attempts = 1 -----------------------------

    #[test]
    fn rt9_boundary_max_attempts_one() {
        let r = Retrier::new(
            default_errors(),
            1,
            1,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .expect("valid");
        assert_eq!(r.max_attempts(), 1);
    }

    // -- RT-10: Boundary -- max_delay_seconds just above interval --------

    #[test]
    fn rt10_boundary_max_delay_just_above() {
        let r = Retrier::new(
            default_errors(),
            5,
            3,
            default_backoff(),
            Some(6),
            JitterStrategy::None,
        )
        .expect("valid");
        assert_eq!(r.max_delay_seconds(), Some(6));
    }

    // -- RT-11: Serde roundtrip (JSON) ----------------------------------

    #[test]
    fn rt11_serde_roundtrip_json() {
        let r = Retrier::new(
            vec![ErrorCode::Timeout],
            2,
            5,
            BackoffRate::new(1.5).unwrap(),
            Some(60),
            JitterStrategy::Full,
        )
        .unwrap();

        let json = serde_json::to_string(&r).unwrap();
        // Verify camelCase keys
        assert!(json.contains("errorEquals"));
        assert!(json.contains("intervalSeconds"));
        assert!(json.contains("maxAttempts"));
        assert!(json.contains("backoffRate"));
        assert!(json.contains("maxDelaySeconds"));
        assert!(json.contains("jitterStrategy"));

        let back: Retrier = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    // -- RT-12: Serde omits None fields ---------------------------------

    #[test]
    fn rt12_serde_omits_none() {
        let r = Retrier::new(
            vec![ErrorCode::All],
            1,
            1,
            BackoffRate::new(1.0).unwrap(),
            None,
            JitterStrategy::None,
        )
        .unwrap();

        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("maxDelaySeconds"),
            "None field should be omitted: {json}"
        );
    }

    // -- RT-13: Serde defaults jitter_strategy to NONE ------------------

    #[test]
    fn rt13_serde_defaults_jitter() {
        let json = r#"{"errorEquals": ["all"], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0}"#;
        let r: Retrier = serde_json::from_str(json).unwrap();
        assert_eq!(r.jitter_strategy(), JitterStrategy::None);
    }

    // -- RT-14: Serde rejects invalid retrier ---------------------------

    #[test]
    fn rt14_serde_rejects_empty_errors() {
        let json =
            r#"{"errorEquals": [], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0}"#;
        let err = serde_json::from_str::<Retrier>(json).unwrap_err();
        assert!(
            err.to_string().contains("empty"),
            "expected 'empty' in: {err}"
        );
    }

    // -- RT-15: Serde rejects zero interval -----------------------------

    #[test]
    fn rt15_serde_rejects_zero_interval() {
        let json = r#"{"errorEquals": ["all"], "intervalSeconds": 0, "maxAttempts": 3, "backoffRate": 2.0}"#;
        let err = serde_json::from_str::<Retrier>(json).unwrap_err();
        assert!(
            err.to_string().contains("interval"),
            "expected 'interval' in: {err}"
        );
    }

    // -- RT-16: YAML deserialization ------------------------------------

    #[test]
    fn rt16_yaml_deserialization() {
        let yaml = "\
errorEquals:
  - timeout
  - taskfailed
intervalSeconds: 3
maxAttempts: 5
backoffRate: 2.0
jitterStrategy: FULL
";
        let r: Retrier = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(
            r.error_equals(),
            &[ErrorCode::Timeout, ErrorCode::TaskFailed]
        );
        assert_eq!(r.jitter_strategy(), JitterStrategy::Full);
    }

    // -- RT-17: Single error_equals entry (boundary) --------------------

    #[test]
    fn rt17_single_error_equals() {
        let r = Retrier::new(
            vec![ErrorCode::All],
            1,
            1,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .unwrap();
        assert_eq!(r.error_equals().len(), 1);
    }

    // -- RT-18: Large max_attempts value --------------------------------

    #[test]
    fn rt18_large_max_attempts() {
        let r = Retrier::new(
            default_errors(),
            1,
            u32::MAX,
            default_backoff(),
            None,
            JitterStrategy::None,
        )
        .unwrap();
        assert_eq!(r.max_attempts(), u32::MAX);
    }
}

// ===========================================================================
// Catcher
// ===========================================================================

mod catcher_tests {
    use super::*;

    fn default_next() -> StateName {
        StateName::new("HandleError").unwrap()
    }

    // -- CA-1: Valid construction with all fields -----------------------

    #[test]
    fn ca1_valid_all_fields() {
        let mut assign_map = HashMap::new();
        assign_map.insert(
            VariableName::new("retries").unwrap(),
            Expression::new("$.retryCount").unwrap(),
        );

        let c = Catcher::new(
            vec![ErrorCode::Timeout, ErrorCode::TaskFailed],
            StateName::new("HandleError").unwrap(),
            Some(JsonPath::new("$.error").unwrap()),
            Some(assign_map),
        )
        .expect("valid catcher");

        assert_eq!(
            c.error_equals(),
            &[ErrorCode::Timeout, ErrorCode::TaskFailed]
        );
        assert_eq!(c.next().as_str(), "HandleError");
        assert_eq!(c.result_path().map(|p| p.as_str()), Some("$.error"));
        assert!(c.assign().is_some());
        assert_eq!(c.assign().unwrap().len(), 1);
    }

    // -- CA-2: Valid construction with minimal fields -------------------

    #[test]
    fn ca2_valid_minimal() {
        let c = Catcher::new(
            vec![ErrorCode::All],
            StateName::new("Fallback").unwrap(),
            None,
            None,
        )
        .expect("valid catcher");

        assert_eq!(c.result_path(), None);
        assert_eq!(c.assign(), None);
    }

    // -- CA-3: Reject empty error_equals --------------------------------

    #[test]
    fn ca3_reject_empty_errors() {
        let err = Catcher::new(vec![], default_next(), None, None).unwrap_err();
        assert_eq!(err, CatcherError::EmptyErrorEquals);
    }

    // -- CA-4: Serde roundtrip (JSON) -----------------------------------

    #[test]
    fn ca4_serde_roundtrip() {
        let c = Catcher::new(
            vec![ErrorCode::Timeout],
            StateName::new("RecoveryState").unwrap(),
            Some(JsonPath::new("$.error").unwrap()),
            None,
        )
        .unwrap();

        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("errorEquals"));
        assert!(json.contains("RecoveryState"));
        assert!(json.contains("resultPath"));

        let back: Catcher = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    // -- CA-5: Serde omits None fields ----------------------------------

    #[test]
    fn ca5_serde_omits_none() {
        let c = Catcher::new(
            vec![ErrorCode::All],
            StateName::new("X").unwrap(),
            None,
            None,
        )
        .unwrap();

        let json = serde_json::to_string(&c).unwrap();
        assert!(
            !json.contains("resultPath"),
            "None field should be omitted: {json}"
        );
        assert!(
            !json.contains("assign"),
            "None field should be omitted: {json}"
        );
    }

    // -- CA-6: Serde rejects empty errorEquals on deserialize -----------

    #[test]
    fn ca6_serde_rejects_empty_errors() {
        let json = r#"{"errorEquals": [], "next": "Foo"}"#;
        let err = serde_json::from_str::<Catcher>(json).unwrap_err();
        assert!(
            err.to_string().contains("empty"),
            "expected 'empty' in: {err}"
        );
    }

    // -- CA-7: Serde with assign map ------------------------------------

    #[test]
    fn ca7_serde_with_assign() {
        let json =
            r#"{"errorEquals": ["all"], "next": "HandleAll", "assign": {"error_msg": "$.Cause"}}"#;
        let c: Catcher = serde_json::from_str(json).unwrap();
        let assign = c.assign().expect("assign present");
        let key = VariableName::new("error_msg").unwrap();
        let val = assign.get(&key).expect("key present");
        assert_eq!(val.as_str(), "$.Cause");
    }

    // -- CA-8: YAML deserialization -------------------------------------

    #[test]
    fn ca8_yaml_deserialization() {
        let yaml = "\
errorEquals:
  - timeout
next: RecoveryState
resultPath: \"$.error\"
";
        let c: Catcher = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(c.next().as_str(), "RecoveryState");
        assert_eq!(c.result_path().map(|p| p.as_str()), Some("$.error"));
    }

    // -- CA-9: Single error_equals entry (boundary) ---------------------

    #[test]
    fn ca9_single_error() {
        let c = Catcher::new(
            vec![ErrorCode::Custom("MyError".into())],
            default_next(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(c.error_equals().len(), 1);
    }

    // -- CA-10: Multiple catchers in a list -----------------------------

    #[test]
    fn ca10_multiple_catchers() {
        let json = r#"[
            {"errorEquals": ["timeout"], "next": "TimeoutHandler"},
            {"errorEquals": ["all"], "next": "DefaultHandler"}
        ]"#;
        let catchers: Vec<Catcher> = serde_json::from_str(json).unwrap();
        assert_eq!(catchers.len(), 2);
        assert_eq!(catchers[0].next().as_str(), "TimeoutHandler");
        assert_eq!(catchers[1].next().as_str(), "DefaultHandler");
    }

    // -- CA-11: Multiple retriers in a list -----------------------------

    #[test]
    fn ca11_multiple_retriers() {
        let json = r#"[
            {"errorEquals": ["timeout"], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0},
            {"errorEquals": ["all"], "intervalSeconds": 5, "maxAttempts": 2, "backoffRate": 1.5, "jitterStrategy": "FULL"}
        ]"#;
        let retriers: Vec<Retrier> = serde_json::from_str(json).unwrap();
        assert_eq!(retriers.len(), 2);
        assert_eq!(retriers[0].error_equals(), &[ErrorCode::Timeout]);
        assert_eq!(retriers[1].jitter_strategy(), JitterStrategy::Full);
    }
}
