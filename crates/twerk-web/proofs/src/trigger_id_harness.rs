use twerk_web::api::trigger_api::{TriggerId, TriggerUpdateError};

#[kani::proof]
fn trigger_id_rejects_too_short() {
    // 2 characters is below TRIGGER_ID_MIN_LEN (3)
    let result = TriggerId::parse("ab");
    assert!(
        matches!(result, Err(TriggerUpdateError::InvalidIdFormat(_))),
        "IDs shorter than 3 chars must be rejected"
    );
}

#[kani::proof]
fn trigger_id_rejects_too_long() {
    // 65 characters exceeds TRIGGER_ID_MAX_LEN (64)
    let long_id = "a".repeat(65);
    let result = TriggerId::parse(&long_id);
    assert!(
        matches!(result, Err(TriggerUpdateError::InvalidIdFormat(_))),
        "IDs longer than 64 chars must be rejected"
    );
}

#[kani::proof]
fn trigger_id_accepts_boundary_3() {
    // Exactly 3 valid characters is the minimum accepted length
    let result = TriggerId::parse("abc");
    assert!(
        result.is_ok(),
        "3-character valid ID must be accepted"
    );
    assert_eq!(result.unwrap().as_str(), "abc");
}

#[kani::proof]
fn trigger_id_accepts_boundary_64() {
    // Exactly 64 valid characters is the maximum accepted length
    let id_64 = "a".repeat(64);
    let result = TriggerId::parse(&id_64);
    assert!(
        result.is_ok(),
        "64-character valid ID must be accepted"
    );
    assert_eq!(result.unwrap().as_str().len(), 64);
}

#[kani::proof]
fn trigger_id_rejects_invalid_chars() {
    // Characters like @, #, and space are not alphanumeric/underscore/hyphen
    let bad_ids = ["bad@id", "bad#id", "bad id"];
    for id in bad_ids {
        let result = TriggerId::parse(id);
        assert!(
            matches!(result, Err(TriggerUpdateError::InvalidIdFormat(_))),
            "ID '{id}' contains invalid characters and must be rejected"
        );
    }
}

#[kani::proof]
fn trigger_id_serde_roundtrip() {
    let original = TriggerId::parse("my-trigger_123").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: TriggerId = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}
