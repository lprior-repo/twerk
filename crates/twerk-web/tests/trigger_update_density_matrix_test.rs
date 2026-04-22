#![allow(clippy::unwrap_used)]

use rstest::rstest;
use std::collections::HashMap;
use twerk_web::api::trigger_api::{
    apply_trigger_update, validate_trigger_update, Trigger, TriggerId, TriggerUpdateError,
    TriggerUpdateRequest, ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, NAME_REQUIRED_MSG,
};

fn valid_request() -> TriggerUpdateRequest {
    TriggerUpdateRequest {
        name: "name".to_string(),
        enabled: true,
        event: "event".to_string(),
        condition: Some("condition".to_string()),
        action: "action".to_string(),
        metadata: Some(HashMap::from([("key".to_string(), "value".to_string())])),
        id: None,
        version: None,
    }
}

fn current_trigger() -> Trigger {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    Trigger {
        id: TriggerId::parse("seed-1").unwrap(),
        name: "old-name".to_string(),
        enabled: false,
        event: "old-event".to_string(),
        condition: None,
        action: "old-action".to_string(),
        metadata: HashMap::new(),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

#[rstest]
fn validate_trigger_update_returns_ok_trigger_id_for_cartesian_valid_ids(
    #[values("a", "b", "c", "d", "e", "f", "g", "h", "i", "j")] p0: &str,
    #[values("0", "1", "2", "3", "4", "5", "6", "7", "8", "9")] p1: &str,
    #[values("x", "y", "z", "m", "n", "p", "q", "r", "s", "t")] p2: &str,
) {
    let path_id = format!("{}{}{}", p0, p1, p2);
    let req = valid_request();
    let result = validate_trigger_update(&path_id, &req);
    assert_eq!(
        result,
        Ok(TriggerId::parse(path_id.as_str()).expect("valid id"))
    );
}

#[rstest]
fn apply_trigger_update_replaces_mutable_fields_for_cartesian_valid_inputs(
    #[values("aa", "bb", "cc", "dd", "ee")] name_prefix: &str,
    #[values("11", "22", "33", "44", "55")] event_part: &str,
    #[values("xx", "yy", "zz", "ww", "vv", "uu", "tt", "ss")] action_part: &str,
) {
    let mut req = valid_request();
    req.name = format!("{}-name", name_prefix);
    req.event = format!("event-{}", event_part);
    req.action = format!("action-{}", action_part);
    let result = apply_trigger_update(
        current_trigger(),
        req.clone(),
        time::OffsetDateTime::UNIX_EPOCH,
    );
    let updated = result.expect("valid projection update should succeed");
    assert_eq!(updated.name, req.name);
    assert_eq!(updated.event, req.event);
    assert_eq!(updated.action, req.action);
}

#[rstest]
fn validate_trigger_update_returns_exact_name_validation_error_for_trimmed_blank_names(
    #[values("", " ", "   ", "\n", "\t", "\n\t", "\t\n", " \t ")] blank: &str,
) {
    let mut req = valid_request();
    req.name = blank.to_string();
    let result = validate_trigger_update("valid-id", &req);
    assert_eq!(
        result,
        Err(TriggerUpdateError::ValidationFailed(
            NAME_REQUIRED_MSG.to_string()
        ))
    );
}

#[rstest]
fn validate_trigger_update_returns_exact_event_validation_error_for_trimmed_blank_events(
    #[values("", " ", "   ", "\n", "\t", "\n\t", "\t\n", " \t ")] blank: &str,
) {
    let mut req = valid_request();
    req.event = blank.to_string();
    let result = validate_trigger_update("valid-id", &req);
    assert_eq!(
        result,
        Err(TriggerUpdateError::ValidationFailed(
            EVENT_REQUIRED_MSG.to_string()
        ))
    );
}

#[rstest]
fn validate_trigger_update_returns_exact_action_validation_error_for_trimmed_blank_actions(
    #[values("", " ", "   ", "\n", "\t", "\n\t", "\t\n", " \t ")] blank: &str,
) {
    let mut req = valid_request();
    req.action = blank.to_string();
    let result = validate_trigger_update("valid-id", &req);
    assert_eq!(
        result,
        Err(TriggerUpdateError::ValidationFailed(
            ACTION_REQUIRED_MSG.to_string()
        ))
    );
}

#[rstest]
fn apply_trigger_update_rejects_backward_timestamp_for_multiple_nanosecond_offsets(
    #[values(1_i64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)] nanos_back: i64,
) {
    let req = valid_request();
    let mut current = current_trigger();
    current.updated_at = time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(100);
    let now = current.updated_at - time::Duration::nanoseconds(nanos_back);
    let result = apply_trigger_update(current, req, now);
    assert_eq!(
        result,
        Err(TriggerUpdateError::ValidationFailed(
            "updated_at cannot move backwards".to_string()
        ))
    );
}
