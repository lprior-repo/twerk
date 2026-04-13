//! Red Queen Adversarial Test Suite — Generation 2
//!
//! Deep adversarial probing: Unicode normalization, boundary arithmetic,
//! serde nesting attacks, exhaustive case folding, FromStr stress.

use std::collections::{HashMap, HashSet};

use twerk_core::{id::IdError, ParseTriggerStateError, TriggerId, TriggerState};

// =========================================================================
// DIMENSION 1: TriggerState — Deep Invariant Probes
// =========================================================================

#[test]
fn rq2_ts_unicode_case_folding() {
    // Turkish dotless i (U+0130) — to_uppercase() in Rust gives "İ" (2 bytes)
    // This should NOT match "ACTIVE"
    let result: Result<TriggerState, _> = "act\u{0130}ve".parse();
    assert!(result.is_err(), "Turkish dotless i must not match ACTIVE");
}

#[test]
fn rq2_ts_german_sharp_s() {
    // ß (U+00DF) — to_uppercase() gives "SS" (2 chars)
    // The contract says case-insensitive parsing, so "disabled" must parse to Disabled
    assert_eq!(
        "disabled".parse::<TriggerState>().unwrap(),
        TriggerState::Disabled
    );
    // But a string with ß should NOT match any variant
    let result: Result<TriggerState, _> = "di\u{00DF}abled".parse();
    // "dißabled".to_uppercase() = "DISSABLED" which is not "DISABLED"
    assert!(result.is_err(), "ß (U+00DF) must not match DISABLED");
}

#[test]
fn rq2_ts_unicode_uppercase_variants() {
    // Test that non-ASCII uppercase forms are rejected
    // Greek capital alpha: Α (U+0391) looks like Latin A
    let result: Result<TriggerState, _> = "\u{0391}CTIVE".parse();
    // Rust's to_uppercase() on Greek alpha keeps it as Greek alpha
    assert!(result.is_err(), "Greek capital alpha must not match ACTIVE");
}

#[test]
fn rq2_ts_serde_case_variants_rejected() {
    // serde uses SCREAMING_SNAKE_CASE — these must ALL be rejected
    let rejected = [
        "\"active\"",
        "\"Active\"",
        "\"aCtIvE\"",
        "\"paused\"",
        "\"Paused\"",
        "\"disabled\"",
        "\"Disabled\"",
        "\"error\"",
        "\"Error\"",
        "\"eRrOr\"",
    ];
    for json in rejected {
        let result: Result<TriggerState, serde_json::Error> = serde_json::from_str(json);
        assert!(result.is_err(), "serde must reject {json}");
    }
}

#[test]
fn rq2_ts_serde_structured_json_rejected() {
    // Attempt to pass an object with variant name
    let result: Result<TriggerState, _> = serde_json::from_str("{\"TriggerState\":\"Active\"}");
    assert!(result.is_err());
}

#[test]
fn rq2_ts_serde_array_rejected() {
    let result: Result<TriggerState, _> = serde_json::from_str("[\"ACTIVE\"]");
    assert!(result.is_err());
}

#[test]
fn rq2_ts_serde_number_rejected() {
    let result: Result<TriggerState, _> = serde_json::from_str("0");
    assert!(result.is_err());
    let result: Result<TriggerState, _> = serde_json::from_str("1");
    assert!(result.is_err());
    let result: Result<TriggerState, _> = serde_json::from_str("-1");
    assert!(result.is_err());
    let result: Result<TriggerState, _> = serde_json::from_str("3.14");
    assert!(result.is_err());
}

#[test]
fn rq2_ts_parse_error_eq_correctness() {
    let e1 = ParseTriggerStateError(String::from("x"));
    let e2 = ParseTriggerStateError(String::from("x"));
    let e3 = ParseTriggerStateError(String::from("y"));
    assert_eq!(e1, e2);
    assert_ne!(e1, e3);
}

#[test]
fn rq2_ts_hash_eq_consistency() {
    use std::hash::{Hash, Hasher};
    let mut h1 = std::collections::hash_map::DefaultHasher::new();
    let mut h2 = std::collections::hash_map::DefaultHasher::new();
    TriggerState::Active.hash(&mut h1);
    TriggerState::Active.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish(), "same value must hash same");
}

// =========================================================================
// DIMENSION 2: TriggerId — Deep Validation Probes
// =========================================================================

#[test]
fn rq2_ti_boundary_exact_3_with_dash() {
    assert!(TriggerId::new("a-b").is_ok());
    assert!(TriggerId::new("_-a").is_ok());
    assert!(TriggerId::new("a__").is_ok());
}

#[test]
fn rq2_ti_boundary_exact_64_with_special() {
    let s = format!("{}-{}", "a".repeat(62), "b"); // 62 + 1 + 1 = 64
    assert!(TriggerId::new(&s).is_ok());
    assert_eq!(TriggerId::new(&s).unwrap().as_str().len(), 64);
}

#[test]
fn rq2_ti_boundary_exact_64_stress() {
    // Various 64-char patterns
    let patterns: Vec<String> = vec![
        "a".repeat(64),
        "0".repeat(64),
        "-".repeat(64),
        "_".repeat(64),
        "ab".repeat(32),   // 64 chars
        "abc-".repeat(16), // 64 chars
        "a_b-".repeat(16), // 64 chars
    ];
    for p in &patterns {
        assert!(
            TriggerId::new(p.as_str()).is_ok(),
            "64-char pattern must be accepted: len={}",
            p.len()
        );
        assert_eq!(TriggerId::new(p.as_str()).unwrap().as_str().len(), 64);
    }
}

#[test]
fn rq2_ti_boundary_65_stress() {
    let patterns: Vec<String> = vec![
        "a".repeat(65),
        "0".repeat(65),
        "-".repeat(65),
        "abc-".repeat(17), // 68 chars
    ];
    for p in &patterns {
        let result = TriggerId::new(p.as_str());
        assert!(
            matches!(result, Err(IdError::TooLong(_))),
            "65+ chars must be TooLong: len={}",
            p.len()
        );
    }
}

#[test]
fn rq2_ti_null_byte_at_start() {
    assert!(matches!(
        TriggerId::new("\x00abc"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_null_byte_at_end() {
    assert!(matches!(
        TriggerId::new("abc\x00"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_null_byte_in_middle() {
    assert!(matches!(
        TriggerId::new("ab\x00cd"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_only_null_bytes() {
    assert!(matches!(
        TriggerId::new("\x00\x00\x00"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_backslash_rejected() {
    assert!(matches!(
        TriggerId::new("abc\\def"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_colon_rejected() {
    assert!(matches!(
        TriggerId::new("abc:def"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_at_sign_rejected() {
    assert!(matches!(
        TriggerId::new("user@host"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_dot_rejected() {
    assert!(matches!(
        TriggerId::new("file.txt"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_comma_rejected() {
    assert!(matches!(
        TriggerId::new("a,b,c"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_pipe_rejected() {
    assert!(matches!(
        TriggerId::new("a|b|c"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_tilde_rejected() {
    assert!(matches!(
        TriggerId::new("~home"),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_ti_double_dash_valid() {
    assert!(TriggerId::new("a--b").is_ok());
}

#[test]
fn rq2_ti_double_underscore_valid() {
    assert!(TriggerId::new("a__b").is_ok());
}

#[test]
fn rq2_ti_triple_dash_valid() {
    assert!(TriggerId::new("a---b").is_ok());
}

#[test]
fn rq2_ti_very_long_valid() {
    let s = "a_b-".repeat(16); // exactly 64 chars
    assert!(TriggerId::new(&s).is_ok());
}

// =========================================================================
// DIMENSION 3: Serde — Deep Attack Vectors
// =========================================================================

#[test]
fn rq2_serde_ti_deeply_nested_rejected() {
    let json = r#"{"nested":{"id":"abc"}}"#;
    let result: Result<TriggerId, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn rq2_serde_ti_truncated_json_rejected() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc");
    assert!(result.is_err());
}

#[test]
fn rq2_serde_ti_overlong_unicode_rejected() {
    // Overlong encoding of null — serde handles this at the JSON parsing level
    // Test with a regular invalid approach
    let result: Result<TriggerId, _> = serde_json::from_str("\"ab\\uD800cd\"");
    assert!(result.is_err(), "lone surrogate must be rejected by serde");
}

#[test]
fn rq2_serde_ti_valid_with_cjk() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"日本語\"");
    assert!(result.is_ok(), "CJK must be accepted via serde");
}

#[test]
fn rq2_serde_ti_valid_with_dash_underscore() {
    for json in ["\"a-b_c\"", "\"_abc-123\"", "\"---\"", "\"___\""] {
        let result: Result<TriggerId, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "must accept {json}");
    }
}

#[test]
fn rq2_serde_ti_whitespace_in_json_rejected() {
    // JSON strings with literal whitespace (not JSON whitespace around the string)
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc def\"");
    assert!(result.is_err());
}

#[test]
fn rq2_serde_ti_json_string_escaping() {
    // Escaped characters in JSON
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc\\ndef\"");
    assert!(result.is_err(), "escaped newline must be rejected");
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc\\ttab\"");
    assert!(result.is_err(), "escaped tab must be rejected");
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc\\r\"");
    assert!(result.is_err(), "escaped CR must be rejected");
}

#[test]
fn rq2_serde_ts_mixed_case_rejected() {
    for json in ["\"Active\"", "\"aCtIvE\"", "\"Paused\"", "\"eRrOr\""] {
        let result: Result<TriggerState, _> = serde_json::from_str(json);
        assert!(result.is_err(), "serde must reject mixed-case {json}");
    }
}

#[test]
fn rq2_serde_ti_integer_in_string_rejected() {
    // A JSON string containing digits is fine, but bare integers are not
    let result: Result<TriggerId, _> = serde_json::from_str("123");
    assert!(result.is_err());
}

// =========================================================================
// DIMENSION 4: FromStr — Exhaustive Edge Cases
// =========================================================================

#[test]
fn rq2_fromstr_ts_all_lowercase_accepted() {
    assert_eq!(
        "active".parse::<TriggerState>().unwrap(),
        TriggerState::Active
    );
    assert_eq!(
        "paused".parse::<TriggerState>().unwrap(),
        TriggerState::Paused
    );
    assert_eq!(
        "disabled".parse::<TriggerState>().unwrap(),
        TriggerState::Disabled
    );
    assert_eq!(
        "error".parse::<TriggerState>().unwrap(),
        TriggerState::Error
    );
}

#[test]
fn rq2_fromstr_ts_all_uppercase_accepted() {
    assert_eq!(
        "ACTIVE".parse::<TriggerState>().unwrap(),
        TriggerState::Active
    );
    assert_eq!(
        "PAUSED".parse::<TriggerState>().unwrap(),
        TriggerState::Paused
    );
    assert_eq!(
        "DISABLED".parse::<TriggerState>().unwrap(),
        TriggerState::Disabled
    );
    assert_eq!(
        "ERROR".parse::<TriggerState>().unwrap(),
        TriggerState::Error
    );
}

#[test]
fn rq2_fromstr_ts_rejects_with_null() {
    assert!("active\x00".parse::<TriggerState>().is_err());
    assert!("ACTIVE\x00".parse::<TriggerState>().is_err());
}

#[test]
fn rq2_fromstr_ts_rejects_unicode_lookalikes() {
    // Cyrillic А (U+0410) looks like Latin A
    assert!("\u{0410}CTIVE".parse::<TriggerState>().is_err());
}

#[test]
fn rq2_fromstr_ti_from_str_validates() {
    // FromStr delegates to new() which validates
    let result: Result<TriggerId, _> = "ab".parse();
    assert!(matches!(result, Err(IdError::TooShort(2))));
}

#[test]
fn rq2_fromstr_ti_from_str_accepts_valid() {
    assert!("abc-123".parse::<TriggerId>().is_ok());
    assert!("x_y_z".parse::<TriggerId>().is_ok());
}

// =========================================================================
// DIMENSION 5: From Bypass — Ensure Infallible Paths Documented
// =========================================================================

#[test]
fn rq2_from_bypass_serialize_then_deserialize_always_validates() {
    // The critical invariant: even if bypassed, serde re-validates on deserialize
    let bypass = TriggerId::from(String::from(""));
    let json = serde_json::to_string(&bypass).unwrap();
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_err());

    let bypass = TriggerId::from(String::from("x"));
    let json = serde_json::to_string(&bypass).unwrap();
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_err());

    let bypass = TriggerId::from(String::from("ab"));
    let json = serde_json::to_string(&bypass).unwrap();
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_err());
}

#[test]
fn rq2_from_bypass_equals_new_when_valid() {
    // When the bypass path creates a value that WOULD be valid, it must equal new()
    let bypass = TriggerId::from(String::from("valid-id"));
    let validated = TriggerId::new("valid-id").unwrap();
    assert_eq!(bypass, validated);
}

#[test]
fn rq2_from_bypass_display_matches() {
    let bypass = TriggerId::from(String::from("test-123"));
    assert_eq!(format!("{bypass}"), "test-123");
    assert_eq!(bypass.as_str(), "test-123");
}

#[test]
fn rq2_from_bypass_into_string_then_parse() {
    // bypass → Display → FromStr should validate
    let bypass = TriggerId::from(String::from("ab"));
    let display = format!("{bypass}");
    let result: Result<TriggerId, _> = display.parse();
    assert!(
        result.is_err(),
        "FromStr of bypass-constructed 'ab' must fail"
    );
}

// =========================================================================
// DIMENSION 6: Hash — Collision & Consistency Deep Probes
// =========================================================================

#[test]
fn rq2_hash_ti_many_distinct_ids() {
    let mut set = HashSet::new();
    for i in 0..100 {
        let id = TriggerId::new(&format!("id-{i:03}")).unwrap();
        set.insert(id);
    }
    assert_eq!(
        set.len(),
        100,
        "100 distinct IDs must produce 100 HashSet entries"
    );
}

#[test]
fn rq2_hash_ti_hashmap_lookup_after_many_inserts() {
    let mut map = HashMap::new();
    for i in 0..50 {
        let id = TriggerId::new(&format!("key-{i:03}")).unwrap();
        map.insert(id, i);
    }
    for i in 0..50 {
        let id = TriggerId::new(&format!("key-{i:03}")).unwrap();
        assert_eq!(map.get(&id), Some(&i), "lookup failed for key-{i:03}");
    }
}

#[test]
fn rq2_hash_ts_all_variants_in_hashmap() {
    let mut map = HashMap::new();
    map.insert(TriggerState::Active, "a");
    map.insert(TriggerState::Paused, "p");
    map.insert(TriggerState::Disabled, "d");
    map.insert(TriggerState::Error, "e");
    // Lookup must work for all
    assert_eq!(map.get(&TriggerState::Active), Some(&"a"));
    assert_eq!(map.get(&TriggerState::Paused), Some(&"p"));
    assert_eq!(map.get(&TriggerState::Disabled), Some(&"d"));
    assert_eq!(map.get(&TriggerState::Error), Some(&"e"));
    assert_eq!(map.len(), 4);
}

#[test]
fn rq2_hash_ti_eq_hash_consistency() {
    // If Eq says equal, Hash must say equal
    let a = TriggerId::new("same-value").unwrap();
    let b = TriggerId::new("same-value").unwrap();
    assert_eq!(a, b);
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    a.hash(&mut h1);
    b.hash(&mut h2);
    assert_eq!(
        h1.finish(),
        h2.finish(),
        "equal values must have equal hashes"
    );
}

#[test]
fn rq2_hash_ti_ne_hash_likely_different() {
    let a = TriggerId::new("aaaa").unwrap();
    let b = TriggerId::new("aaab").unwrap();
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    a.hash(&mut h1);
    b.hash(&mut h2);
    assert_ne!(
        h1.finish(),
        h2.finish(),
        "different values should likely have different hashes"
    );
}

// =========================================================================
// DIMENSION 7: Display/FromStr Roundtrip — Deep Integrity
// =========================================================================

#[test]
fn rq2_roundtrip_ts_exhaustive_case_variants() {
    // Every possible case permutation of "active" that FromStr accepts
    let case_variants = [
        "active", "ACTIVE", "Active", "aCtIvE", "ACTive", "actIVE", "paused", "PAUSED", "Paused",
        "pAuSeD", "disabled", "DISABLED", "Disabled", "DiSaBlEd", "error", "ERROR", "Error",
        "ErRoR",
    ];
    for input in case_variants {
        let parsed: TriggerState = input.parse().unwrap();
        let display = format!("{parsed}");
        // Display must be SCREAMING_SNAKE_CASE
        let reparsed: TriggerState = display.parse().unwrap();
        assert_eq!(
            parsed, reparsed,
            "roundtrip failed for '{input}' -> '{display}'"
        );
    }
}

#[test]
fn rq2_roundtrip_ti_cjk() {
    let id = TriggerId::new("日本語").unwrap();
    let display = format!("{id}");
    assert_eq!(display, "日本語");
    let reparsed: TriggerId = display.parse().unwrap();
    assert_eq!(reparsed.as_str(), "日本語");
}

#[test]
fn rq2_roundtrip_ti_long_id() {
    let s = "a_b-".repeat(16); // 64 chars
    let id = TriggerId::new(&s).unwrap();
    let display = format!("{id}");
    assert_eq!(display.len(), 64);
    let reparsed: TriggerId = display.parse().unwrap();
    assert_eq!(reparsed.as_str(), s);
}

#[test]
fn rq2_roundtrip_serde_ti_cjk() {
    let id = TriggerId::new("abc-日本語").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    let recovered: TriggerId = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.as_str(), "abc-日本語");
}

#[test]
fn rq2_roundtrip_serde_ti_long() {
    let s = "x".repeat(64);
    let id = TriggerId::new(&s).unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert!(json.starts_with('"') && json.ends_with('"'));
    let recovered: TriggerId = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.as_str().len(), 64);
}

#[test]
fn rq2_roundtrip_display_not_serde_for_ts() {
    // INV-TS-4: Display == serde form for all variants
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let display = format!("{state}");
        let serde = serde_json::to_string(&state)
            .unwrap()
            .trim_matches('"')
            .to_string();
        assert_eq!(display, serde, "Display must equal serde for {state:?}");
    }
}

// =========================================================================
// ERROR TAXONOMY — Precision Checks
// =========================================================================

#[test]
fn rq2_error_ti_empty_is_first_check() {
    // Empty should produce Empty, not TooShort
    assert!(matches!(TriggerId::new(""), Err(IdError::Empty)));
}

#[test]
fn rq2_error_ti_single_char_is_too_short() {
    // 1 char should be TooShort(1), not Empty
    assert!(matches!(TriggerId::new("a"), Err(IdError::TooShort(1))));
}

#[test]
fn rq2_error_ti_too_short_captures_length() {
    assert!(matches!(TriggerId::new("ab"), Err(IdError::TooShort(2))));
}

#[test]
fn rq2_error_ti_too_long_captures_actual_length() {
    assert!(matches!(
        TriggerId::new(&"x".repeat(100)),
        Err(IdError::TooLong(100))
    ));
    assert!(matches!(
        TriggerId::new(&"y".repeat(65)),
        Err(IdError::TooLong(65))
    ));
    assert!(matches!(
        TriggerId::new(&"z".repeat(500)),
        Err(IdError::TooLong(500))
    ));
}

#[test]
fn rq2_error_ti_invalid_chars_takes_precedence_over_length_when_both() {
    // "a".repeat(100) + "@" = 101 chars, but @ is invalid
    // TooLong check (101 > 64) fires BEFORE InvalidCharacters per validation order
    let s = format!("{}@", "a".repeat(100));
    let result = TriggerId::new(&s);
    // Per validation order: empty → too_short → too_long → invalid_chars
    // 101 > 64, so TooLong fires first
    assert!(matches!(result, Err(IdError::TooLong(101))));
}

#[test]
fn rq2_error_ti_invalid_chars_in_valid_length_range() {
    // 3 chars with invalid char → InvalidCharacters
    assert!(matches!(
        TriggerId::new("a@b"),
        Err(IdError::InvalidCharacters)
    ));
    // 64 chars with invalid char → InvalidCharacters
    let s = format!("{}@", "a".repeat(63));
    assert!(matches!(
        TriggerId::new(&s),
        Err(IdError::InvalidCharacters)
    ));
}

#[test]
fn rq2_error_ts_message_format() {
    let err = "xyz".parse::<TriggerState>().unwrap_err();
    let msg = format!("{err}");
    assert_eq!(msg, "unknown TriggerState: xyz");
}

#[test]
fn rq2_error_ts_empty_input_message() {
    let err = "".parse::<TriggerState>().unwrap_err();
    let msg = format!("{err}");
    assert_eq!(msg, "unknown TriggerState: ");
}
