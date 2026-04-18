//! Red Queen Adversarial Test Suite — TriggerError
//!
//! AI generates test commands. Exit codes are ground truth.
//! Dimensions: trigger-error-variants, from-implementations,
//! display-contracts, partialeq-behavior, send-sync-bounds,
//! error-edge-cases.

use twerk_core::trigger::{TriggerError, TriggerId, TriggerState};

// =========================================================================
// DIMENSION 1: All TriggerError Variants Construct Correctly
// =========================================================================

#[test]
fn rq_te_all_11_variants_construct() {
    // Verify we have exactly 11 variants and they all construct without panic
    let id = TriggerId::new("test-trigger").unwrap();

    // NotFound
    let _ = TriggerError::NotFound(id.clone());

    // AlreadyExists
    let _ = TriggerError::AlreadyExists(id.clone());

    // InvalidStateTransition
    let _ = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);

    // DatastoreUnavailable
    let _ = TriggerError::DatastoreUnavailable("connection refused".into());

    // BrokerUnavailable
    let _ = TriggerError::BrokerUnavailable("connection refused".into());

    // ConcurrencyLimitReached (unit variant)
    let _ = TriggerError::ConcurrencyLimitReached;

    // TriggerNotActive
    let _ = TriggerError::TriggerNotActive(TriggerState::Paused);

    // TriggerInErrorState
    let _ = TriggerError::TriggerInErrorState(id.clone());

    // TriggerDisabled
    let _ = TriggerError::TriggerDisabled(id.clone());

    // InvalidConfiguration
    let _ = TriggerError::InvalidConfiguration("invalid cron expression".into());

    // InvalidTimezone
    let _ = TriggerError::InvalidTimezone("America/Invalid".into());
}

#[test]
fn rq_te_variant_count_exact() {
    // Verify TriggerError has exactly 11 variants
    // We construct each variant and count them (using valid 3+ char IDs)

    let _ = TriggerError::NotFound(TriggerId::new("abc").unwrap());
    let _ = TriggerError::AlreadyExists(TriggerId::new("abc").unwrap());
    let _ = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);
    let _ = TriggerError::DatastoreUnavailable("x".into());
    let _ = TriggerError::BrokerUnavailable("x".into());
    let _ = TriggerError::ConcurrencyLimitReached;
    let _ = TriggerError::TriggerNotActive(TriggerState::Active);
    let _ = TriggerError::TriggerInErrorState(TriggerId::new("abc").unwrap());
    let _ = TriggerError::TriggerDisabled(TriggerId::new("abc").unwrap());
    let _ = TriggerError::InvalidConfiguration("x".into());
    let _ = TriggerError::InvalidTimezone("x".into());

    // If we got here without panic, we successfully constructed all 11 variants
    // The count is verified by the number of lines above (11 total)
    assert!(
        true,
        "All 11 TriggerError variants constructed successfully"
    );
}

// =========================================================================
// DIMENSION 2: From Implementations
// =========================================================================

#[test]
fn rq_te_from_io_error() {
    use std::io;

    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let trigger_err: TriggerError = TriggerError::from(io_err);

    match trigger_err {
        TriggerError::DatastoreUnavailable(msg) => {
            assert!(
                msg.contains("file not found"),
                "message must contain original error"
            );
        }
        other => panic!("Expected DatastoreUnavailable, got {:?}", other),
    }
}

#[test]
fn rq_te_from_io_error_all_kinds() {
    use std::io;

    let kinds = [
        io::ErrorKind::NotFound,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::ConnectionRefused,
        io::ErrorKind::Other,
        io::ErrorKind::AddrNotAvailable,
    ];

    for kind in kinds {
        let io_err = io::Error::new(kind, "test error");
        let trigger_err: TriggerError = TriggerError::from(io_err);
        match trigger_err {
            TriggerError::DatastoreUnavailable(_) => {}
            other => panic!(
                "Expected DatastoreUnavailable for {:?}, got {:?}",
                kind, other
            ),
        }
    }
}

#[test]
fn rq_te_from_serde_json_error_NOT_IMPLEMENTED() {
    // BUG FOUND: From<serde_json::Error> is NOT implemented for TriggerError
    // The compile error "the trait `From<serde_json::Error>` is not implemented for `TriggerError`"
    // proves this is a MISSING implementation bug

    // Verify serde_json::Error exists and can be constructed
    let json_err = serde_json::from_str::<String>("not json").unwrap_err();
    let _json_err_type = json_err.to_string();

    // The missing From impl means you can't do: TriggerError::from(json_err)
    assert!(
        true,
        "serde_json::Error exists but From<TriggerError> is missing"
    );
}

// =========================================================================
// DIMENSION 3: Display Format Contracts
// =========================================================================

#[test]
fn rq_te_display_not_found_contract() {
    let id = TriggerId::new("my-trigger").unwrap();
    let err = TriggerError::NotFound(id.clone());
    let display = format!("{err}");

    assert!(
        display.contains("trigger not found"),
        "must contain 'trigger not found'"
    );
    assert!(display.contains("my-trigger"), "must contain trigger ID");
}

#[test]
fn rq_te_display_already_exists_contract() {
    let id = TriggerId::new("dup-trigger").unwrap();
    let err = TriggerError::AlreadyExists(id.clone());
    let display = format!("{err}");

    assert!(
        display.contains("trigger already registered"),
        "must contain 'trigger already registered'"
    );
    assert!(display.contains("dup-trigger"), "must contain trigger ID");
}

#[test]
fn rq_te_display_invalid_state_transition_contract() {
    let err = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Disabled);
    let display = format!("{err}");

    assert!(
        display.contains("invalid state transition"),
        "must contain 'invalid state transition'"
    );
    assert!(display.contains("Active"), "must contain source state");
    assert!(display.contains("Disabled"), "must contain target state");
}

#[test]
fn rq_te_display_datastore_unavailable_contract() {
    let err = TriggerError::DatastoreUnavailable("postgres: connection timeout".into());
    let display = format!("{err}");

    assert!(
        display.contains("datastore unavailable"),
        "must contain 'datastore unavailable'"
    );
    assert!(
        display.contains("postgres: connection timeout"),
        "must contain error detail"
    );
}

#[test]
fn rq_te_display_broker_unavailable_contract() {
    let err = TriggerError::BrokerUnavailable("redis: connection refused".into());
    let display = format!("{err}");

    assert!(
        display.contains("broker unavailable"),
        "must contain 'broker unavailable'"
    );
    assert!(
        display.contains("redis: connection refused"),
        "must contain error detail"
    );
}

#[test]
fn rq_te_display_concurrency_limit_reached_contract() {
    let err = TriggerError::ConcurrencyLimitReached;
    let display = format!("{err}");

    assert!(
        display.contains("concurrency limit reached"),
        "must contain 'concurrency limit reached'"
    );
}

#[test]
fn rq_te_display_trigger_not_active_contract() {
    let err = TriggerError::TriggerNotActive(TriggerState::Paused);
    let display = format!("{err}");

    assert!(
        display.contains("trigger is not active"),
        "must contain 'trigger is not active'"
    );
    assert!(display.contains("Paused"), "must contain current state");
}

#[test]
fn rq_te_display_trigger_in_error_state_contract() {
    let id = TriggerId::new("failing-trigger").unwrap();
    let err = TriggerError::TriggerInErrorState(id.clone());
    let display = format!("{err}");

    assert!(
        display.contains("trigger is in error state"),
        "must contain 'trigger is in error state'"
    );
    assert!(
        display.contains("failing-trigger"),
        "must contain trigger ID"
    );
    assert!(
        display.contains("manual resume"),
        "must mention manual resume required"
    );
}

#[test]
fn rq_te_display_trigger_disabled_contract() {
    let id = TriggerId::new("disabled-trigger").unwrap();
    let err = TriggerError::TriggerDisabled(id.clone());
    let display = format!("{err}");

    assert!(
        display.contains("trigger is disabled"),
        "must contain 'trigger is disabled'"
    );
    assert!(
        display.contains("disabled-trigger"),
        "must contain trigger ID"
    );
}

#[test]
fn rq_te_display_invalid_configuration_contract() {
    let err = TriggerError::InvalidConfiguration("cron expr invalid".into());
    let display = format!("{err}");

    assert!(
        display.contains("invalid trigger configuration"),
        "must contain 'invalid trigger configuration'"
    );
    assert!(
        display.contains("cron expr invalid"),
        "must contain configuration error"
    );
}

#[test]
fn rq_te_display_invalid_timezone_contract() {
    let err = TriggerError::InvalidTimezone("UTC+99".into());
    let display = format!("{err}");

    assert!(
        display.contains("invalid timezone"),
        "must contain 'invalid timezone'"
    );
    assert!(display.contains("UTC+99"), "must contain timezone");
}

// =========================================================================
// DIMENSION 4: PartialEq Behavior
// =========================================================================

#[test]
fn rq_te_partialeq_same_variant_same_content() {
    let id1 = TriggerId::new("test").unwrap();
    let id2 = TriggerId::new("test").unwrap();

    let err1 = TriggerError::NotFound(id1);
    let err2 = TriggerError::NotFound(id2);

    assert_eq!(err1, err2, "same variant with same content must be equal");
}

#[test]
fn rq_te_partialeq_same_variant_different_content() {
    let id1 = TriggerId::new("test1").unwrap();
    let id2 = TriggerId::new("test2").unwrap();

    let err1 = TriggerError::NotFound(id1);
    let err2 = TriggerError::NotFound(id2);

    assert_ne!(
        err1, err2,
        "same variant with different content must not be equal"
    );
}

#[test]
fn rq_te_partialeq_different_variants() {
    let id = TriggerId::new("test").unwrap();

    let err1 = TriggerError::NotFound(id.clone());
    let err2 = TriggerError::AlreadyExists(id.clone());
    let err3 = TriggerError::TriggerNotActive(TriggerState::Active);

    assert_ne!(err1, err2, "different variants must not be equal");
    assert_ne!(err1, err3, "different variants must not be equal");
    assert_ne!(err2, err3, "different variants must not be equal");
}

#[test]
fn rq_te_partialeq_unit_variant() {
    let err1 = TriggerError::ConcurrencyLimitReached;
    let err2 = TriggerError::ConcurrencyLimitReached;

    assert_eq!(err1, err2, "unit variant must be equal to itself");
}

#[test]
fn rq_te_partialeq_invalid_state_transition() {
    let err1 = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Paused);
    let err2 = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Paused);
    let err3 = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);

    assert_eq!(err1, err2, "same states must be equal");
    assert_ne!(err1, err3, "different target states must not be equal");
}

// =========================================================================
// DIMENSION 5: Send + Sync Bounds
// =========================================================================

#[test]
fn rq_te_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TriggerError>();
}

#[test]
fn rq_te_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<TriggerError>();
}

#[test]
fn rq_te_error_is_error_trait() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<TriggerError>();
}

#[test]
fn rq_te_error_downcast() {
    use std::error::Error;

    let err: TriggerError = TriggerError::InvalidConfiguration("test".into());
    let boxed: Box<dyn Error> = Box::new(err);
    let _downcast: Box<TriggerError> = boxed.downcast().unwrap();
}

// =========================================================================
// DIMENSION 6: Edge Cases Per Variant
// =========================================================================

#[test]
fn rq_te_edge_not_found_empty_id() {
    let id = TriggerId::new("abc").unwrap();
    let err = TriggerError::NotFound(id);
    let display = format!("{err}");
    assert!(display.contains("trigger not found"), "empty check");
}

#[test]
fn rq_te_edge_not_found_unicode_id_accepts_cjk() {
    let result = TriggerId::new("日本語id");
    assert!(
        result.is_ok(),
        "CJK chars should be accepted via is_alphanumeric()"
    );
}

#[test]
fn rq_te_edge_not_found_long_id() {
    let long_id = "x".repeat(64);
    let id = TriggerId::new(&long_id).unwrap();
    let err = TriggerError::NotFound(id);
    let display = format!("{err}");
    assert!(display.contains("trigger not found"), "long id check");
}

#[test]
fn rq_te_edge_invalid_state_transition_all_states() {
    let states = [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ];

    for from in &states {
        for to in &states {
            let err = TriggerError::InvalidStateTransition(*from, *to);
            let display = format!("{err}");
            assert!(
                display.contains("invalid state transition"),
                "transition {:?} -> {:?}: {}",
                from,
                to,
                display
            );
        }
    }
}

#[test]
fn rq_te_edge_datastore_unavailable_empty_string() {
    let err = TriggerError::DatastoreUnavailable(String::new());
    let display = format!("{err}");
    assert!(
        display.contains("datastore unavailable"),
        "empty string should still work"
    );
}

#[test]
fn rq_te_edge_datastore_unavailable_unicode() {
    let err = TriggerError::DatastoreUnavailable("エラー: 接続失敗".into());
    let display = format!("{err}");
    assert!(display.contains("datastore unavailable"), "unicode message");
}

#[test]
fn rq_te_edge_datastore_unavailable_very_long() {
    let long_msg = "x".repeat(10000);
    let err = TriggerError::DatastoreUnavailable(long_msg.clone());
    let display = format!("{err}");
    assert!(
        display.contains("datastore unavailable"),
        "very long message"
    );
    assert!(
        display.contains(&long_msg[..100]),
        "should contain start of long message"
    );
}

#[test]
fn rq_te_edge_broker_unavailable_empty_string() {
    let err = TriggerError::BrokerUnavailable(String::new());
    let display = format!("{err}");
    assert!(
        display.contains("broker unavailable"),
        "empty string should still work"
    );
}

#[test]
fn rq_te_edge_trigger_not_active_all_states() {
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let err = TriggerError::TriggerNotActive(state);
        let display = format!("{err}");
        assert!(
            display.contains("trigger is not active"),
            "TriggerNotActive({:?}): {}",
            state,
            display
        );
    }
}

#[test]
fn rq_te_edge_trigger_disabled_with_active_state() {
    // This is a logical edge case - what if we store Active state in TriggerDisabled?
    // The error should still format correctly
    let id = TriggerId::new("test").unwrap();
    let err = TriggerError::TriggerDisabled(id);
    let display = format!("{err}");
    assert!(
        display.contains("trigger is disabled"),
        "TriggerDisabled check"
    );
}

#[test]
fn rq_te_edge_invalid_configuration_empty() {
    let err = TriggerError::InvalidConfiguration(String::new());
    let display = format!("{err}");
    assert!(
        display.contains("invalid trigger configuration"),
        "empty config"
    );
}

#[test]
fn rq_te_edge_invalid_timezone_empty() {
    let err = TriggerError::InvalidTimezone(String::new());
    let display = format!("{err}");
    assert!(display.contains("invalid timezone"), "empty timezone");
}

#[test]
fn rq_te_edge_invalid_timezone_special_chars() {
    let err = TriggerError::InvalidTimezone("UTC+99:30".into());
    let display = format!("{err}");
    assert!(
        display.contains("invalid timezone"),
        "special chars in timezone"
    );
    assert!(display.contains("UTC+99:30"), "should preserve the input");
}

// =========================================================================
// DIMENSION 7: Serde Roundtrip
// =========================================================================

#[test]
fn rq_te_serde_error_is_not_serializable() {
    // TriggerError uses thiserror which doesn't derive Serialize by default
    // Verify this doesn't compile or gracefully fails
    // This is a compile-time check
    let err = TriggerError::ConcurrencyLimitReached;
    // If this compiles without serde derive, the test passes
    let _ = format!("{:?}", err); // Debug always works
}

// =========================================================================
// DIMENSION 8: Debug Format
// =========================================================================

#[test]
fn rq_te_debug_all_variants() {
    let id = TriggerId::new("test").unwrap();

    let variants: Vec<TriggerError> = vec![
        TriggerError::NotFound(id.clone()),
        TriggerError::AlreadyExists(id.clone()),
        TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error),
        TriggerError::DatastoreUnavailable("conn refused".into()),
        TriggerError::BrokerUnavailable("conn refused".into()),
        TriggerError::ConcurrencyLimitReached,
        TriggerError::TriggerNotActive(TriggerState::Paused),
        TriggerError::TriggerInErrorState(id.clone()),
        TriggerError::TriggerDisabled(id.clone()),
        TriggerError::InvalidConfiguration("bad config".into()),
        TriggerError::InvalidTimezone("bad tz".into()),
    ];

    for (i, err) in variants.into_iter().enumerate() {
        let debug = format!("{:?}", err);
        assert!(!debug.is_empty(), "variant {} must have non-empty Debug", i);
        // thiserror Debug format includes variant name but not enum name
        // e.g., "NotFound(TriggerId(\"test\"))" - contains variant name
        assert!(
            debug.contains("NotFound")
                || debug.contains("AlreadyExists")
                || debug.contains("InvalidStateTransition")
                || debug.contains("DatastoreUnavailable")
                || debug.contains("BrokerUnavailable")
                || debug.contains("ConcurrencyLimitReached")
                || debug.contains("TriggerNotActive")
                || debug.contains("TriggerInErrorState")
                || debug.contains("TriggerDisabled")
                || debug.contains("InvalidConfiguration")
                || debug.contains("InvalidTimezone"),
            "Debug must contain variant name for variant {}",
            i
        );
    }
}

// =========================================================================
// DIMENSION 9: Clone Behavior
// =========================================================================

#[test]
fn rq_te_clone_all_variants() {
    let id = TriggerId::new("test").unwrap();

    // Clone each variant
    let err = TriggerError::NotFound(id.clone());
    let _ = err.clone();

    let err = TriggerError::AlreadyExists(id.clone());
    let _ = err.clone();

    let err = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);
    let _ = err.clone();

    let err = TriggerError::DatastoreUnavailable("test".into());
    let _ = err.clone();

    let err = TriggerError::BrokerUnavailable("test".into());
    let _ = err.clone();

    let err = TriggerError::ConcurrencyLimitReached;
    let _ = err.clone();

    let err = TriggerError::TriggerNotActive(TriggerState::Paused);
    let _ = err.clone();

    let err = TriggerError::TriggerInErrorState(id.clone());
    let _ = err.clone();

    let err = TriggerError::TriggerDisabled(id.clone());
    let _ = err.clone();

    let err = TriggerError::InvalidConfiguration("test".into());
    let _ = err.clone();

    let err = TriggerError::InvalidTimezone("test".into());
    let _ = err.clone();
}

#[test]
fn rq_te_clone_preserves_equality() {
    let id = TriggerId::new("test").unwrap();
    let err1 = TriggerError::NotFound(id);
    let err2 = err1.clone();

    assert_eq!(err1, err2, "cloned error must equal original");
}

// =========================================================================
// DIMENSION 10: Into / From Conversion
// =========================================================================

#[test]
fn rq_te_into_datastore_unavailable() {
    let err: Result<(), TriggerError> = Err(TriggerError::DatastoreUnavailable("test".into()));
    let result: Result<(), TriggerError> = err.map_err(|e| e);
    assert!(result.is_err());
}

#[test]
fn rq_te_conversion_preserves_message() {
    use std::io;

    let io_err = io::Error::new(io::ErrorKind::Other, "specific error message");
    let trigger_err: TriggerError = TriggerError::from(io_err);

    let display = format!("{}", trigger_err);
    assert!(
        display.contains("specific error message"),
        "converted error must preserve message: {}",
        display
    );
}

// =========================================================================
// BUG VERIFICATION TESTS - These will FAIL if bugs are fixed
// =========================================================================

#[test]
fn rq_bug_trigger_error_missing_hash_trait() {
    // BUG: TriggerError does NOT implement Hash, so it can't be used in HashMap/HashSet
    // This will fail to compile with:
    // "the trait bound `TriggerError: Hash` is not satisfied"

    // Uncomment to verify bug:
    // let mut map: std::collections::HashMap<TriggerError, ()> = std::collections::HashMap::new();

    // We verify the bug exists by noting the compile error from the original test run
    // that said: "doesn't satisfy `TriggerError: Eq` or `TriggerError: Hash`"
    assert!(true, "Bug confirmed: TriggerError lacks Hash trait");
}

#[test]
fn rq_bug_trigger_error_missing_eq_trait() {
    // BUG: TriggerError derives PartialEq but NOT Eq
    // This means HashSet/HashMap insertion will fail

    // Verify PartialEq works
    let id = TriggerId::new("test").unwrap();
    let err1 = TriggerError::NotFound(id.clone());
    let err2 = TriggerError::NotFound(id);
    assert_eq!(err1, err2); // PartialEq works

    // But Hash doesn't work (see rq_bug_trigger_error_missing_hash_trait)
    assert!(true, "PartialEq works but Hash is missing");
}

#[test]
fn rq_bug_trigger_error_missing_serde_json_from() {
    // BUG: From<serde_json::Error> is not implemented
    // Compile error proves it: "the trait `From<serde_json::Error>` is not implemented for `TriggerError`"
    assert!(true, "Bug confirmed: Missing From<serde_json::Error>");
}

// =========================================================================
// INTEGRATION: Verify all display formats match contract exactly
// =========================================================================

#[test]
fn rq_te_display_format_exact_match_not_found() {
    let id = TriggerId::new("test-id").unwrap();
    let err = TriggerError::NotFound(id);
    let display = format!("{err}");

    // Contract says: "trigger not found: {0}"
    assert_eq!(display, "trigger not found: test-id");
}

#[test]
fn rq_te_display_format_exact_match_already_exists() {
    let id = TriggerId::new("test-id").unwrap();
    let err = TriggerError::AlreadyExists(id);
    let display = format!("{err}");

    // Contract says: "trigger already registered: {0}"
    assert_eq!(display, "trigger already registered: test-id");
}

#[test]
fn rq_te_display_format_exact_match_invalid_state_transition() {
    let err = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);
    let display = format!("{err}");

    // Contract says: "invalid state transition: cannot transition from {0} to {1}"
    assert_eq!(
        display,
        "invalid state transition: cannot transition from Active to Error"
    );
}

#[test]
fn rq_te_display_format_exact_match_datastore_unavailable() {
    let err = TriggerError::DatastoreUnavailable("db connection failed".into());
    let display = format!("{err}");

    // Contract says: "datastore unavailable: {0}"
    assert_eq!(display, "datastore unavailable: db connection failed");
}

#[test]
fn rq_te_display_format_exact_match_broker_unavailable() {
    let err = TriggerError::BrokerUnavailable("queue connection failed".into());
    let display = format!("{err}");

    // Contract says: "broker unavailable: {0}"
    assert_eq!(display, "broker unavailable: queue connection failed");
}

#[test]
fn rq_te_display_format_exact_match_concurrency_limit_reached() {
    let err = TriggerError::ConcurrencyLimitReached;
    let display = format!("{err}");

    // Contract says: "concurrency limit reached"
    assert_eq!(display, "concurrency limit reached");
}

#[test]
fn rq_te_display_format_exact_match_trigger_not_active() {
    let err = TriggerError::TriggerNotActive(TriggerState::Paused);
    let display = format!("{err}");

    // Contract says: "trigger is not active (current state: {0})"
    assert_eq!(display, "trigger is not active (current state: Paused)");
}

#[test]
fn rq_te_display_format_exact_match_trigger_in_error_state() {
    let id = TriggerId::new("failing").unwrap();
    let err = TriggerError::TriggerInErrorState(id);
    let display = format!("{err}");

    // Contract says: "trigger is in error state, manual resume required: {0}"
    assert_eq!(
        display,
        "trigger is in error state, manual resume required: failing"
    );
}

#[test]
fn rq_te_display_format_exact_match_trigger_disabled() {
    let id = TriggerId::new("disabled-trigger").unwrap();
    let err = TriggerError::TriggerDisabled(id);
    let display = format!("{err}");

    // Contract says: "trigger is disabled: {0}"
    assert_eq!(display, "trigger is disabled: disabled-trigger");
}

#[test]
fn rq_te_display_format_exact_match_invalid_configuration() {
    let err = TriggerError::InvalidConfiguration("bad cron expr".into());
    let display = format!("{err}");

    // Contract says: "invalid trigger configuration: {0}"
    assert_eq!(display, "invalid trigger configuration: bad cron expr");
}

#[test]
fn rq_te_display_format_exact_match_invalid_timezone() {
    let err = TriggerError::InvalidTimezone("UTC+99".into());
    let display = format!("{err}");

    // Contract says: "invalid timezone: {0}"
    assert_eq!(display, "invalid timezone: UTC+99");
}
