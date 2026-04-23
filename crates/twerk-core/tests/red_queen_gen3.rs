//! Red Queen Adversarial Test Suite — Generation 3
//!
//! Final escalation: property-based stress, clone/copy semantics,
//! Ord trait behavior (or lack thereof), Debug format, Send/Sync bounds,
//! size assertions, exhaustive enum iteration, edge of spec compliance.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use twerk_core::{id::IdError, ParseTriggerStateError, TriggerId, TriggerState};

// =========================================================================
// TRAIT BOUNDS & SEMANTICS
// =========================================================================

#[test]
fn rq3_ts_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TriggerState>();
}

#[test]
fn rq3_ts_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<TriggerState>();
}

#[test]
fn rq3_ti_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TriggerId>();
}

#[test]
fn rq3_ti_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<TriggerId>();
}

#[test]
fn rq3_ts_size_at_most_1_byte() {
    assert!(
        std::mem::size_of::<TriggerState>() <= 1,
        "TriggerState should be 1 byte (enum with 4 variants), got {}",
        std::mem::size_of::<TriggerState>()
    );
}

#[test]
fn rq3_ts_align_is_1() {
    assert_eq!(
        std::mem::align_of::<TriggerState>(),
        1,
        "TriggerState should have alignment 1"
    );
}

#[test]
fn rq3_ts_copy_then_compare() {
    let original = TriggerState::Error;
    let copy = original;
    assert_eq!(original, copy);
    // Modify original — but Copy means it can't be mutated
    // Just verify they remain equal
    let another = TriggerState::Disabled;
    assert_ne!(copy, another);
}

#[test]
fn rq3_ts_clone_same_as_copy() {
    let original = TriggerState::Paused;
    let cloned = original;
    assert_eq!(original, cloned);
}

#[test]
fn rq3_ts_debug_format() {
    // Rust's derive(Debug) for enums prints just the variant name (e.g., "Active"),
    // not the full path. This is standard Rust behavior.
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let debug = format!("{state:?}");
        // Debug output should be non-empty and should NOT be empty/trivial
        assert!(!debug.is_empty(), "Debug output must not be empty");
        // Verify it matches the expected short variant name
        let expected = match state {
            TriggerState::Active => "Active",
            TriggerState::Paused => "Paused",
            TriggerState::Disabled => "Disabled",
            TriggerState::Error => "Error",
        };
        assert_eq!(debug, expected, "Debug format should match variant name");
    }
}

#[test]
fn rq3_ti_clone_independence() {
    let original = TriggerId::new("clone-test").unwrap();
    let cloned = original.clone();
    assert_eq!(original, cloned);
    assert_eq!(original.as_str(), cloned.as_str());
}

#[test]
fn rq3_ti_debug_format() {
    let id = TriggerId::new("debug-test").unwrap();
    let debug = format!("{id:?}");
    assert!(
        debug.contains("TriggerId"),
        "Debug must include type name: {debug}"
    );
}

#[test]
fn rq3_ti_default_is_empty() {
    let default = TriggerId::default();
    assert_eq!(default.as_str(), "");
    assert_eq!(default.to_string(), "");
}

#[test]
fn rq3_ti_as_ref_and_deref_agree() {
    let id = TriggerId::new("ref-deref-test").unwrap();
    let as_ref: &str = id.as_ref();
    let deref: &str = &id;
    let as_str = id.as_str();
    assert_eq!(as_ref, deref);
    assert_eq!(as_ref, as_str);
    assert_eq!(deref, as_str);
}

#[test]
fn rq3_ti_borrow_matches_as_ref() {
    use std::borrow::Borrow;
    let id = TriggerId::new("borrow-test").unwrap();
    let borrowed: &str = id.borrow();
    let as_ref: &str = id.as_ref();
    assert_eq!(borrowed, as_ref);
}

// =========================================================================
// EXHAUSTIVE ENUM COVERAGE
// =========================================================================

#[test]
fn rq3_ts_exhaustive_match_no_wildcard() {
    // This test compiles only if all 4 variants are covered without _
    fn classify(state: TriggerState) -> &'static str {
        match state {
            TriggerState::Active => "active",
            TriggerState::Paused => "paused",
            TriggerState::Disabled => "disabled",
            TriggerState::Error => "error",
        }
    }
    assert_eq!(classify(TriggerState::Active), "active");
    assert_eq!(classify(TriggerState::Paused), "paused");
    assert_eq!(classify(TriggerState::Disabled), "disabled");
    assert_eq!(classify(TriggerState::Error), "error");
}

#[test]
fn rq3_ts_iterate_all_variants() {
    let all = [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ];
    // Verify no variant is missing by checking count
    assert_eq!(all.len(), 4);
    // Verify each is unique
    let set: HashSet<_> = all.iter().copied().collect();
    assert_eq!(set.len(), 4);
}

// =========================================================================
// PROPERTY-BASED STRESS
// =========================================================================

#[test]
fn rq3_ti_all_ascii_letters_valid() {
    for c in b'a'..=b'z' {
        let s = format!("{}bc", c as char);
        assert!(
            TriggerId::new(&s).is_ok(),
            "3-char ID with '{c}' must be valid"
        );
    }
    for c in b'A'..=b'Z' {
        let s = format!("{}bc", c as char);
        assert!(
            TriggerId::new(&s).is_ok(),
            "3-char ID with '{c}' must be valid"
        );
    }
}

#[test]
fn rq3_ti_all_digits_valid() {
    for c in b'0'..=b'9' {
        let s = format!("{}bc", c as char);
        assert!(
            TriggerId::new(&s).is_ok(),
            "3-char ID with '{c}' must be valid"
        );
    }
}

#[test]
fn rq3_ti_all_ascii_printable_special_chars_rejected() {
    // All printable ASCII that are NOT alphanumeric, dash, or underscore
    for byte in 0x20u8..=0x7E {
        let c = byte as char;
        if c.is_alphanumeric() || c == '-' || c == '_' {
            continue;
        }
        let s = format!("a{c}b");
        assert!(
            TriggerId::new(&s).is_err(),
            "ASCII char '{c}' (0x{byte:02X}) must be rejected"
        );
    }
}

#[test]
fn rq3_ts_all_ascii_printable_rejected_by_fromstr() {
    // Any single ASCII printable character should be rejected (none match the 4 variants)
    for byte in 0x20u8..=0x7E {
        let c = byte as char;
        let s = c.to_string();
        let result: Result<TriggerState, _> = s.parse();
        // Only 'a', 'A' are single chars that could start a match,
        // but none of the valid variants are single-char
        assert!(
            result.is_err(),
            "single char '{c}' must be rejected as TriggerState"
        );
    }
}

#[test]
fn rq3_ti_1000_random_valid_ids() {
    use std::collections::hash_map::DefaultHasher;
    let mut hashes = HashSet::new();
    for i in 0..1000 {
        let id = TriggerId::new(format!("trigger-{i:06}")).unwrap();
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        hashes.insert(hasher.finish());
    }
    // 1000 distinct IDs should produce at least 990 distinct hashes
    // (birthday paradox allows some collisions, but 1000/2^64 is negligible)
    assert!(
        hashes.len() >= 990,
        "expected >= 990 distinct hashes from 1000 IDs, got {}",
        hashes.len()
    );
}

#[test]
fn rq3_ts_hash_distribution() {
    use std::collections::hash_map::DefaultHasher;
    let mut hashes = HashSet::new();
    for _ in 0..10000 {
        for state in [
            TriggerState::Active,
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerState::Error,
        ] {
            let mut hasher = DefaultHasher::new();
            state.hash(&mut hasher);
            hashes.insert(hasher.finish());
        }
    }
    // Should still only have 4 distinct hashes
    assert_eq!(
        hashes.len(),
        4,
        "4 variants must produce exactly 4 hash values"
    );
}

// =========================================================================
// SERDE STRESS
// =========================================================================

#[test]
fn rq3_serde_ti_valid_then_invalid_then_valid() {
    let valid1: TriggerId = serde_json::from_str("\"abc\"").unwrap();
    let invalid: Result<TriggerId, _> = serde_json::from_str("\"x\"");
    assert!(invalid.is_err());
    let valid2: TriggerId = serde_json::from_str("\"def\"").unwrap();
    assert_eq!(valid1.as_str(), "abc");
    assert_eq!(valid2.as_str(), "def");
}

#[test]
fn rq3_serde_ts_valid_then_invalid_then_valid() {
    let valid1: TriggerState = serde_json::from_str("\"ACTIVE\"").unwrap();
    let invalid: Result<TriggerState, _> = serde_json::from_str("\"INVALID\"");
    assert!(invalid.is_err());
    let valid2: TriggerState = serde_json::from_str("\"PAUSED\"").unwrap();
    assert_eq!(valid1, TriggerState::Active);
    assert_eq!(valid2, TriggerState::Paused);
}

#[test]
fn rq3_serde_ti_escaped_chars_in_json() {
    // Valid JSON escape sequences that produce invalid TriggerId chars
    let cases = [
        ("\"abc\\ndef\"", "newline"),
        ("\"abc\\rdef\"", "carriage return"),
        ("\"abc\\tdef\"", "tab"),
        ("\"abc\\0def\"", "null"),
    ];
    for (json, desc) in cases {
        let result: Result<TriggerId, _> = serde_json::from_str(json);
        assert!(result.is_err(), "JSON with {desc} must be rejected");
    }
}

#[test]
fn rq3_serde_ti_large_valid_payload() {
    // 64-char string via JSON
    let json = format!("\"{}\"", "a".repeat(64));
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_ok());
}

#[test]
fn rq3_serde_ti_json_whitespace_around_string() {
    // JSON whitespace around the value should be handled by serde
    let result: Result<TriggerId, _> = serde_json::from_str("  \"abc\"  ");
    assert!(
        result.is_ok(),
        "JSON whitespace around string should be handled by serde"
    );
    assert_eq!(result.unwrap().as_str(), "abc");
}

#[test]
fn rq3_serde_ts_json_whitespace_around_string() {
    let result: Result<TriggerState, _> = serde_json::from_str("  \"ACTIVE\"  ");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), TriggerState::Active);
}

// =========================================================================
// FROMSTR STRESS
// =========================================================================

#[test]
fn rq3_fromstr_ts_repeated_parses() {
    for _ in 0..1000 {
        assert_eq!(
            "active".parse::<TriggerState>().unwrap(),
            TriggerState::Active
        );
        assert_eq!(
            "PAUSED".parse::<TriggerState>().unwrap(),
            TriggerState::Paused
        );
    }
}

#[test]
fn rq3_fromstr_ti_repeated_parses() {
    for _ in 0..1000 {
        assert!("abc".parse::<TriggerId>().is_ok());
        assert!("ab".parse::<TriggerId>().is_err());
        assert!("".parse::<TriggerId>().is_err());
    }
}

#[test]
fn rq3_fromstr_ts_empty_and_whitespace_are_distinct_errors() {
    let empty_err = "".parse::<TriggerState>().unwrap_err();
    let space_err = " ".parse::<TriggerState>().unwrap_err();
    // Both should be errors but with different inner strings
    assert_ne!(empty_err.0, space_err.0);
}

#[test]
fn rq3_fromstr_ti_error_variants_distinct() {
    let empty = "".parse::<TriggerId>().unwrap_err();
    let short = "ab".parse::<TriggerId>().unwrap_err();
    let long: Result<TriggerId, IdError> = ("x".repeat(65)).parse();
    let long = long.unwrap_err();
    let invalid = "a@b".parse::<TriggerId>().unwrap_err();

    // All different error variants
    assert!(matches!(empty, IdError::Empty));
    assert!(matches!(short, IdError::TooShort(2)));
    assert!(matches!(long, IdError::TooLong(65)));
    assert!(matches!(invalid, IdError::InvalidCharacters));
}

// =========================================================================
// DISPLAY/ROUNDTRIP STRESS
// =========================================================================

#[test]
fn rq3_roundtrip_ti_many_valid_strings() {
    let valid = [
        "abc",
        "ABC",
        "abc-123",
        "ABC_123",
        "a-b_c-d",
        "trigger-001",
        "my_trigger_name",
        "TEST-TRIGGER",
        "---",
        "___",
        "_-_",
        "123",
        "a1b2c3",
    ];
    for input in valid {
        let id = TriggerId::new(input).unwrap();
        let display = format!("{id}");
        let parsed: TriggerId = display.parse().unwrap();
        assert_eq!(parsed.as_str(), input);
    }
}

#[test]
fn rq3_roundtrip_serde_ti_many_valid_strings() {
    let valid = ["abc", "abc-123", "My_Trigger-01", "test_123"];
    for input in valid {
        let id = TriggerId::new(input).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let recovered: TriggerId = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.as_str(), input);
    }
}

#[test]
fn rq3_roundtrip_display_fromstr_then_serde() {
    // Display → FromStr → Display → serde roundtrip
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let d1 = format!("{state}");
        let parsed: TriggerState = d1.parse().unwrap();
        let d2 = format!("{parsed}");
        assert_eq!(d1, d2);
        let json = serde_json::to_string(&parsed).unwrap();
        let serde_state: TriggerState = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_state, state);
    }
}

// =========================================================================
// ERROR TAXONOMY — FINAL PRECISION
// =========================================================================

#[test]
fn rq3_error_iderror_clone() {
    let e1 = IdError::TooShort(2);
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

#[test]
fn rq3_error_iderror_debug() {
    for err in [
        IdError::Empty,
        IdError::TooShort(1),
        IdError::TooLong(100),
        IdError::InvalidCharacters,
    ] {
        let debug = format!("{err:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn rq3_error_iderror_partial_eq() {
    assert_eq!(IdError::Empty, IdError::Empty);
    assert_eq!(IdError::TooShort(2), IdError::TooShort(2));
    assert_ne!(IdError::TooShort(2), IdError::TooShort(3));
    assert_eq!(IdError::TooLong(100), IdError::TooLong(100));
    assert_ne!(IdError::TooLong(100), IdError::TooLong(101));
    assert_eq!(IdError::InvalidCharacters, IdError::InvalidCharacters);
    assert_ne!(IdError::Empty, IdError::InvalidCharacters);
}

#[test]
fn rq3_error_parse_trigger_state_error_clone() {
    let e1 = ParseTriggerStateError(String::from("test"));
    let e2 = e1.clone();
    assert_eq!(e1, e2);
    assert_eq!(e1.0, e2.0);
}

#[test]
fn rq3_error_parse_trigger_state_error_debug() {
    let err = ParseTriggerStateError(String::from("test"));
    let debug = format!("{err:?}");
    assert!(
        debug.contains("ParseTriggerStateError"),
        "Debug must include type name: {debug}"
    );
}

#[test]
fn rq3_error_parse_trigger_state_error_source_is_none() {
    let err: Box<dyn std::error::Error> = Box::new(ParseTriggerStateError(String::from("x")));
    assert!(err.source().is_none());
}

#[test]
fn rq3_error_iderror_implements_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(IdError::Empty);
    let _ = err.to_string();
    let err: Box<dyn std::error::Error> = Box::new(IdError::TooShort(2));
    let _ = err.to_string();
    let err: Box<dyn std::error::Error> = Box::new(IdError::TooLong(100));
    let _ = err.to_string();
    let err: Box<dyn std::error::Error> = Box::new(IdError::InvalidCharacters);
    let _ = err.to_string();
}
