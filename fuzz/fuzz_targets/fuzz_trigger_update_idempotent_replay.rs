#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;
use time::OffsetDateTime;
use twerk_web::api::trigger_api::{
    apply_trigger_update, validate_trigger_update, Trigger, TriggerId, TriggerUpdateError,
    TriggerUpdateRequest,
};

#[derive(arbitrary::Arbitrary)]
struct IdempotentReplayInput<'a> {
    path_id: &'a [u8],
    name: &'a [u8],
    event: &'a [u8],
    action: &'a [u8],
    metadata_key: &'a [u8],
    metadata_value: &'a [u8],
    timestamp_delta_secs: i64,
}

fn make_request(
    name: &str,
    event: &str,
    action: &str,
    key: &str,
    value: &str,
) -> TriggerUpdateRequest {
    let mut metadata = HashMap::new();
    if !key.is_empty() {
        metadata.insert(key.to_string(), value.to_string());
    }
    TriggerUpdateRequest {
        name: name.to_string(),
        enabled: true,
        event: event.to_string(),
        condition: None,
        action: action.to_string(),
        metadata: Some(metadata),
        id: None,
        version: Some(1),
    }
}

fuzz_target!(|input: IdempotentReplayInput| {
    let path_id_str = String::from_utf8_lossy(input.path_id);
    let name_str = String::from_utf8_lossy(input.name);
    let event_str = String::from_utf8_lossy(input.event);
    let action_str = String::from_utf8_lossy(input.action);
    let key_str = String::from_utf8_lossy(input.metadata_key);
    let value_str = String::from_utf8_lossy(input.metadata_value);

    if path_id_str.is_empty()
        || name_str.is_empty()
        || event_str.is_empty()
        || action_str.is_empty()
    {
        return;
    }

    let path_id = match TriggerId::parse(&path_id_str) {
        Ok(id) => id,
        Err(_) => return,
    };

    let req = make_request(&name_str, &event_str, &action_str, &key_str, &value_str);

    if validate_trigger_update(&path_id_str, &req).is_err() {
        return;
    }

    let now = OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(1000);

    let current = Trigger {
        id: path_id.clone(),
        name: "initial".to_string(),
        enabled: false,
        event: "initial.event".to_string(),
        condition: None,
        action: "initial_action".to_string(),
        metadata: HashMap::new(),
        created_at: now - time::Duration::seconds(100),
        updated_at: now - time::Duration::seconds(50),
    };

    let first_update = apply_trigger_update(current.clone(), req.clone(), now);
    if first_update.is_err() {
        return;
    }

    let replay_timestamp = now + time::Duration::seconds(input.timestamp_delta_secs.abs() + 1);
    let _second_update = apply_trigger_update(current, req, replay_timestamp);
});
