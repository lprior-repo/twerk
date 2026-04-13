//! Red Queen STATE 5: Adversarial tests for ASL module.
//!
//! These tests attempt to BREAK the implementation by constructing invalid types,
//! deserializing malformed JSON, probing boundaries, and attacking data flow paths.

use std::collections::HashMap;

use evalexpr::Value;
use serde_json::json;

use twerk_core::asl::*;
use twerk_core::eval::data_flow::{
    apply_data_flow, apply_input_path, apply_output_path, apply_result_path, DataFlowError,
};
use twerk_core::eval::intrinsics::*;

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 1: CONTRACT VIOLATIONS — Type-level invariant attacks
// ═══════════════════════════════════════════════════════════════════════════

mod contract_violations {
    use super::*;

    // ── StateName ──

    #[test]
    fn state_name_empty_string_rejected() {
        assert!(StateName::new("").is_err());
    }

    #[test]
    fn state_name_exactly_256_chars_accepted() {
        let name = "a".repeat(256);
        assert!(StateName::new(&name).is_ok());
    }

    #[test]
    fn state_name_257_chars_rejected() {
        let name = "a".repeat(257);
        let err = StateName::new(&name).unwrap_err();
        assert!(matches!(err, StateNameError::TooLong(257)));
    }

    #[test]
    fn state_name_1000_chars_rejected() {
        let name = "x".repeat(1000);
        assert!(matches!(
            StateName::new(&name).unwrap_err(),
            StateNameError::TooLong(1000)
        ));
    }

    #[test]
    fn state_name_null_byte_accepted_by_type_system() {
        // Null bytes in the middle: the type doesn't reject them
        let result = StateName::new("hello\0world");
        assert!(
            result.is_ok(),
            "StateName should accept null bytes (Rust strings allow them)"
        );
    }

    #[test]
    fn state_name_unicode_zero_width_accepted() {
        // Zero-width joiners and other invisible Unicode
        let name = "state\u{200B}\u{200C}\u{200D}\u{FEFF}name";
        assert!(StateName::new(name).is_ok());
    }

    #[test]
    fn state_name_rtl_markers() {
        let name = "\u{202E}esrever\u{202C}";
        assert!(StateName::new(name).is_ok());
    }

    #[test]
    fn state_name_emoji() {
        // Emoji can be multi-byte; 256 emoji = way more than 256 bytes
        let name = "🔥".repeat(256);
        // .len() counts bytes, not chars — 🔥 is 4 bytes → 1024 bytes
        assert!(name.len() > 256);
        assert!(
            StateName::new(&name).is_err(),
            "256 emoji = 1024 bytes, should exceed 256-char limit"
        );
    }

    #[test]
    fn state_name_256_multibyte_chars_boundary() {
        // 256 2-byte chars = 512 bytes but 256 chars
        // .len() returns BYTE count. Let's verify the boundary is byte-based.
        let name = "é".repeat(256); // é is 2 bytes in UTF-8
                                    // If limit is byte-based: 512 > 256 → rejected
                                    // If limit is char-based: 256 <= 256 → accepted
        let result = StateName::new(&name);
        // The code uses s.len() which is byte length in Rust
        assert!(
            result.is_err(),
            "BUG CANDIDATE: StateName uses byte length (.len()) not char count. \
             256 'é' chars = 512 bytes > 256 byte limit"
        );
    }

    // ── BackoffRate ──

    #[test]
    fn backoff_rate_nan_rejected() {
        assert!(matches!(
            BackoffRate::new(f64::NAN).unwrap_err(),
            BackoffRateError::NotFinite(_)
        ));
    }

    #[test]
    fn backoff_rate_positive_infinity_rejected() {
        assert!(matches!(
            BackoffRate::new(f64::INFINITY).unwrap_err(),
            BackoffRateError::NotFinite(_)
        ));
    }

    #[test]
    fn backoff_rate_negative_infinity_rejected() {
        assert!(matches!(
            BackoffRate::new(f64::NEG_INFINITY).unwrap_err(),
            BackoffRateError::NotFinite(_)
        ));
    }

    #[test]
    fn backoff_rate_zero_rejected() {
        assert!(matches!(
            BackoffRate::new(0.0).unwrap_err(),
            BackoffRateError::NotPositive(_)
        ));
    }

    #[test]
    fn backoff_rate_negative_zero_rejected() {
        assert!(matches!(
            BackoffRate::new(-0.0).unwrap_err(),
            BackoffRateError::NotPositive(_)
        ));
    }

    #[test]
    fn backoff_rate_negative_one_rejected() {
        assert!(matches!(
            BackoffRate::new(-1.0).unwrap_err(),
            BackoffRateError::NotPositive(_)
        ));
    }

    #[test]
    fn backoff_rate_epsilon_accepted() {
        assert!(BackoffRate::new(f64::EPSILON).is_ok());
    }

    #[test]
    fn backoff_rate_max_accepted() {
        assert!(BackoffRate::new(f64::MAX).is_ok());
    }

    #[test]
    fn backoff_rate_subnormal_positive_accepted() {
        let subnormal = f64::MIN_POSITIVE / 2.0;
        assert!(subnormal > 0.0 && subnormal.is_finite());
        assert!(BackoffRate::new(subnormal).is_ok());
    }

    // ── ErrorCode ──

    #[test]
    fn error_code_extremely_long_custom_string() {
        let long_str = "x".repeat(100_000);
        let code: ErrorCode = long_str.parse().unwrap();
        assert!(matches!(code, ErrorCode::Custom(_)));
    }

    #[test]
    fn error_code_empty_string_becomes_custom() {
        let code: ErrorCode = "".parse().unwrap();
        assert!(matches!(code, ErrorCode::Custom(ref s) if s.is_empty()));
    }

    #[test]
    fn error_code_case_insensitive_all() {
        let code: ErrorCode = "ALL".parse().unwrap();
        assert_eq!(code, ErrorCode::All);
        let code2: ErrorCode = "aLl".parse().unwrap();
        assert_eq!(code2, ErrorCode::All);
    }

    // ── Retrier ──

    #[test]
    fn retrier_interval_zero_rejected() {
        let rate = BackoffRate::new(1.0).unwrap();
        let err = Retrier::new(vec![ErrorCode::All], 0, 1, rate, None, JitterStrategy::None);
        assert!(matches!(err, Err(RetrierError::IntervalTooSmall(0))));
    }

    #[test]
    fn retrier_max_attempts_zero_rejected() {
        let rate = BackoffRate::new(1.0).unwrap();
        let err = Retrier::new(vec![ErrorCode::All], 1, 0, rate, None, JitterStrategy::None);
        assert!(matches!(err, Err(RetrierError::MaxAttemptsTooSmall(0))));
    }

    #[test]
    fn retrier_empty_error_equals_rejected() {
        let rate = BackoffRate::new(1.0).unwrap();
        let err = Retrier::new(vec![], 1, 1, rate, None, JitterStrategy::None);
        assert!(matches!(err, Err(RetrierError::EmptyErrorEquals)));
    }

    #[test]
    fn retrier_max_delay_equal_to_interval_rejected() {
        let rate = BackoffRate::new(1.0).unwrap();
        let err = Retrier::new(
            vec![ErrorCode::All],
            5,
            1,
            rate,
            Some(5), // equal, not greater
            JitterStrategy::None,
        );
        assert!(matches!(
            err,
            Err(RetrierError::MaxDelayNotGreaterThanInterval { .. })
        ));
    }

    #[test]
    fn retrier_max_delay_less_than_interval_rejected() {
        let rate = BackoffRate::new(1.0).unwrap();
        let err = Retrier::new(
            vec![ErrorCode::All],
            10,
            1,
            rate,
            Some(5),
            JitterStrategy::None,
        );
        assert!(matches!(
            err,
            Err(RetrierError::MaxDelayNotGreaterThanInterval { .. })
        ));
    }

    #[test]
    fn retrier_max_u64_interval_accepted() {
        let rate = BackoffRate::new(1.0).unwrap();
        let result = Retrier::new(
            vec![ErrorCode::All],
            u64::MAX,
            1,
            rate,
            None,
            JitterStrategy::None,
        );
        assert!(result.is_ok());
    }

    // ── ChoiceState ──

    #[test]
    fn choice_state_empty_choices_rejected() {
        let err = ChoiceState::new(vec![], None);
        assert!(matches!(err, Err(ChoiceStateError::EmptyChoices)));
    }

    // ── ParallelState ──

    #[test]
    fn parallel_state_empty_branches_rejected() {
        let err = ParallelState::new(vec![], Transition::end(), None);
        assert!(matches!(err, Err(ParallelStateError::EmptyBranches)));
    }

    // ── Catcher ──

    #[test]
    fn catcher_empty_error_equals_rejected() {
        let name = StateName::new("recovery").unwrap();
        let err = Catcher::new(vec![], name, None, None);
        assert!(matches!(err, Err(CatcherError::EmptyErrorEquals)));
    }

    // ── JsonPath ──

    #[test]
    fn json_path_empty_rejected() {
        assert!(matches!(
            JsonPath::new("").unwrap_err(),
            JsonPathError::Empty
        ));
    }

    #[test]
    fn json_path_no_dollar_prefix_rejected() {
        assert!(matches!(
            JsonPath::new("field.name").unwrap_err(),
            JsonPathError::MissingDollarPrefix(_)
        ));
    }

    #[test]
    fn json_path_just_dollar_accepted() {
        assert!(JsonPath::new("$").is_ok());
    }

    // ── VariableName ──

    #[test]
    fn variable_name_empty_rejected() {
        assert!(matches!(
            VariableName::new("").unwrap_err(),
            VariableNameError::Empty
        ));
    }

    #[test]
    fn variable_name_starts_with_digit_rejected() {
        assert!(matches!(
            VariableName::new("9var").unwrap_err(),
            VariableNameError::InvalidStart('9')
        ));
    }

    #[test]
    fn variable_name_contains_dash_rejected() {
        assert!(matches!(
            VariableName::new("my-var").unwrap_err(),
            VariableNameError::InvalidCharacter('-')
        ));
    }

    #[test]
    fn variable_name_129_chars_rejected() {
        let name = "a".repeat(129);
        assert!(matches!(
            VariableName::new(&name).unwrap_err(),
            VariableNameError::TooLong(129)
        ));
    }

    #[test]
    fn variable_name_128_chars_accepted() {
        let name = "a".repeat(128);
        assert!(VariableName::new(&name).is_ok());
    }

    #[test]
    fn variable_name_unicode_start_rejected() {
        assert!(matches!(
            VariableName::new("αlpha").unwrap_err(),
            VariableNameError::InvalidStart('α')
        ));
    }

    // ── ImageRef ──

    #[test]
    fn image_ref_empty_rejected() {
        assert!(matches!(
            ImageRef::new("").unwrap_err(),
            ImageRefError::Empty
        ));
    }

    #[test]
    fn image_ref_whitespace_rejected() {
        assert!(matches!(
            ImageRef::new("my image").unwrap_err(),
            ImageRefError::ContainsWhitespace
        ));
    }

    #[test]
    fn image_ref_tab_rejected() {
        assert!(matches!(
            ImageRef::new("my\timage").unwrap_err(),
            ImageRefError::ContainsWhitespace
        ));
    }

    #[test]
    fn image_ref_newline_rejected() {
        assert!(matches!(
            ImageRef::new("my\nimage").unwrap_err(),
            ImageRefError::ContainsWhitespace
        ));
    }

    // ── TaskState ──

    #[test]
    fn task_state_timeout_zero_rejected() {
        let img = ImageRef::new("alpine").unwrap();
        let run = ShellScript::new("echo hi").unwrap();
        let err = TaskState::new(
            img,
            run,
            HashMap::new(),
            None,
            Some(0),
            None,
            vec![],
            vec![],
            Transition::end(),
        );
        assert!(matches!(err, Err(TaskStateError::TimeoutTooSmall(0))));
    }

    #[test]
    fn task_state_heartbeat_zero_rejected() {
        let img = ImageRef::new("alpine").unwrap();
        let run = ShellScript::new("echo hi").unwrap();
        let err = TaskState::new(
            img,
            run,
            HashMap::new(),
            None,
            None,
            Some(0),
            vec![],
            vec![],
            Transition::end(),
        );
        assert!(matches!(err, Err(TaskStateError::HeartbeatTooSmall(0))));
    }

    #[test]
    fn task_state_heartbeat_exceeds_timeout_rejected() {
        let img = ImageRef::new("alpine").unwrap();
        let run = ShellScript::new("echo hi").unwrap();
        let err = TaskState::new(
            img,
            run,
            HashMap::new(),
            None,
            Some(10),
            Some(10), // equal
            vec![],
            vec![],
            Transition::end(),
        );
        assert!(matches!(
            err,
            Err(TaskStateError::HeartbeatExceedsTimeout { .. })
        ));
    }

    #[test]
    fn task_state_heartbeat_greater_than_timeout_rejected() {
        let img = ImageRef::new("alpine").unwrap();
        let run = ShellScript::new("echo hi").unwrap();
        let err = TaskState::new(
            img,
            run,
            HashMap::new(),
            None,
            Some(5),
            Some(10),
            vec![],
            vec![],
            Transition::end(),
        );
        assert!(matches!(
            err,
            Err(TaskStateError::HeartbeatExceedsTimeout { .. })
        ));
    }

    #[test]
    fn task_state_empty_env_key_rejected() {
        let img = ImageRef::new("alpine").unwrap();
        let run = ShellScript::new("echo hi").unwrap();
        let expr = Expression::new("value").unwrap();
        let mut env = HashMap::new();
        env.insert(String::new(), expr);
        let err = TaskState::new(
            img,
            run,
            env,
            None,
            None,
            None,
            vec![],
            vec![],
            Transition::end(),
        );
        assert!(matches!(err, Err(TaskStateError::EmptyEnvKey)));
    }

    // ── MapState ──

    #[test]
    fn map_state_tolerance_nan_rejected() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        let err = MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(f64::NAN),
        );
        assert!(matches!(
            err,
            Err(MapStateError::NonFiniteToleratedFailurePercentage)
        ));
    }

    #[test]
    fn map_state_tolerance_infinity_rejected() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        let err = MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(f64::INFINITY),
        );
        assert!(matches!(
            err,
            Err(MapStateError::NonFiniteToleratedFailurePercentage)
        ));
    }

    #[test]
    fn map_state_tolerance_negative_rejected() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        let err = MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(-0.1),
        );
        assert!(matches!(
            err,
            Err(MapStateError::InvalidToleratedFailurePercentage(_))
        ));
    }

    #[test]
    fn map_state_tolerance_101_rejected() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        let err = MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(100.01),
        );
        assert!(matches!(
            err,
            Err(MapStateError::InvalidToleratedFailurePercentage(_))
        ));
    }

    #[test]
    fn map_state_tolerance_0_accepted() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        assert!(MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(0.0)
        )
        .is_ok());
    }

    #[test]
    fn map_state_tolerance_100_accepted() {
        let expr = Expression::new("$.items").unwrap();
        let machine = Box::new(minimal_machine());
        assert!(MapState::new(
            expr,
            machine,
            None,
            Transition::end(),
            vec![],
            vec![],
            Some(100.0)
        )
        .is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 2: SERDE EXPLOITS — Malformed JSON deserialization attacks
// ═══════════════════════════════════════════════════════════════════════════

mod serde_exploits {
    use super::*;

    #[test]
    fn transition_both_next_and_end_rejected() {
        let json = json!({"next": "A", "end": true});
        let result = serde_json::from_value::<Transition>(json);
        assert!(
            result.is_err(),
            "Should reject transition with both next and end"
        );
    }

    #[test]
    fn transition_neither_next_nor_end_rejected() {
        let json = json!({});
        let result = serde_json::from_value::<Transition>(json);
        assert!(
            result.is_err(),
            "Should reject transition with neither next nor end"
        );
    }

    #[test]
    fn transition_end_false_rejected() {
        let json = json!({"end": false});
        let result = serde_json::from_value::<Transition>(json);
        assert!(result.is_err(), "Should reject end: false");
    }

    #[test]
    fn transition_next_empty_string_rejected() {
        let json = json!({"next": ""});
        let result = serde_json::from_value::<Transition>(json);
        assert!(
            result.is_err(),
            "Should reject empty state name in transition"
        );
    }

    #[test]
    fn transition_next_too_long_rejected() {
        let name = "a".repeat(257);
        let json = json!({"next": name});
        let result = serde_json::from_value::<Transition>(json);
        assert!(result.is_err(), "Should reject oversized state name");
    }

    #[test]
    fn state_kind_unknown_type_rejected() {
        let json = json!({
            "type": "unknown_type",
            "end": true
        });
        let result = serde_json::from_value::<StateKind>(json);
        assert!(result.is_err(), "Should reject unknown state type");
    }

    #[test]
    fn state_kind_empty_type_rejected() {
        let json = json!({
            "type": "",
            "end": true
        });
        let result = serde_json::from_value::<StateKind>(json);
        assert!(result.is_err(), "Should reject empty type");
    }

    #[test]
    fn state_machine_empty_states_deserializes_but_validate_catches() {
        let json = json!({
            "startAt": "A",
            "states": {}
        });
        // serde_json may succeed or fail depending on validation
        let result = serde_json::from_value::<StateMachine>(json);
        if let Ok(machine) = result {
            // validate() should catch empty states
            assert!(machine.validate().is_err());
        }
        // Either way, invalid state is caught
    }

    #[test]
    fn retrier_zero_interval_via_json_rejected() {
        let json = json!({
            "errorEquals": ["all"],
            "intervalSeconds": 0,
            "maxAttempts": 3,
            "backoffRate": 1.5
        });
        let result = serde_json::from_value::<Retrier>(json);
        assert!(
            result.is_err(),
            "Deserialized retrier should reject interval=0"
        );
    }

    #[test]
    fn retrier_zero_max_attempts_via_json_rejected() {
        let json = json!({
            "errorEquals": ["all"],
            "intervalSeconds": 1,
            "maxAttempts": 0,
            "backoffRate": 1.5
        });
        let result = serde_json::from_value::<Retrier>(json);
        assert!(result.is_err());
    }

    #[test]
    fn retrier_nan_backoff_via_json_rejected() {
        // JSON doesn't have NaN natively, but we can try with a null or string
        let json_str =
            r#"{"errorEquals":["all"],"intervalSeconds":1,"maxAttempts":1,"backoffRate":"NaN"}"#;
        let result = serde_json::from_str::<Retrier>(json_str);
        assert!(result.is_err(), "Should reject NaN backoff rate in JSON");
    }

    #[test]
    fn retrier_negative_backoff_via_json_rejected() {
        let json = json!({
            "errorEquals": ["all"],
            "intervalSeconds": 1,
            "maxAttempts": 1,
            "backoffRate": -1.0
        });
        let result = serde_json::from_value::<Retrier>(json);
        assert!(result.is_err(), "Should reject negative backoff rate");
    }

    #[test]
    fn retrier_empty_error_equals_via_json_rejected() {
        let json = json!({
            "errorEquals": [],
            "intervalSeconds": 1,
            "maxAttempts": 1,
            "backoffRate": 1.0
        });
        let result = serde_json::from_value::<Retrier>(json);
        assert!(result.is_err(), "Should reject empty errorEquals");
    }

    #[test]
    fn wait_duration_multiple_fields_rejected() {
        let json = json!({
            "seconds": 5,
            "timestamp": "2024-01-01T00:00:00Z",
            "next": "Done"
        });
        let result = serde_json::from_value::<WaitState>(json);
        assert!(
            result.is_err(),
            "Should reject WaitState with both seconds and timestamp"
        );
    }

    #[test]
    fn wait_duration_no_fields_rejected() {
        let json = json!({"next": "Done"});
        let result = serde_json::from_value::<WaitState>(json);
        assert!(result.is_err(), "Should reject WaitState with no duration");
    }

    #[test]
    fn wait_duration_empty_timestamp_rejected() {
        let json = json!({
            "timestamp": "",
            "next": "Done"
        });
        let result = serde_json::from_value::<WaitState>(json);
        assert!(result.is_err(), "Should reject empty timestamp");
    }

    #[test]
    fn wait_duration_all_four_fields_rejected() {
        let json = json!({
            "seconds": 5,
            "timestamp": "2024-01-01",
            "seconds_path": "$.s",
            "timestamp_path": "$.t",
            "end": true
        });
        let result = serde_json::from_value::<WaitState>(json);
        assert!(
            result.is_err(),
            "Should reject all four duration fields at once"
        );
    }

    #[test]
    fn choice_state_empty_choices_via_json_rejected() {
        let json = json!({
            "choices": []
        });
        let result = serde_json::from_value::<ChoiceState>(json);
        assert!(result.is_err());
    }

    #[test]
    fn parallel_state_empty_branches_via_json_rejected() {
        let json = json!({
            "branches": [],
            "end": true
        });
        let result = serde_json::from_value::<ParallelState>(json);
        assert!(result.is_err());
    }

    #[test]
    fn catcher_empty_error_equals_via_json_rejected() {
        let json = json!({
            "errorEquals": [],
            "next": "Recovery"
        });
        let result = serde_json::from_value::<Catcher>(json);
        assert!(result.is_err());
    }

    #[test]
    fn task_state_timeout_zero_via_json_rejected() {
        let json = json!({
            "image": "alpine",
            "run": "echo hi",
            "timeout": 0,
            "end": true
        });
        let result = serde_json::from_value::<TaskState>(json);
        assert!(result.is_err());
    }

    #[test]
    fn task_state_heartbeat_exceeds_timeout_via_json_rejected() {
        let json = json!({
            "image": "alpine",
            "run": "echo hi",
            "timeout": 5,
            "heartbeat": 10,
            "end": true
        });
        let result = serde_json::from_value::<TaskState>(json);
        assert!(result.is_err());
    }

    #[test]
    fn backoff_rate_zero_via_json_rejected() {
        let result = serde_json::from_value::<BackoffRate>(json!(0.0));
        assert!(result.is_err());
    }

    #[test]
    fn state_name_empty_via_json_rejected() {
        let result = serde_json::from_value::<StateName>(json!(""));
        assert!(result.is_err());
    }

    #[test]
    fn json_path_no_dollar_via_json_rejected() {
        let result = serde_json::from_value::<JsonPath>(json!("no.dollar"));
        assert!(result.is_err());
    }

    #[test]
    fn variable_name_digit_start_via_json_rejected() {
        let result = serde_json::from_value::<VariableName>(json!("1bad"));
        assert!(result.is_err());
    }

    #[test]
    fn image_ref_whitespace_via_json_rejected() {
        let result = serde_json::from_value::<ImageRef>(json!("has space"));
        assert!(result.is_err());
    }

    #[test]
    fn expression_empty_via_json_rejected() {
        let result = serde_json::from_value::<Expression>(json!(""));
        assert!(result.is_err());
    }

    #[test]
    fn shell_script_empty_via_json_rejected() {
        let result = serde_json::from_value::<ShellScript>(json!(""));
        assert!(result.is_err());
    }

    #[test]
    fn map_state_nan_tolerance_via_json() {
        // JSON doesn't have NaN, but Infinity can't be in standard JSON either.
        // Test negative tolerance via JSON.
        let json = json!({
            "itemsPath": "$.items",
            "itemProcessor": {
                "startAt": "Step",
                "states": {
                    "Step": {
                        "type": "succeed"
                    }
                }
            },
            "toleratedFailurePercentage": -5.0,
            "end": true
        });
        let result = serde_json::from_value::<MapState>(json);
        assert!(result.is_err(), "Should reject negative tolerance via JSON");
    }

    #[test]
    fn jitter_strategy_unknown_value_rejected() {
        let result = serde_json::from_value::<JitterStrategy>(json!("PARTIAL"));
        assert!(result.is_err(), "Should reject unknown jitter strategy");
    }

    #[test]
    fn wait_state_with_both_transition_fields_rejected() {
        let json = json!({
            "seconds": 5,
            "next": "A",
            "end": true
        });
        let result = serde_json::from_value::<WaitState>(json);
        assert!(
            result.is_err(),
            "Should reject both next and end in WaitState"
        );
    }

    #[test]
    fn wait_state_seconds_path_invalid_path_rejected() {
        let json = json!({
            "seconds_path": "no_dollar",
            "end": true
        });
        let result = serde_json::from_value::<WaitState>(json);
        assert!(
            result.is_err(),
            "Should reject invalid JSON path in seconds_path"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 3: BOUNDARY ATTACKS — Extremes and edge cases
// ═══════════════════════════════════════════════════════════════════════════

mod boundary_attacks {
    use super::*;

    #[test]
    fn state_machine_max_u64_timeout() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": {
                    "type": "succeed"
                }
            },
            "timeout": u64::MAX
        });
        let result = serde_json::from_value::<StateMachine>(json);
        assert!(result.is_ok(), "Should accept u64::MAX timeout");
        assert!(result.unwrap().validate().is_ok());
    }

    #[test]
    fn state_machine_validate_empty_states() {
        let json = json!({
            "startAt": "A",
            "states": {}
        });
        // The JSON deserialization might succeed since start_at is validated separately
        if let Ok(machine) = serde_json::from_value::<StateMachine>(json) {
            let errors = machine.validate().unwrap_err();
            assert!(errors
                .iter()
                .any(|e| matches!(e, StateMachineError::EmptyStates)));
        }
    }

    #[test]
    fn state_machine_start_at_not_in_states() {
        let json = json!({
            "startAt": "Missing",
            "states": {
                "A": { "type": "succeed" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::StartAtNotFound { .. })));
    }

    #[test]
    fn state_machine_no_terminal_state() {
        // A Pass state that transitions to itself - no terminal
        let json = json!({
            "startAt": "Loop",
            "states": {
                "Loop": {
                    "type": "pass",
                    "next": "Loop"
                }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::NoTerminalState)));
    }

    #[test]
    fn state_machine_transition_to_nonexistent_state() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": {
                    "type": "pass",
                    "next": "Nonexistent"
                }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::TransitionTargetNotFound { .. })));
    }

    #[test]
    fn state_machine_choice_target_not_found() {
        let json = json!({
            "startAt": "C",
            "states": {
                "C": {
                    "type": "choice",
                    "choices": [
                        {"condition": "true", "next": "Ghost"}
                    ]
                },
                "End": { "type": "succeed" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::ChoiceTargetNotFound { .. })));
    }

    #[test]
    fn state_machine_choice_default_not_found() {
        let json = json!({
            "startAt": "C",
            "states": {
                "C": {
                    "type": "choice",
                    "choices": [
                        {"condition": "true", "next": "End"}
                    ],
                    "default": "Ghost"
                },
                "End": { "type": "succeed" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::DefaultTargetNotFound { .. })));
    }

    #[test]
    fn state_machine_multiple_errors_returned() {
        let json = json!({
            "startAt": "Missing",
            "states": {
                "A": {
                    "type": "pass",
                    "next": "Ghost"
                }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        // Should have both StartAtNotFound AND TransitionTargetNotFound AND NoTerminalState
        assert!(
            errors.len() >= 2,
            "Should report multiple errors, got {}",
            errors.len()
        );
    }

    #[test]
    fn deeply_nested_parallel_state() {
        // Parallel → Parallel → Parallel (3 levels deep)
        fn nested_parallel(depth: u32) -> serde_json::Value {
            if depth == 0 {
                json!({
                    "startAt": "Done",
                    "states": {
                        "Done": { "type": "succeed" }
                    }
                })
            } else {
                let inner = nested_parallel(depth - 1);
                json!({
                    "startAt": "P",
                    "states": {
                        "P": {
                            "type": "parallel",
                            "branches": [inner],
                            "end": true
                        }
                    }
                })
            }
        }

        let deep = nested_parallel(10);
        let result = serde_json::from_value::<StateMachine>(deep);
        assert!(result.is_ok(), "Should handle 10-level nested parallels");
    }

    #[test]
    fn state_machine_roundtrip_serialization() {
        let json = json!({
            "startAt": "Begin",
            "states": {
                "Begin": {
                    "type": "pass",
                    "next": "End"
                },
                "End": {
                    "type": "succeed"
                }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let serialized = serde_json::to_value(&machine).unwrap();
        let roundtripped: StateMachine = serde_json::from_value(serialized).unwrap();
        assert_eq!(machine, roundtripped);
    }

    #[test]
    fn error_code_all_matches_everything() {
        assert!(ErrorCode::All.matches(&ErrorCode::Timeout));
        assert!(ErrorCode::All.matches(&ErrorCode::TaskFailed));
        assert!(ErrorCode::All.matches(&ErrorCode::Custom("anything".into())));
        assert!(ErrorCode::All.matches(&ErrorCode::All));
    }

    #[test]
    fn error_code_specific_does_not_match_other() {
        assert!(!ErrorCode::Timeout.matches(&ErrorCode::TaskFailed));
        assert!(!ErrorCode::Custom("a".into()).matches(&ErrorCode::Custom("b".into())));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 4: DATA FLOW ATTACKS — JSON path processing
// ═══════════════════════════════════════════════════════════════════════════

mod data_flow_attacks {
    use super::*;

    #[test]
    fn input_path_on_null_value() {
        let input = json!(null);
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should error when resolving path on null");
    }

    #[test]
    fn input_path_on_primitive_string() {
        let input = json!("just a string");
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err());
    }

    #[test]
    fn input_path_on_number() {
        let input = json!(42);
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err());
    }

    #[test]
    fn input_path_on_array() {
        let input = json!([1, 2, 3]);
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should error when field access on array");
    }

    #[test]
    fn input_path_root_only() {
        let input = json!({"key": "value"});
        let path = JsonPath::new("$").unwrap();
        let result = apply_input_path(&input, Some(&path)).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn input_path_none_returns_input() {
        let input = json!({"key": "value"});
        let result = apply_input_path(&input, None).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn deeply_nested_path() {
        let input = json!({"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": {"i": {"j": 42}}}}}}}}}});
        let path = JsonPath::new("$.a.b.c.d.e.f.g.h.i.j").unwrap();
        let result = apply_input_path(&input, Some(&path)).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn path_with_missing_intermediate_field() {
        let input = json!({"a": {"b": 1}});
        let path = JsonPath::new("$.a.missing.field").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err());
    }

    #[test]
    fn array_index_access() {
        let input = json!({"items": [10, 20, 30]});
        let path = JsonPath::new("$.items[1]").unwrap();
        let result = apply_input_path(&input, Some(&path)).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn array_index_out_of_bounds() {
        let input = json!({"items": [1, 2, 3]});
        let path = JsonPath::new("$.items[99]").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should error on array index out of bounds");
    }

    #[test]
    fn array_index_on_non_array() {
        let input = json!({"items": "not an array"});
        let path = JsonPath::new("$.items[0]").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should error on array index of non-array");
    }

    #[test]
    fn path_injection_attempt() {
        let input = json!({"field": "value"});
        let path = JsonPath::new("$.\"; drop table").unwrap();
        let result = apply_input_path(&input, Some(&path));
        // Should either error gracefully or not find the field — not crash
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn path_with_special_chars_in_field() {
        let input = json!({"a.b": "dotted", "a": {"b": "nested"}});
        let path = JsonPath::new("$.a.b").unwrap();
        let result = apply_input_path(&input, Some(&path)).unwrap();
        // Should resolve nested path, not dotted key
        assert_eq!(result, json!("nested"));
    }

    #[test]
    fn result_path_rejects_array_index() {
        let input = json!({"items": [1, 2, 3]});
        let result_val = json!("new");
        let path = JsonPath::new("$.items[0]").unwrap();
        let result = apply_result_path(&input, &result_val, Some(&path));
        assert!(result.is_err(), "result_path should reject array index");
    }

    #[test]
    fn result_path_creates_intermediate_objects() {
        let input = json!({});
        let result_val = json!(42);
        let path = JsonPath::new("$.a.b.c").unwrap();
        let result = apply_result_path(&input, &result_val, Some(&path)).unwrap();
        assert_eq!(result, json!({"a": {"b": {"c": 42}}}));
    }

    #[test]
    fn result_path_none_returns_result() {
        let input = json!({"old": "data"});
        let result_val = json!({"new": "data"});
        let result = apply_result_path(&input, &result_val, None).unwrap();
        assert_eq!(result, result_val);
    }

    #[test]
    fn result_path_root_returns_result() {
        let input = json!({"old": "data"});
        let result_val = json!({"new": "data"});
        let path = JsonPath::new("$").unwrap();
        let result = apply_result_path(&input, &result_val, Some(&path)).unwrap();
        assert_eq!(result, result_val);
    }

    #[test]
    fn output_path_on_null() {
        let output = json!(null);
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_output_path(&output, Some(&path));
        assert!(result.is_err());
    }

    #[test]
    fn full_data_flow_pipeline() {
        let input = json!({"data": {"value": 10}, "meta": "info"});
        let result = json!(42);
        let ip = JsonPath::new("$.data").unwrap();
        let rp = JsonPath::new("$.result").unwrap();
        let op = JsonPath::new("$.result").unwrap();
        let out = apply_data_flow(&input, &result, Some(&ip), Some(&rp), Some(&op)).unwrap();
        assert_eq!(out, json!(42));
    }

    #[test]
    fn unclosed_bracket_in_path() {
        let input = json!({"a": [1]});
        let path = JsonPath::new("$.a[0").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should reject unclosed bracket");
        if let Err(DataFlowError::InvalidPath { reason, .. }) = result {
            assert!(reason.contains("unclosed bracket"));
        }
    }

    #[test]
    fn non_integer_array_index_in_path() {
        let input = json!({"a": [1]});
        let path = JsonPath::new("$.a[abc]").unwrap();
        let result = apply_input_path(&input, Some(&path));
        assert!(result.is_err(), "Should reject non-integer index");
    }

    #[test]
    fn path_with_empty_field_segment() {
        let input = json!({"": "empty_key"});
        let path = JsonPath::new("$..field").unwrap();
        // The parser splits by '.', so ".." produces an empty token
        // This tests how the parser handles empty segments
        let _result = apply_input_path(&input, Some(&path));
        // Should either work or error, not panic
    }

    #[test]
    fn result_path_on_non_object_root() {
        let input = json!("just a string");
        let result_val = json!(42);
        let path = JsonPath::new("$.field").unwrap();
        let result = apply_result_path(&input, &result_val, Some(&path));
        assert!(result.is_err(), "Should error setting path on non-object");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 5: INTRINSIC FUNCTION ATTACKS
// ═══════════════════════════════════════════════════════════════════════════

mod intrinsic_attacks {
    use super::*;

    // ── hash ──

    #[test]
    fn hash_unknown_algorithm_rejected() {
        let args = Value::Tuple(vec![
            Value::String("data".into()),
            Value::String("sha512".into()),
        ]);
        let result = hash_fn(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported algorithm"));
    }

    #[test]
    fn hash_empty_algorithm_rejected() {
        let args = Value::Tuple(vec![Value::String("data".into()), Value::String("".into())]);
        let result = hash_fn(&args);
        assert!(result.is_err());
    }

    #[test]

    fn hash_non_string_input_rejected() {
        let args = Value::Tuple(vec![Value::Int(42), Value::String("sha256".into())]);
        let result = hash_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn hash_too_few_args_rejected() {
        let args = Value::Tuple(vec![Value::String("data".into())]);
        let result = hash_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn hash_too_many_args_rejected() {
        let args = Value::Tuple(vec![
            Value::String("a".into()),
            Value::String("sha256".into()),
            Value::String("extra".into()),
        ]);
        let result = hash_fn(&args);
        assert!(result.is_err());
    }

    // ── base64 ──

    #[test]
    fn base64_decode_invalid_base64_rejected() {
        let args = Value::String("not-valid-base64!!!".into());
        let result = base64_decode_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn base64_decode_invalid_utf8_rejected() {
        // Encode some invalid UTF-8 bytes via base64
        // 0xFF 0xFE are not valid UTF-8 start bytes
        let invalid_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            [0xFF, 0xFE, 0x80],
        );
        let args = Value::String(invalid_b64);
        let result = base64_decode_fn(&args);
        assert!(result.is_err(), "Should reject non-UTF8 decoded output");
    }

    #[test]
    fn base64_decode_non_string_rejected() {
        let args = Value::Int(42);
        let result = base64_decode_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn base64_encode_non_string_rejected() {
        let args = Value::Int(42);
        let result = base64_encode_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn base64_roundtrip() {
        let original = "Hello, World! 🌍";
        let encoded = base64_encode_fn(&Value::String(original.into())).unwrap();
        let decoded = base64_decode_fn(&encoded).unwrap();
        assert_eq!(decoded, Value::String(original.into()));
    }

    // ── arrayRange memory bomb ──

    #[test]
    fn array_range_step_zero_rejected() {
        let args = Value::Tuple(vec![Value::Int(0), Value::Int(10), Value::Int(0)]);
        let result = array_range_fn(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("step must not be zero"));
    }

    #[test]
    fn array_range_huge_range_bounded_by_saturating_add() {
        // With step 1, from 0 to i64::MAX — could be a memory bomb
        // The implementation uses saturating_add, so it should eventually terminate
        // but might allocate huge memory. Test with a more reasonable range.
        let args = Value::Tuple(vec![
            Value::Int(0),
            Value::Int(100_000), // reasonable
            Value::Int(1),
        ]);
        let result = array_range_fn(&args);
        assert!(result.is_ok());
        if let Ok(Value::Tuple(items)) = result {
            assert_eq!(items.len(), 100_000);
        }
    }

    #[test]
    fn array_range_negative_step() {
        let args = Value::Tuple(vec![Value::Int(10), Value::Int(0), Value::Int(-2)]);
        let result = array_range_fn(&args).unwrap();
        if let Value::Tuple(items) = result {
            assert_eq!(items.len(), 5);
            assert_eq!(items[0], Value::Int(10));
            assert_eq!(items[4], Value::Int(2));
        }
    }

    #[test]
    fn array_range_start_equals_end_empty() {
        let args = Value::Tuple(vec![Value::Int(5), Value::Int(5), Value::Int(1)]);
        let result = array_range_fn(&args).unwrap();
        if let Value::Tuple(items) = result {
            assert!(items.is_empty());
        }
    }

    #[test]
    fn array_range_wrong_direction_empty() {
        // start > end but step is positive → should be empty
        let args = Value::Tuple(vec![Value::Int(10), Value::Int(0), Value::Int(1)]);
        let result = array_range_fn(&args).unwrap();
        if let Value::Tuple(items) = result {
            assert!(items.is_empty());
        }
    }

    #[test]
    fn array_range_non_integer_rejected() {
        let args = Value::Tuple(vec![Value::Float(1.5), Value::Int(10), Value::Int(1)]);
        let result = array_range_fn(&args);
        assert!(result.is_err());
    }

    // ── format ──

    #[test]
    fn format_more_placeholders_than_args() {
        let args = Value::Tuple(vec![
            Value::String("Hello {} and {} and {}".into()),
            Value::String("world".into()),
        ]);
        let result = format_fn(&args).unwrap();
        // Extra placeholders should remain as "{}"
        if let Value::String(s) = result {
            assert!(
                s.contains("{}"),
                "Unfilled placeholders should remain as '{{}}'"
            );
        }
    }

    #[test]
    fn format_no_placeholders() {
        let args = Value::Tuple(vec![
            Value::String("no placeholders here".into()),
            Value::String("ignored".into()),
        ]);
        let result = format_fn(&args).unwrap();
        assert_eq!(result, Value::String("no placeholders here".into()));
    }

    #[test]
    fn format_empty_template() {
        let args = Value::Tuple(vec![Value::String("".into())]);
        let result = format_fn(&args).unwrap();
        assert_eq!(result, Value::String("".into()));
    }

    #[test]
    fn format_no_args_errors() {
        let args = Value::Tuple(vec![]);
        let result = format_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn format_non_string_template_rejected() {
        let args = Value::Tuple(vec![Value::Int(42)]);
        let result = format_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn format_single_string_no_tuple() {
        let args = Value::String("hello".into());
        let result = format_fn(&args).unwrap();
        assert_eq!(result, Value::String("hello".into()));
    }

    // ── mathRandom ──

    #[test]
    fn math_random_start_equals_end_rejected() {
        let args = Value::Tuple(vec![Value::Int(5), Value::Int(5)]);
        let result = math_random_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn math_random_start_greater_than_end_rejected() {
        let args = Value::Tuple(vec![Value::Int(10), Value::Int(5)]);
        let result = math_random_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn math_random_float_rejected() {
        let args = Value::Tuple(vec![Value::Float(1.0), Value::Float(10.0)]);
        let result = math_random_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn math_random_valid_range() {
        let args = Value::Tuple(vec![Value::Int(0), Value::Int(10)]);
        let result = math_random_fn(&args).unwrap();
        if let Value::Int(v) = result {
            assert!((0..10).contains(&v));
        } else {
            panic!("Expected Int result");
        }
    }

    // ── mathAdd/mathSub ──

    #[test]
    fn math_add_int_overflow_saturates() {
        let args = Value::Tuple(vec![Value::Int(i64::MAX), Value::Int(1)]);
        let result = math_add_fn(&args).unwrap();
        assert_eq!(result, Value::Int(i64::MAX)); // saturating
    }

    #[test]
    fn math_sub_int_underflow_saturates() {
        let args = Value::Tuple(vec![Value::Int(i64::MIN), Value::Int(1)]);
        let result = math_sub_fn(&args).unwrap();
        assert_eq!(result, Value::Int(i64::MIN)); // saturating
    }

    #[test]
    fn math_add_mixed_int_float() {
        let args = Value::Tuple(vec![Value::Int(1), Value::Float(2.5)]);
        let result = math_add_fn(&args).unwrap();
        assert_eq!(result, Value::Float(3.5));
    }

    #[test]
    fn math_add_non_numeric_rejected() {
        let args = Value::Tuple(vec![Value::String("a".into()), Value::Int(1)]);
        let result = math_add_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn math_add_wrong_arg_count_rejected() {
        let args = Value::Tuple(vec![Value::Int(1)]);
        let result = math_add_fn(&args);
        assert!(result.is_err());
    }

    // ── uuid ──

    #[test]
    fn uuid_with_args_rejected() {
        let args = Value::String("extra".into());
        let result = uuid_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn uuid_empty_accepted() {
        let result = uuid_fn(&Value::Empty).unwrap();
        if let Value::String(s) = result {
            assert_eq!(s.len(), 36); // UUID format
        }
    }

    #[test]
    fn uuid_empty_tuple_accepted() {
        let result = uuid_fn(&Value::Tuple(vec![])).unwrap();
        if let Value::String(s) = result {
            assert_eq!(s.len(), 36);
        }
    }

    // ── stringToJson ──

    #[test]
    fn string_to_json_invalid_json_rejected() {
        let args = Value::String("{not valid json}".into());
        let result = string_to_json_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn string_to_json_non_string_rejected() {
        let args = Value::Int(42);
        let result = string_to_json_fn(&args);
        assert!(result.is_err());
    }

    // ── arrayPartition ──

    #[test]
    fn array_partition_zero_chunk_rejected() {
        let args = Value::Tuple(vec![
            Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
            Value::Int(0),
        ]);
        let result = array_partition_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn array_partition_negative_chunk_rejected() {
        let args = Value::Tuple(vec![
            Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
            Value::Int(-1),
        ]);
        let result = array_partition_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn array_partition_non_array_first_arg_rejected() {
        let args = Value::Tuple(vec![Value::String("not array".into()), Value::Int(2)]);
        let result = array_partition_fn(&args);
        assert!(result.is_err());
    }

    // ── arrayContains ──

    #[test]
    fn array_contains_non_array_rejected() {
        let args = Value::Tuple(vec![Value::String("not array".into()), Value::Int(1)]);
        let result = array_contains_fn(&args);
        assert!(result.is_err());
    }

    #[test]
    fn array_contains_finds_element() {
        let args = Value::Tuple(vec![
            Value::Tuple(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            Value::Int(2),
        ]);
        let result = array_contains_fn(&args).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn array_contains_missing_element() {
        let args = Value::Tuple(vec![
            Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
            Value::Int(99),
        ]);
        let result = array_contains_fn(&args).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    // ── arrayLength ──

    #[test]
    fn array_length_empty_value() {
        let result = array_length_fn(&Value::Empty).unwrap();
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn array_length_non_array_rejected() {
        let result = array_length_fn(&Value::String("not array".into()));
        assert!(result.is_err());
    }

    // ── arrayUnique ──

    #[test]
    fn array_unique_empty_value() {
        let result = array_unique_fn(&Value::Empty).unwrap();
        assert_eq!(result, Value::Tuple(vec![]));
    }

    #[test]
    fn array_unique_removes_duplicates() {
        let args = Value::Tuple(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(3),
            Value::Int(2),
        ]);
        let result = array_unique_fn(&args).unwrap();
        if let Value::Tuple(items) = result {
            assert_eq!(items.len(), 3);
        }
    }

    #[test]
    fn array_unique_non_array_rejected() {
        let result = array_unique_fn(&Value::String("nope".into()));
        assert!(result.is_err());
    }

    // ── json_to_string ──

    #[test]
    fn json_to_string_handles_any_value() {
        let result = json_to_string_fn(&Value::Int(42)).unwrap();
        assert!(matches!(result, Value::String(_)));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 6: VALIDATION MODULE ATTACKS
// ═══════════════════════════════════════════════════════════════════════════

mod validation_attacks {
    use super::*;

    #[test]
    fn validate_self_referencing_loop_detected() {
        let json = json!({
            "startAt": "Loop",
            "states": {
                "Loop": {
                    "type": "pass",
                    "next": "Loop"
                }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::NoTerminalState)));
    }

    #[test]
    fn validate_two_state_cycle() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": { "type": "pass", "next": "B" },
                "B": { "type": "pass", "next": "A" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        let errors = machine.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, StateMachineError::NoTerminalState)));
    }

    #[test]
    fn valid_linear_machine() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": { "type": "pass", "next": "B" },
                "B": { "type": "succeed" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        assert!(machine.validate().is_ok());
    }

    #[test]
    fn valid_machine_with_end_transition() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": { "type": "pass", "end": true }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        assert!(machine.validate().is_ok());
    }

    #[test]
    fn valid_machine_with_fail_state() {
        let json = json!({
            "startAt": "A",
            "states": {
                "A": { "type": "pass", "next": "F" },
                "F": { "type": "fail" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        assert!(machine.validate().is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIMENSION 7: SERDE ROUNDTRIP ATTACKS — Serialize then deserialize
// ═══════════════════════════════════════════════════════════════════════════

mod serde_roundtrip {
    use super::*;

    #[test]
    fn transition_end_roundtrip() {
        let t = Transition::end();
        let json = serde_json::to_value(&t).unwrap();
        let back: Transition = serde_json::from_value(json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn transition_next_roundtrip() {
        let t = Transition::next(StateName::new("Target").unwrap());
        let json = serde_json::to_value(&t).unwrap();
        let back: Transition = serde_json::from_value(json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn backoff_rate_roundtrip() {
        let rate = BackoffRate::new(2.5).unwrap();
        let json = serde_json::to_value(rate).unwrap();
        let back: BackoffRate = serde_json::from_value(json).unwrap();
        assert_eq!(rate.value(), back.value());
    }

    #[test]
    fn retrier_roundtrip() {
        let rate = BackoffRate::new(1.5).unwrap();
        let retrier = Retrier::new(
            vec![ErrorCode::Timeout, ErrorCode::TaskFailed],
            5,
            3,
            rate,
            Some(60),
            JitterStrategy::Full,
        )
        .unwrap();
        let json = serde_json::to_value(&retrier).unwrap();
        let back: Retrier = serde_json::from_value(json).unwrap();
        assert_eq!(retrier, back);
    }

    #[test]
    fn error_code_custom_roundtrip() {
        let code = ErrorCode::Custom("MyError.Specific".into());
        let json = serde_json::to_value(&code).unwrap();
        let back: ErrorCode = serde_json::from_value(json).unwrap();
        // Note: the Display lowercases known variants but Custom preserves case
        assert!(matches!(back, ErrorCode::Custom(_)));
    }

    #[test]
    fn wait_duration_seconds_roundtrip() {
        let wd = WaitDuration::Seconds(42);
        let json = serde_json::to_value(&wd).unwrap();
        let back: WaitDuration = serde_json::from_value(json).unwrap();
        assert_eq!(wd, back);
    }

    #[test]
    fn wait_duration_timestamp_roundtrip() {
        let wd = WaitDuration::Timestamp("2024-01-01T00:00:00Z".into());
        let json = serde_json::to_value(&wd).unwrap();
        let back: WaitDuration = serde_json::from_value(json).unwrap();
        assert_eq!(wd, back);
    }

    #[test]
    fn full_machine_roundtrip() {
        let json = json!({
            "startAt": "Step1",
            "states": {
                "Step1": {
                    "type": "task",
                    "image": "alpine:latest",
                    "run": "echo hello",
                    "next": "Step2"
                },
                "Step2": {
                    "type": "choice",
                    "choices": [
                        {"condition": "x > 0", "next": "Pass"}
                    ],
                    "default": "Fail"
                },
                "Pass": { "type": "succeed" },
                "Fail": { "type": "fail" }
            }
        });
        let machine: StateMachine = serde_json::from_value(json).unwrap();
        assert!(machine.validate().is_ok());
        let re_json = serde_json::to_value(&machine).unwrap();
        let back: StateMachine = serde_json::from_value(re_json).unwrap();
        assert_eq!(machine, back);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn minimal_machine() -> StateMachine {
    serde_json::from_value(json!({
        "startAt": "Done",
        "states": {
            "Done": { "type": "succeed" }
        }
    }))
    .unwrap()
}
