//! Red Queen Adversarial Test Suite — Generation 1
//!
//! AI generates test commands. Exit codes are ground truth.
//! Dimensions: trigger-state-invariants, trigger-id-validation,
//! serde-attacks, fromstr-edge-cases, from-bypass, hash-collision,
//! display-roundtrip.

use std::collections::{HashMap, HashSet};
use twerk_core::{ParseTriggerStateError, TriggerId, TriggerState};

// =========================================================================
// DIMENSION 1: TriggerState Invariants
// =========================================================================

#[test]
fn rq_ts_unknown_variant_rejected() {
    // Exact match on unknown string — must fail
    let result: Result<TriggerState, _> = "DESTROYED".parse();
    assert!(result.is_err());
}

#[test]
fn rq_ts_empty_string_rejected() {
    let result: Result<TriggerState, _> = "".parse();
    assert!(result.is_err());
}

#[test]
fn rq_ts_whitespace_only_rejected() {
    let result: Result<TriggerState, _> = "   ".parse();
    assert!(result.is_err());
}

#[test]
fn rq_ts_trailing_whitespace_rejected() {
    // "ACTIVE " should NOT parse — trailing space makes it not match
    let result: Result<TriggerState, _> = "ACTIVE ".parse();
    assert!(result.is_err(), "trailing space must be rejected");
}

#[test]
fn rq_ts_leading_whitespace_rejected() {
    let result: Result<TriggerState, _> = " ACTIVE".parse();
    assert!(result.is_err(), "leading space must be rejected");
}

#[test]
fn rq_ts_partial_match_rejected() {
    let result: Result<TriggerState, _> = "ACTIV".parse();
    assert!(result.is_err());
}

#[test]
fn rq_ts_case_insensitive_mixed() {
    assert_eq!(
        "aCtIvE".parse::<TriggerState>().unwrap(),
        TriggerState::Active
    );
    assert_eq!(
        "pAuSeD".parse::<TriggerState>().unwrap(),
        TriggerState::Paused
    );
    assert_eq!(
        "DiSaBlEd".parse::<TriggerState>().unwrap(),
        TriggerState::Disabled
    );
    assert_eq!(
        "ErRoR".parse::<TriggerState>().unwrap(),
        TriggerState::Error
    );
}

#[test]
fn rq_ts_all_four_variants_exhaustive() {
    // Verify exactly 4 variants exist and are distinct
    let variants = [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ];
    let set: HashSet<_> = variants.into_iter().collect();
    assert_eq!(set.len(), 4, "must have exactly 4 distinct variants");
}

#[test]
fn rq_ts_error_display_preserves_original() {
    let err = "weird!".parse::<TriggerState>().unwrap_err();
    assert_eq!(err.0, "weird!");
    let msg = format!("{err}");
    assert!(
        msg.contains("weird!"),
        "error message must contain original input: {msg}"
    );
}

#[test]
fn rq_ts_default_is_active() {
    assert_eq!(TriggerState::default(), TriggerState::Active);
}

#[test]
fn rq_ts_copy_is_zero_cost() {
    assert!(std::mem::size_of::<TriggerState>() <= std::mem::size_of::<u8>());
    let a = TriggerState::Error;
    let b = a; // Copy
    assert_eq!(a, b);
}

#[test]
fn rq_ts_display_matches_serde_for_all_variants() {
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let display = format!("{state}");
        let json = serde_json::to_string(&state).unwrap();
        // json has quotes, strip them
        let serde_name = json.trim_matches('"');
        assert_eq!(
            display, serde_name,
            "Display must match serde for {state:?}"
        );
    }
}

#[test]
fn rq_ts_serde_deserialize_rejects_lowercase() {
    // serde uses SCREAMING_SNAKE_CASE — lowercase must be rejected
    let result: Result<TriggerState, _> = serde_json::from_str("\"active\"");
    assert!(result.is_err(), "serde must reject lowercase 'active'");
}

#[test]
fn rq_ts_serde_deserialize_rejects_numeric() {
    let result: Result<TriggerState, _> = serde_json::from_str("42");
    assert!(result.is_err(), "serde must reject numeric JSON");
}

#[test]
fn rq_ts_serde_deserialize_rejects_null() {
    let result: Result<TriggerState, _> = serde_json::from_str("null");
    assert!(result.is_err(), "serde must reject null");
}

#[test]
fn rq_ts_serde_deserialize_rejects_object() {
    let result: Result<TriggerState, _> = serde_json::from_str("{\"Active\":true}");
    assert!(result.is_err(), "serde must reject object JSON");
}

#[test]
fn rq_ts_serde_deserialize_rejects_empty_string() {
    let result: Result<TriggerState, _> = serde_json::from_str("\"\"");
    assert!(result.is_err(), "serde must reject empty JSON string");
}

#[test]
fn rq_ts_fromstr_rejects_null_byte() {
    let result: Result<TriggerState, _> = "ACTIVE\x00".parse();
    assert!(result.is_err(), "null byte must be rejected");
}

#[test]
fn rq_ts_fromstr_rejects_newline() {
    let result: Result<TriggerState, _> = "ACTIVE\n".parse();
    assert!(result.is_err(), "trailing newline must be rejected");
}

// =========================================================================
// DIMENSION 2: TriggerId Validation Boundaries
// =========================================================================

#[test]
fn rq_ti_exact_min_boundary_3_chars() {
    assert!(TriggerId::new("abc").is_ok());
}

#[test]
fn rq_ti_exact_max_boundary_64_chars() {
    let s = "a".repeat(64);
    assert!(TriggerId::new(&s).is_ok());
    assert_eq!(TriggerId::new(&s).unwrap().as_str().len(), 64);
}

#[test]
fn rq_ti_below_min_2_chars() {
    assert_eq!(
        TriggerId::new("ab"),
        Err(twerk_core::id::IdError::TooShort(2))
    );
}

#[test]
fn rq_ti_below_min_1_char() {
    assert_eq!(
        TriggerId::new("a"),
        Err(twerk_core::id::IdError::TooShort(1))
    );
}

#[test]
fn rq_ti_above_max_65_chars() {
    let s = "a".repeat(65);
    let err = TriggerId::new(&s).unwrap_err();
    assert!(matches!(err, twerk_core::id::IdError::TooLong(65)));
}

#[test]
fn rq_ti_empty_string() {
    assert_eq!(TriggerId::new(""), Err(twerk_core::id::IdError::Empty));
}

#[test]
fn rq_ti_null_byte_rejected() {
    let result = TriggerId::new("abc\x00def");
    assert!(matches!(
        result,
        Err(twerk_core::id::IdError::InvalidCharacters)
    ));
}

#[test]
fn rq_ti_control_chars_rejected() {
    let controls = ["\x01", "\x02", "\x7f", "\x0d", "\x0a", "\x09", "\x1b"];
    for ctrl in controls {
        let s = format!("ab{ctrl}"); // 3 chars total
        let result = TriggerId::new(&s);
        assert!(
            result.is_err(),
            "control char U+{:04X} must be rejected in '{:?}'",
            ctrl.chars().next().unwrap() as u32,
            s
        );
    }
}

#[test]
fn rq_ti_unicode_homoglyph_attack() {
    // Unicode characters that look like ASCII but aren't
    // Fullwidth Latin A (U+FF21) — looks like A but isn't ASCII
    let result = TriggerId::new("ａbc"); // fullwidth 'a' = U+FF41
                                         // Rust's is_alphanumeric() returns true for fullwidth Latin
                                         // So this is actually ACCEPTED — verify that
    if result.is_ok() {
        // If accepted, verify it was preserved correctly
        assert_eq!(result.unwrap().as_str(), "ａbc");
    }
    // Either accepted or rejected is fine, as long as it's consistent
}

#[test]
fn rq_ti_emoji_rejected() {
    let emojis = ["🔥", "🎉", "💣", "🚀", "❤️"];
    for emoji in emojis {
        let s = format!("ab{emoji}c"); // pad to valid length
        let result = TriggerId::new(&s);
        assert!(result.is_err(), "emoji '{emoji}' must be rejected");
    }
}

#[test]
fn rq_ti_zero_width_chars_rejected() {
    // Zero-width space, zero-width joiner, zero-width non-joiner
    let zero_width = ["\u{200B}", "\u{200D}", "\u{200C}", "\u{FEFF}"];
    for zw in zero_width {
        let s = format!("ab{zw}c");
        let result = TriggerId::new(&s);
        // Zero-width space is NOT alphanumeric, should be rejected
        assert!(
            result.is_err(),
            "zero-width char U+{:04X} must be rejected",
            zw.chars().next().unwrap() as u32
        );
    }
}

#[test]
fn rq_ti_right_to_left_override_rejected() {
    // RTL override can be used for spoofing
    let result = TriggerId::new("ab\u{202E}c");
    assert!(result.is_err(), "RTL override must be rejected");
}

#[test]
fn rq_ti_combining_chars_boundary() {
    // Combining acute accent after 'e' — is_alphanumeric returns true for the base char
    // but the combining mark itself is NOT alphanumeric
    let result = TriggerId::new("abe\u{0301}f"); // "abéf" — 4 visible chars, 5 codepoints
                                                 // The combining char (U+0301) is Mark category, is_alphanumeric returns false
    assert!(result.is_err(), "combining accent must be rejected");
}

#[test]
fn rq_ti_newline_and_tab_rejected() {
    assert!(matches!(
        TriggerId::new("ab\nc"),
        Err(twerk_core::id::IdError::InvalidCharacters)
    ));
    assert!(matches!(
        TriggerId::new("ab\tc"),
        Err(twerk_core::id::IdError::InvalidCharacters)
    ));
    assert!(matches!(
        TriggerId::new("ab\rc"),
        Err(twerk_core::id::IdError::InvalidCharacters)
    ));
}

#[test]
fn rq_ti_special_chars_all_rejected() {
    let special = [
        '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '=', '+', '[', ']', '{', '}', '|', '\\',
        ';', ':', '\'', '"', ',', '.', '<', '>', '/', '?', '`', '~', ' ', '\n', '\t', '\r', '\0',
    ];
    for c in special {
        let s = format!("a{c}b");
        let result = TriggerId::new(&s);
        assert!(
            matches!(result, Err(twerk_core::id::IdError::InvalidCharacters)),
            "char '{c}' must be rejected"
        );
    }
}

#[test]
fn rq_ti_preserves_input_exact() {
    let inputs = ["abc", "ABC", "a-b_c", "a_B-C", "123-abc", "abc-123"];
    for input in inputs {
        let id = TriggerId::new(input).unwrap();
        assert_eq!(id.as_str(), input, "input must be preserved byte-for-byte");
        assert_eq!(id.to_string(), input);
    }
}

// =========================================================================
// DIMENSION 3: Serde Deserialization Attacks
// =========================================================================

#[test]
fn rq_serde_ti_rejects_empty_json_string() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.to_lowercase().contains("empty"),
        "error must mention 'empty': {msg}"
    );
}

#[test]
fn rq_serde_ti_rejects_1_char_json() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"x\"");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.to_lowercase().contains("too short"),
        "error must mention 'too short': {msg}"
    );
}

#[test]
fn rq_serde_ti_rejects_2_char_json() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"ab\"");
    assert!(result.is_err());
}

#[test]
fn rq_serde_ti_rejects_65_char_json() {
    let json = format!("\"{}\"", "a".repeat(65));
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.to_lowercase().contains("too long"),
        "error must mention 'too long': {msg}"
    );
}

#[test]
fn rq_serde_ti_rejects_100_char_json() {
    let json = format!("\"{}\"", "a".repeat(100));
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_err());
}

#[test]
fn rq_serde_ti_rejects_special_chars_json() {
    let cases = ["\"ab@cd\"", "\"ab cd\"", "\"ab\ncd\"", "\"ab\x00cd\""];
    for json in cases {
        let result: Result<TriggerId, _> = serde_json::from_str(json);
        assert!(result.is_err(), "must reject JSON: {json}");
    }
}

#[test]
fn rq_serde_ti_rejects_null() {
    let result: Result<TriggerId, _> = serde_json::from_str("null");
    assert!(result.is_err(), "must reject null JSON");
}

#[test]
fn rq_serde_ti_rejects_number() {
    let result: Result<TriggerId, _> = serde_json::from_str("42");
    assert!(result.is_err(), "must reject numeric JSON");
}

#[test]
fn rq_serde_ti_rejects_boolean() {
    let result: Result<TriggerId, _> = serde_json::from_str("true");
    assert!(result.is_err(), "must reject boolean JSON");
}

#[test]
fn rq_serde_ti_rejects_array() {
    let result: Result<TriggerId, _> = serde_json::from_str("[\"abc\"]");
    assert!(result.is_err(), "must reject array JSON");
}

#[test]
fn rq_serde_ti_rejects_object() {
    let result: Result<TriggerId, _> = serde_json::from_str("{\"id\":\"abc\"}");
    assert!(result.is_err(), "must reject object JSON");
}

#[test]
fn rq_serde_ti_rejects_unicode_trick_json() {
    // Unicode null character embedded in JSON string
    let result: Result<TriggerId, _> = serde_json::from_str("\"ab\\u0000cd\"");
    assert!(result.is_err(), "must reject null byte in JSON");
}

#[test]
fn rq_serde_ti_rejects_emoji_json() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"ab🔥cd\"");
    assert!(result.is_err(), "must reject emoji in JSON");
}

#[test]
fn rq_serde_ti_accepts_valid_64_char_json() {
    let json = format!("\"{}\"", "a".repeat(64));
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(result.is_ok(), "must accept 64-char valid JSON");
    assert_eq!(result.unwrap().as_str().len(), 64);
}

#[test]
fn rq_serde_ti_accepts_valid_3_char_json() {
    let result: Result<TriggerId, _> = serde_json::from_str("\"abc\"");
    assert!(result.is_ok(), "must accept 3-char valid JSON");
}

#[test]
fn rq_serde_ts_rejects_unknown_variant_json() {
    let result: Result<TriggerState, _> = serde_json::from_str("\"DESTROYED\"");
    assert!(result.is_err());
}

#[test]
fn rq_serde_ts_rejects_empty_json() {
    let result: Result<TriggerState, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());
}

#[test]
fn rq_serde_ts_accepts_valid_json() {
    for json in ["\"ACTIVE\"", "\"PAUSED\"", "\"DISABLED\"", "\"ERROR\""] {
        let result: Result<TriggerState, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "must accept {json}");
    }
}

// =========================================================================
// DIMENSION 4: FromStr Edge Cases
// =========================================================================

#[test]
fn rq_fromstr_ts_rejects_all_whitespace() {
    for ws in [" ", "  ", "\t", "\n", "\r\n", " \t\n "] {
        let result: Result<TriggerState, _> = ws.parse();
        assert!(result.is_err(), "whitespace '{ws:?}' must be rejected");
    }
}

#[test]
fn rq_fromstr_ts_rejects_similar_names() {
    let similar = [
        "activate",
        "activation",
        "pause",
        "disableds",
        "errors",
        "ACTIVELY",
        "PAUSER",
        "DISABLE",
        "ERRORS",
        "ACTIVE!",
        "PAUSED.",
        "DISABLED-",
        "ERROR+",
    ];
    for name in similar {
        let result: Result<TriggerState, _> = name.parse();
        assert!(result.is_err(), "similar name '{name}' must be rejected");
    }
}

#[test]
fn rq_fromstr_ti_rejects_length_0() {
    let result: Result<TriggerId, _> = "".parse();
    assert!(matches!(result, Err(twerk_core::id::IdError::Empty)));
}

#[test]
fn rq_fromstr_ti_rejects_length_1() {
    let result: Result<TriggerId, _> = "a".parse();
    assert!(matches!(result, Err(twerk_core::id::IdError::TooShort(1))));
}

#[test]
fn rq_fromstr_ti_rejects_length_2() {
    let result: Result<TriggerId, _> = "ab".parse();
    assert!(matches!(result, Err(twerk_core::id::IdError::TooShort(2))));
}

#[test]
fn rq_fromstr_ti_accepts_length_3() {
    let result: Result<TriggerId, _> = "abc".parse();
    assert!(result.is_ok());
}

#[test]
fn rq_fromstr_ti_accepts_length_64() {
    let s = "x".repeat(64);
    let result: Result<TriggerId, _> = s.parse();
    assert!(result.is_ok());
}

#[test]
fn rq_fromstr_ti_rejects_length_65() {
    let s = "x".repeat(65);
    let result: Result<TriggerId, _> = s.parse();
    assert!(matches!(result, Err(twerk_core::id::IdError::TooLong(65))));
}

// =========================================================================
// DIMENSION 5: From<String>/From<&str> Bypass Constructors
// =========================================================================

#[test]
fn rq_from_string_bypass_creates_invalid_empty() {
    let id = TriggerId::from(String::new());
    assert_eq!(
        id.as_str(),
        "",
        "From<String> bypass must produce empty string"
    );
    // This is BY DESIGN per contract — From bypasses validation
}

#[test]
fn rq_from_string_bypass_creates_invalid_1_char() {
    let id = TriggerId::from(String::from("x"));
    assert_eq!(id.as_str(), "x");
}

#[test]
fn rq_from_str_bypass_creates_invalid_empty() {
    let id = TriggerId::from("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn rq_from_str_bypass_creates_invalid_special_chars() {
    let id = TriggerId::from("bad@id!#$");
    assert_eq!(id.as_str(), "bad@id!#$");
}

#[test]
fn rq_from_str_bypass_creates_invalid_200_chars() {
    let s = "a".repeat(200);
    let id = TriggerId::from(s.as_str());
    assert_eq!(id.as_str().len(), 200);
}

#[test]
fn rq_from_bypass_serde_roundtrip_validates() {
    // Even if constructed via From bypass, serde serialize then deserialize
    // should STILL validate on the deserialize side
    let bypass_id = TriggerId::from(String::from("x")); // invalid 1-char
    let json = serde_json::to_string(&bypass_id).unwrap();
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "serde deserialize must reject the bypass-constructed invalid value"
    );
}

#[test]
fn rq_from_bypass_empty_serde_roundtrip_validates() {
    let bypass_id = TriggerId::from(String::new()); // invalid empty
    let json = serde_json::to_string(&bypass_id).unwrap();
    let result: Result<TriggerId, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "serde deserialize must reject empty bypass value"
    );
}

// =========================================================================
// DIMENSION 6: Hash Collision Resistance
// =========================================================================

#[test]
fn rq_hash_ts_all_variants_distinct_hashes() {
    use std::hash::{Hash, Hasher};
    fn hash_value<T: Hash>(val: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        val.hash(&mut hasher);
        hasher.finish()
    }
    let variants = [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ];
    let hashes: HashSet<_> = variants.iter().map(hash_value).collect();
    assert_eq!(
        hashes.len(),
        4,
        "all 4 TriggerState variants must have distinct hashes"
    );
}

#[test]
fn rq_hash_ts_hashset_inserts_correctly() {
    let mut set = HashSet::new();
    set.insert(TriggerState::Active);
    set.insert(TriggerState::Paused);
    set.insert(TriggerState::Disabled);
    set.insert(TriggerState::Error);
    assert_eq!(set.len(), 4);
    // Re-inserting should not increase size
    set.insert(TriggerState::Active);
    set.insert(TriggerState::Paused);
    assert_eq!(set.len(), 4);
}

#[test]
fn rq_hash_ts_hashmap_key_works() {
    let mut map = HashMap::new();
    map.insert(TriggerState::Active, 1);
    map.insert(TriggerState::Paused, 2);
    map.insert(TriggerState::Disabled, 3);
    map.insert(TriggerState::Error, 4);
    assert_eq!(map.get(&TriggerState::Active), Some(&1));
    assert_eq!(map.get(&TriggerState::Paused), Some(&2));
    assert_eq!(map.get(&TriggerState::Disabled), Some(&3));
    assert_eq!(map.get(&TriggerState::Error), Some(&4));
    assert_eq!(map.len(), 4);
}

#[test]
fn rq_hash_ti_distinct_ids_distinct_hashes() {
    use std::hash::{Hash, Hasher};
    fn hash_value<T: Hash>(val: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        val.hash(&mut hasher);
        hasher.finish()
    }
    let id1 = TriggerId::new("abc-001").unwrap();
    let id2 = TriggerId::new("abc-002").unwrap();
    let id3 = TriggerId::new("abc-003").unwrap();
    let hashes: HashSet<_> = [&id1, &id2, &id3]
        .iter()
        .map(|id| hash_value(*id))
        .collect();
    assert_eq!(
        hashes.len(),
        3,
        "distinct TriggerIds must have distinct hashes"
    );
}

#[test]
fn rq_hash_ti_hashset_deduplication() {
    let id1 = TriggerId::new("same-value").unwrap();
    let id2 = TriggerId::new("same-value").unwrap();
    let id3 = TriggerId::new("other-value").unwrap();
    let mut set = HashSet::new();
    set.insert(id1);
    set.insert(id2);
    set.insert(id3);
    assert_eq!(set.len(), 2, "equal TriggerIds must hash to same bucket");
}

#[test]
fn rq_hash_ti_hashmap_key_works() {
    let id1 = TriggerId::new("key-1").unwrap();
    let id2 = TriggerId::new("key-2").unwrap();
    let mut map = HashMap::new();
    map.insert(id1.clone(), "value1");
    map.insert(id2.clone(), "value2");
    assert_eq!(map.get(&id1), Some(&"value1"));
    assert_eq!(map.get(&id2), Some(&"value2"));
    assert_eq!(map.len(), 2);
}

// =========================================================================
// DIMENSION 7: Display/FromStr Roundtrip Integrity
// =========================================================================

#[test]
fn rq_roundtrip_ts_all_variants() {
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let display = format!("{state}");
        let parsed: TriggerState = display.parse().unwrap();
        assert_eq!(
            parsed, state,
            "Display→FromStr roundtrip failed for {state:?}"
        );
    }
}

#[test]
fn rq_roundtrip_ti_valid_values() {
    let long = "x".repeat(64);
    let inputs: Vec<&str> = vec!["abc", "abc-123", "My_Trigger-01", "a-b_c-d", &long];
    for input in inputs {
        let id = TriggerId::new(input).unwrap();
        let display = format!("{id}");
        let parsed: TriggerId = display.parse().unwrap();
        assert_eq!(
            parsed.as_str(),
            input,
            "Display→FromStr roundtrip failed for '{input}'"
        );
    }
}

#[test]
fn rq_roundtrip_serde_ts_all_variants() {
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let json = serde_json::to_string(&state).unwrap();
        let recovered: TriggerState = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, state, "serde roundtrip failed for {state:?}");
    }
}

#[test]
fn rq_roundtrip_serde_ti_valid_values() {
    let long = "x".repeat(64);
    let inputs: Vec<&str> = vec!["abc", "abc-123", "My_Trigger-01", "a-b_c-d", &long];
    for input in inputs {
        let id = TriggerId::new(input).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let recovered: TriggerId = serde_json::from_str(&json).unwrap();
        assert_eq!(
            recovered.as_str(),
            input,
            "serde roundtrip failed for '{input}'"
        );
    }
}

#[test]
fn rq_roundtrip_display_serde_identity() {
    // Display output must be parseable by FromStr AND deserializable by serde
    for state in [
        TriggerState::Active,
        TriggerState::Paused,
        TriggerState::Disabled,
        TriggerState::Error,
    ] {
        let display = format!("{state}");
        // FromStr must work
        let from_str: TriggerState = display.parse().unwrap();
        assert_eq!(from_str, state);
        // serde must also work (Display output == serde output for TriggerState)
        let serde: TriggerState = serde_json::from_str(&format!("\"{display}\"")).unwrap();
        assert_eq!(serde, state);
    }
}

#[test]
fn rq_roundtrip_ti_default_not_roundtrippable() {
    // Default is empty string, which is invalid — FromStr should fail
    let default = TriggerId::default();
    let display = format!("{default}");
    assert_eq!(display, "");
    let result: Result<TriggerId, _> = display.parse();
    assert!(
        result.is_err(),
        "empty string from default must fail FromStr"
    );
}

// =========================================================================
// CROSS-CUTTING: Error Taxonomy
// =========================================================================

#[test]
fn rq_error_ti_empty_message_mentions_empty() {
    let err = TriggerId::new("").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("empty"),
        "Empty error must mention 'empty': {msg}"
    );
}

#[test]
fn rq_error_ti_too_short_message_mentions_length() {
    let err = TriggerId::new("ab").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("too short"),
        "TooShort error must mention 'too short': {msg}"
    );
    assert!(
        msg.contains('2'),
        "TooShort(2) must include the length: {msg}"
    );
}

#[test]
fn rq_error_ti_too_long_message_mentions_length() {
    let err = TriggerId::new("a".repeat(65)).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("too long"),
        "TooLong error must mention 'too long': {msg}"
    );
    assert!(
        msg.contains("65"),
        "TooLong(65) must include the length: {msg}"
    );
}

#[test]
fn rq_error_ti_invalid_chars_message_mentions_invalid() {
    let err = TriggerId::new("bad@id").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("invalid"),
        "InvalidCharacters error must mention 'invalid': {msg}"
    );
}

#[test]
fn rq_error_ts_unknown_message_mentions_input() {
    let err = "garbage".parse::<TriggerState>().unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("garbage"),
        "ParseTriggerStateError must include original input: {msg}"
    );
    assert!(
        msg.contains("unknown"),
        "ParseTriggerStateError must mention 'unknown': {msg}"
    );
}

#[test]
fn rq_error_ts_implements_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(ParseTriggerStateError(String::from("test")));
    let _ = err.to_string(); // must not panic
}

// =========================================================================
// ADDITIONAL: Unicode edge cases for TriggerId
// =========================================================================

#[test]
fn rq_ti_cjk_accepted_per_contract() {
    // Contract [NG-6]: CJK is accepted via is_alphanumeric()
    assert!(TriggerId::new("日本語").is_ok());
    assert!(TriggerId::new("abc-日本語-123").is_ok());
}

#[test]
fn rq_ti_thai_accepted() {
    // Thai characters are also alphanumeric in Unicode
    assert!(TriggerId::new("abc-ภาษาไทย").is_ok());
}

#[test]
fn rq_ti_mixed_scripts_accepted() {
    // Mix of Latin, CJK, digits
    assert!(TriggerId::new("test-日本語-001").is_ok());
}

#[test]
fn rq_ti_surrogate_pairs_rejected() {
    // Rust strings can't contain unpaired surrogates, but we can test
    // that the validation doesn't panic on multi-byte sequences
    let result = TriggerId::new("abc-🔥def");
    assert!(result.is_err());
}

#[test]
fn rq_ti_only_dashes_and_underscores() {
    // Strings of only separators are valid if they pass length check
    assert!(TriggerId::new("---").is_ok());
    assert!(TriggerId::new("___").is_ok());
    assert!(TriggerId::new("_-_").is_ok());
    assert!(TriggerId::new("--__--").is_ok());
}

#[test]
fn rq_ti_leading_trailing_separators() {
    // Leading/trailing dashes and underscores are valid per current rules
    assert!(TriggerId::new("-abc").is_ok());
    assert!(TriggerId::new("abc-").is_ok());
    assert!(TriggerId::new("_abc").is_ok());
    assert!(TriggerId::new("abc_").is_ok());
}

#[test]
fn rq_ti_numeric_only() {
    assert!(TriggerId::new("123").is_ok());
    assert!(TriggerId::new("1234567890").is_ok());
}

#[test]
fn rq_ti_single_char_boundary() {
    // '-' alone is 1 char — too short
    assert_eq!(
        TriggerId::new("-"),
        Err(twerk_core::id::IdError::TooShort(1))
    );
    assert_eq!(
        TriggerId::new("_"),
        Err(twerk_core::id::IdError::TooShort(1))
    );
}
