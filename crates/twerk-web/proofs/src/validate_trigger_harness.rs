// Note: `validate_metadata` is private (pub(crate)-equivalent), so it cannot be
// tested from this external proof crate. These harnesses cover `validate_trigger_update`
// and `validate_trigger_create` which are the public entry points that internally
// call `validate_metadata`.

use std::collections::HashMap;

use twerk_web::api::trigger_api::{
    validate_trigger_update, TriggerUpdateError, TriggerUpdateRequest,
    NAME_REQUIRED_MSG, EVENT_REQUIRED_MSG, ACTION_REQUIRED_MSG, METADATA_KEY_MSG,
};
use twerk_web::api::trigger_api::domain::validate_trigger_create;

fn valid_request() -> TriggerUpdateRequest {
    TriggerUpdateRequest {
        name: "test-trigger".to_string(),
        enabled: true,
        event: "push".to_string(),
        condition: None,
        action: "deploy".to_string(),
        metadata: None,
        id: None,
        version: None,
    }
}

// ---------------------------------------------------------------------------
// validate_trigger_update
// ---------------------------------------------------------------------------

#[kani::proof]
fn validate_update_rejects_empty_name() {
    let mut req = valid_request();
    req.name = "   ".to_string();
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == NAME_REQUIRED_MSG),
        "Empty name must be rejected"
    );
}

#[kani::proof]
fn validate_update_rejects_empty_event() {
    let mut req = valid_request();
    req.event = "   ".to_string();
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == EVENT_REQUIRED_MSG),
        "Empty event must be rejected"
    );
}

#[kani::proof]
fn validate_update_rejects_empty_action() {
    let mut req = valid_request();
    req.action = "   ".to_string();
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == ACTION_REQUIRED_MSG),
        "Empty action must be rejected"
    );
}

#[kani::proof]
fn validate_update_rejects_metadata_empty_key() {
    let mut req = valid_request();
    let mut meta = HashMap::new();
    meta.insert("".to_string(), "value".to_string());
    req.metadata = Some(meta);
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == METADATA_KEY_MSG),
        "Empty metadata key must be rejected"
    );
}

#[kani::proof]
fn validate_update_rejects_metadata_non_ascii_key() {
    let mut req = valid_request();
    let mut meta = HashMap::new();
    meta.insert("k\u{00e9}y".to_string(), "value".to_string());
    req.metadata = Some(meta);
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == METADATA_KEY_MSG),
        "Non-ASCII metadata key must be rejected"
    );
}

#[kani::proof]
fn validate_update_accepts_valid_request() {
    let req = valid_request();
    let result = validate_trigger_update("trg_1", &req);
    assert!(result.is_ok(), "Valid request must be accepted");
}

#[kani::proof]
fn validate_update_rejects_id_mismatch() {
    let mut req = valid_request();
    req.id = Some("different_id".to_string());
    let result = validate_trigger_update("trg_1", &req);
    assert!(
        matches!(result, Err(TriggerUpdateError::IdMismatch { .. })),
        "Path/body ID mismatch must be rejected"
    );
}

// ---------------------------------------------------------------------------
// validate_trigger_create
// ---------------------------------------------------------------------------

#[kani::proof]
fn validate_create_rejects_empty_name() {
    let mut req = valid_request();
    req.name = "   ".to_string();
    let result = validate_trigger_create(&req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == NAME_REQUIRED_MSG),
        "Empty name must be rejected in create"
    );
}

#[kani::proof]
fn validate_create_accepts_valid_request() {
    let req = valid_request();
    let result = validate_trigger_create(&req);
    assert!(result.is_ok(), "Valid create request must be accepted");
}

#[kani::proof]
fn validate_create_rejects_metadata_empty_key() {
    let mut req = valid_request();
    let mut meta = HashMap::new();
    meta.insert("".to_string(), "value".to_string());
    req.metadata = Some(meta);
    let result = validate_trigger_create(&req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == METADATA_KEY_MSG),
        "Empty metadata key must be rejected in create"
    );
}

#[kani::proof]
fn validate_create_rejects_metadata_non_ascii_key() {
    let mut req = valid_request();
    let mut meta = HashMap::new();
    meta.insert("\u{00e9}".to_string(), "value".to_string());
    req.metadata = Some(meta);
    let result = validate_trigger_create(&req);
    assert!(
        matches!(result, Err(TriggerUpdateError::ValidationFailed(ref msg)) if msg == METADATA_KEY_MSG),
        "Non-ASCII metadata key must be rejected in create"
    );
}

#[kani::proof]
fn validate_create_accepts_valid_metadata_keys() {
    let mut req = valid_request();
    let mut meta = HashMap::new();
    meta.insert("key1".to_string(), "value1".to_string());
    meta.insert("another_key".to_string(), "value2".to_string());
    req.metadata = Some(meta);
    let result = validate_trigger_create(&req);
    assert!(
        result.is_ok(),
        "Normal ASCII metadata keys must be accepted"
    );
}
