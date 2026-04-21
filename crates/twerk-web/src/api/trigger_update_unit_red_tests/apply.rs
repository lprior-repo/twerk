use crate::api::trigger_api::{
    apply_trigger_update, TriggerUpdateError, TriggerUpdateRequest, ACTION_REQUIRED_MSG,
    EVENT_REQUIRED_MSG, NAME_REQUIRED_MSG, TRIGGER_FIELD_MAX_LEN,
};

use super::support::{base_trigger, valid_request};

#[test]
fn apply_trigger_update_projects_request_fields() {
    let req = valid_request();
    let current = base_trigger();
    let now = current.updated_at + time::Duration::seconds(1);
    let result = apply_trigger_update(current, req.clone(), now).expect("valid apply");
    assert_eq!(result.name, req.name);
    assert_eq!(result.enabled, req.enabled);
    assert_eq!(result.event, req.event);
    assert_eq!(result.condition, req.condition);
    assert_eq!(result.action, req.action);
    assert_eq!(result.metadata, req.metadata.unwrap_or_default());
}

#[test]
fn apply_trigger_update_preserves_identity_fields() {
    let req = valid_request();
    let current = base_trigger();
    let id = current.id.clone();
    let created_at = current.created_at;
    let now = current.updated_at + time::Duration::seconds(1);
    let result = apply_trigger_update(current, req, now).expect("valid apply");
    assert_eq!(result.id, id);
    assert_eq!(result.created_at, created_at);
}

#[test]
fn apply_trigger_update_sets_updated_at_correctly() {
    let current = base_trigger();
    let same_time = apply_trigger_update(current.clone(), valid_request(), current.updated_at)
        .expect("same time valid");
    assert_eq!(same_time.updated_at, current.updated_at);

    let later = current.updated_at + time::Duration::seconds(1);
    let later_result =
        apply_trigger_update(current.clone(), valid_request(), later).expect("later valid");
    assert_eq!(later_result.updated_at, later);

    let backwards = current.updated_at - time::Duration::nanoseconds(1);
    assert_eq!(
        apply_trigger_update(current, valid_request(), backwards),
        Err(TriggerUpdateError::ValidationFailed(
            crate::api::trigger_api::UPDATED_AT_BACKWARDS_MSG.to_string(),
        ))
    );
}

#[test]
fn apply_trigger_update_rejects_blank_required_fields() {
    let cases = [
        (
            {
                let mut req = valid_request();
                req.name = "   ".to_string();
                req
            },
            TriggerUpdateError::ValidationFailed(NAME_REQUIRED_MSG.to_string()),
        ),
        (
            {
                let mut req = valid_request();
                req.event = "\t".to_string();
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
    ];

    for (req, expected) in cases {
        assert_eq!(
            apply_trigger_update(base_trigger(), req, base_trigger().updated_at),
            Err(expected)
        );
    }
}

#[test]
fn apply_trigger_update_handles_field_length_boundaries() {
    let max = "x".repeat(TRIGGER_FIELD_MAX_LEN);
    let req = TriggerUpdateRequest {
        name: max.clone(),
        enabled: true,
        event: max.clone(),
        condition: None,
        action: max,
        metadata: None,
        id: None,
        version: Some(1),
    };
    let current = base_trigger();
    let result = apply_trigger_update(current.clone(), req, current.updated_at);
    assert_eq!(
        result.map(|trigger| trigger.name),
        Ok("x".repeat(TRIGGER_FIELD_MAX_LEN))
    );

    let mut too_long = valid_request();
    too_long.event = "x".repeat(TRIGGER_FIELD_MAX_LEN + 1);
    assert_eq!(
        apply_trigger_update(current, too_long, base_trigger().updated_at),
        Err(TriggerUpdateError::ValidationFailed(
            "event exceeds max length".to_string(),
        ))
    );
}
