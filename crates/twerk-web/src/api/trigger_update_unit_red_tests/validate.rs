use crate::api::trigger_api::{
    validate_trigger_update, TriggerId, TriggerUpdateError, TriggerUpdateRequest,
    ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, METADATA_KEY_MSG, NAME_REQUIRED_MSG,
    TRIGGER_FIELD_MAX_LEN,
};

use super::support::valid_request;

#[test]
fn validate_trigger_update_accepts_valid_inputs() {
    let req = valid_request();
    assert_eq!(
        validate_trigger_update("trg_1", &req),
        Ok(TriggerId::from("trg_1"))
    );

    let min_req = TriggerUpdateRequest {
        name: "n".to_string(),
        enabled: true,
        event: "e".to_string(),
        condition: None,
        action: "a".to_string(),
        metadata: None,
        id: None,
        version: Some(1),
    };
    assert_eq!(
        validate_trigger_update("xyz", &min_req),
        Ok(TriggerId::from("xyz"))
    );

    let max = "x".repeat(TRIGGER_FIELD_MAX_LEN);
    let max_req = TriggerUpdateRequest {
        name: max.clone(),
        enabled: true,
        event: max.clone(),
        condition: None,
        action: max,
        metadata: None,
        id: None,
        version: Some(1),
    };
    assert_eq!(
        validate_trigger_update("trg_1", &max_req),
        Ok(TriggerId::from("trg_1"))
    );
}

#[test]
fn validate_trigger_update_rejects_invalid_ids() {
    let req = valid_request();
    assert_eq!(
        validate_trigger_update("bad$id", &req),
        Err(TriggerUpdateError::InvalidIdFormat("bad$id".to_string()))
    );

    let overlong = "a".repeat(crate::api::trigger_api::TRIGGER_ID_MAX_LEN + 1);
    assert_eq!(
        validate_trigger_update(&overlong, &req),
        Err(TriggerUpdateError::InvalidIdFormat(overlong))
    );
}

#[test]
fn validate_trigger_update_rejects_invalid_fields_and_metadata() {
    let cases = [
        (
            {
                let mut req = valid_request();
                req.name = "  ".to_string();
                req
            },
            TriggerUpdateError::ValidationFailed(NAME_REQUIRED_MSG.to_string()),
        ),
        (
            {
                let mut req = valid_request();
                req.event = "\n\t".to_string();
                req
            },
            TriggerUpdateError::ValidationFailed(EVENT_REQUIRED_MSG.to_string()),
        ),
        (
            {
                let mut req = valid_request();
                req.action = " ".to_string();
                req
            },
            TriggerUpdateError::ValidationFailed(ACTION_REQUIRED_MSG.to_string()),
        ),
        (
            {
                let mut req = valid_request();
                req.metadata = Some(std::collections::HashMap::from([(
                    "ключ".to_string(),
                    "v".to_string(),
                )]));
                req
            },
            TriggerUpdateError::ValidationFailed(METADATA_KEY_MSG.to_string()),
        ),
    ];

    cases.into_iter().for_each(|(req, expected)| {
        assert_eq!(validate_trigger_update("trg_1", &req), Err(expected));
    });
}

#[test]
fn validate_trigger_update_rejects_mismatched_and_oversized_fields() {
    let mut mismatch = valid_request();
    mismatch.id = Some("trg_2".to_string());
    assert_eq!(
        validate_trigger_update("trg_1", &mismatch),
        Err(TriggerUpdateError::IdMismatch {
            path_id: "trg_1".to_string(),
            body_id: "trg_2".to_string(),
        })
    );

    let mut oversized = valid_request();
    oversized.name = "x".repeat(TRIGGER_FIELD_MAX_LEN + 1);
    assert_eq!(
        validate_trigger_update("trg_1", &oversized),
        Err(TriggerUpdateError::ValidationFailed(
            "name exceeds max length".to_string(),
        ))
    );
}
