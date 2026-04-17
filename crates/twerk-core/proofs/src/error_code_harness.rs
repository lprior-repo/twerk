use twerk_core::asl::error_code::ErrorCode;

#[kani::proof]
fn error_code_matches_is_reflexive() {
    let variants = [
        ErrorCode::All,
        ErrorCode::Timeout,
        ErrorCode::TaskFailed,
        ErrorCode::Permissions,
        ErrorCode::ResultPathMatchFailure,
        ErrorCode::ParameterPathFailure,
        ErrorCode::BranchFailed,
        ErrorCode::NoChoiceMatched,
        ErrorCode::IntrinsicFailure,
        ErrorCode::HeartbeatTimeout,
    ];

    for variant in variants {
        assert!(
            variant.matches(&variant),
            "ErrorCode::matches should be reflexive for all variants"
        );
    }
}

#[kani::proof]
fn error_code_matches_all_always_true() {
    let variants = [
        ErrorCode::All,
        ErrorCode::Timeout,
        ErrorCode::TaskFailed,
        ErrorCode::Permissions,
        ErrorCode::ResultPathMatchFailure,
        ErrorCode::ParameterPathFailure,
        ErrorCode::BranchFailed,
        ErrorCode::NoChoiceMatched,
        ErrorCode::IntrinsicFailure,
        ErrorCode::HeartbeatTimeout,
        ErrorCode::Custom("custom error".to_string()),
    ];

    let all = ErrorCode::All;
    for variant in variants {
        assert!(
            all.matches(&variant),
            "ErrorCode::All should match everything"
        );
    }
}

#[kani::proof]
fn error_code_matches_non_all_not_reflexive_for_custom() {
    let custom = ErrorCode::Custom("error1".to_string());
    let custom2 = ErrorCode::Custom("error2".to_string());

    assert!(
        !custom.matches(&custom2),
        "Different Custom errors should not match"
    );
    assert!(
        custom.matches(&custom),
        "Same Custom error should match itself"
    );
}

#[kani::proof]
fn error_code_serialize_deserialize_roundtrip() {
    let variants = [
        ErrorCode::All,
        ErrorCode::Timeout,
        ErrorCode::TaskFailed,
        ErrorCode::Permissions,
        ErrorCode::ResultPathMatchFailure,
        ErrorCode::ParameterPathFailure,
        ErrorCode::BranchFailed,
        ErrorCode::NoChoiceMatched,
        ErrorCode::IntrinsicFailure,
        ErrorCode::HeartbeatTimeout,
        ErrorCode::Custom("custom_error".to_string()),
    ];

    for original in variants {
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ErrorCode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            original, deserialized,
            "Roundtrip should preserve ErrorCode"
        );
    }
}

#[kani::proof]
fn error_code_display_matches_known_variants() {
    let known = [
        (ErrorCode::All, "all"),
        (ErrorCode::Timeout, "timeout"),
        (ErrorCode::TaskFailed, "taskfailed"),
        (ErrorCode::Permissions, "permissions"),
        (ErrorCode::ResultPathMatchFailure, "resultpathmatchfailure"),
        (ErrorCode::ParameterPathFailure, "parameterpathfailure"),
        (ErrorCode::BranchFailed, "branchfailed"),
        (ErrorCode::NoChoiceMatched, "nochoicematched"),
        (ErrorCode::IntrinsicFailure, "intrinsicfailure"),
        (ErrorCode::HeartbeatTimeout, "heartbeattimeout"),
    ];

    for (code, expected) in known {
        assert_eq!(
            code.to_string(),
            expected,
            "Display should match expected string"
        );
    }
}
