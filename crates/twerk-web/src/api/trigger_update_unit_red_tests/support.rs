use std::collections::HashMap;

use crate::api::trigger_api::{Trigger, TriggerId, TriggerUpdateRequest};
use time::OffsetDateTime;

pub fn valid_request() -> TriggerUpdateRequest {
    TriggerUpdateRequest {
        name: "trigger-name".to_string(),
        enabled: true,
        event: "event.created".to_string(),
        condition: Some("x > 1".to_string()),
        action: "notify".to_string(),
        metadata: Some(HashMap::from([("k".to_string(), "v".to_string())])),
        id: None,
        version: Some(1),
    }
}

pub fn base_trigger() -> Trigger {
    let now = OffsetDateTime::UNIX_EPOCH;
    Trigger {
        id: TriggerId::parse("trg_1").expect("valid id"),
        name: "old".to_string(),
        enabled: false,
        event: "old.event".to_string(),
        condition: None,
        action: "old_action".to_string(),
        metadata: HashMap::from([("old".to_string(), "value".to_string())]),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}
