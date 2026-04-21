use proptest::prelude::*;

use super::super::{IdError, TriggerId};

#[test]
fn trigger_id_new_returns_ok_when_input_is_3_chars() {
    let result = TriggerId::new("abc");
    assert_eq!(result.unwrap().as_str(), "abc");
}

#[test]
fn trigger_id_new_accepts_exactly_64_chars() {
    let max_valid = "a".repeat(64);
    let id = TriggerId::new(&max_valid).unwrap();
    assert_eq!(id.as_str().len(), 64);
}

#[test]
fn trigger_id_new_accepts_dash_underscore_and_cjk() {
    assert_eq!(TriggerId::new("a_b-c").unwrap().as_str(), "a_b-c");
    assert_eq!(TriggerId::new("日本語").unwrap().as_str(), "日本語");
}

#[test]
fn trigger_id_new_rejects_invalid_lengths() {
    assert_eq!(TriggerId::new(""), Err(IdError::Empty));
    assert_eq!(TriggerId::new("ab"), Err(IdError::TooShort(2)));
    assert_eq!(TriggerId::new("a"), Err(IdError::TooShort(1)));
    assert_eq!(TriggerId::new("a".repeat(65)), Err(IdError::TooLong(65)));
    assert_eq!(TriggerId::new("a".repeat(100)), Err(IdError::TooLong(100)));
}

#[test]
fn trigger_id_new_rejects_invalid_characters() {
    assert_eq!(TriggerId::new("abc@def"), Err(IdError::InvalidCharacters));
    assert_eq!(TriggerId::new("abc def"), Err(IdError::InvalidCharacters));
    assert_eq!(
        TriggerId::new("abc-\u{1F525}def"),
        Err(IdError::InvalidCharacters)
    );
    assert_eq!(
        TriggerId::new("abc\x00def"),
        Err(IdError::InvalidCharacters)
    );
}

#[test]
fn trigger_id_preserves_input_and_whitespace_rules() {
    assert_eq!(
        TriggerId::new("my-trigger_01").unwrap().to_string(),
        "my-trigger_01"
    );
    assert_eq!(TriggerId::new(" abc"), Err(IdError::InvalidCharacters));
    assert_eq!(TriggerId::new("abc "), Err(IdError::InvalidCharacters));
    assert_eq!(
        TriggerId::new("MyTrigger_01").unwrap().as_str(),
        "MyTrigger_01"
    );
}

#[test]
fn trigger_id_accessors_and_traits_return_original_value() {
    let id = TriggerId::new("valid-id").unwrap();
    let as_ref_value: &str = id.as_ref();
    let deref_value: &str = &id;
    let borrow_value: &str = std::borrow::Borrow::borrow(&id);
    assert_eq!(id.as_str(), "valid-id");
    assert_eq!(
        format!("{}", TriggerId::new("my-trigger").unwrap()),
        "my-trigger"
    );
    assert_eq!(as_ref_value, "valid-id");
    assert_eq!(deref_value, "valid-id");
    assert_eq!(borrow_value, "valid-id");
}

#[test]
fn trigger_id_serde_roundtrip_and_rejection() {
    let id = TriggerId::new("trigger-abc").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"trigger-abc\"");
    assert_eq!(
        serde_json::from_str::<TriggerId>("\"my-trigger\"")
            .unwrap()
            .as_str(),
        "my-trigger"
    );
    assert!(serde_json::from_str::<TriggerId>("\"ab\"").is_err());
    assert!(serde_json::from_str::<TriggerId>("\"x\"").is_err());
    assert!(serde_json::from_str::<TriggerId>("\"\"").is_err());
    assert!(serde_json::from_str::<TriggerId>(&format!("\"{}\"", "a".repeat(65))).is_err());
}

#[test]
fn trigger_id_default_and_from_str() {
    assert_eq!(TriggerId::default().as_str(), "");
    assert_eq!(
        "valid-id".parse::<TriggerId>().unwrap().as_str(),
        "valid-id"
    );
    assert_eq!("x".parse::<TriggerId>(), Err(IdError::TooShort(1)));
}

#[test]
fn trigger_id_from_impls_still_bypass_validation() {
    assert_eq!(TriggerId::from(String::from("x")).as_str(), "x");
    assert_eq!(TriggerId::from("y").as_str(), "y");
}

#[test]
fn trigger_id_clone_hash_and_error_messages_work() {
    let id = TriggerId::new("clone-test").unwrap();
    assert_eq!(id.clone(), id);
    assert_eq!(id, id);

    let mut set = std::collections::HashSet::new();
    set.insert(TriggerId::new("same").unwrap());
    set.insert(TriggerId::new("same").unwrap());
    set.insert(TriggerId::new("different").unwrap());
    assert_eq!(set.len(), 2);
    assert!(set.contains(&TriggerId::new("same").unwrap()));

    let empty = TriggerId::new("").unwrap_err();
    let too_long = TriggerId::new("a".repeat(65)).unwrap_err();
    let too_short = TriggerId::new("ab").unwrap_err();
    let invalid = TriggerId::new("bad@id").unwrap_err();
    assert!(format!("{empty}").to_lowercase().contains("empty"));
    assert!(format!("{too_long}").contains("65"));
    assert!(format!("{too_short}").to_lowercase().contains("too short"));
    assert!(format!("{invalid}").to_lowercase().contains("invalid"));
}

proptest! {
    #[test]
    fn proptest_trigger_id_rejects_lengths_outside_3_to_64(len in 0usize..=70) {
        let value = "a".repeat(len);
        let result = TriggerId::new(&value);
        if len < 3 {
            prop_assert!(result.is_err());
        } else if len > 64 {
            prop_assert!(matches!(result, Err(IdError::TooLong(n)) if n == len));
        } else {
            prop_assert!(result.is_ok());
            let id = result.unwrap();
            prop_assert_eq!(id.as_str(), value);
        }
    }

    #[test]
    fn proptest_trigger_id_rejects_invalid_chars(
        base_len in 3usize..=64,
        special_char in proptest::sample::select(vec![
            '\t', '@', '#', '$', '%', '^', '&', '*', '(', ')', ' ', '=', '+',
            '[', ']', '{', '}', '|', '\\', ':', ';', '\'', '"', '<', '>', ',',
            '.', '/', '?', '`', '~',
        ])
    ) {
        let safe_part = "a".repeat(base_len.saturating_sub(1).max(1));
        let value = format!("{safe_part}{special_char}");
        if value.len() >= 3 && value.len() <= 64 {
            prop_assert!(matches!(TriggerId::new(&value), Err(IdError::InvalidCharacters)));
        }
    }

    #[test]
    fn proptest_trigger_id_serde_roundtrip_preserves_string(s in "[a-zA-Z0-9_-]{3,64}") {
        let id = TriggerId::new(&s).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let recovered: TriggerId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(recovered.as_str(), s);
    }

    #[test]
    fn proptest_trigger_id_preserves_input_without_mutation(s in "[a-zA-Z0-9_-]{3,64}") {
        let id = TriggerId::new(&s).unwrap();
        prop_assert_eq!(id.as_str(), s);
    }
}
