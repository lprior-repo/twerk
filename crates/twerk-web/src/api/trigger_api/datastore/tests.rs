use std::collections::HashMap;

use time::OffsetDateTime;

use super::{InMemoryTriggerDatastore, TriggerUpdateError};
use crate::api::trigger_api::{Trigger, TriggerId};

fn trigger_id(id: &str) -> TriggerId {
    TriggerId::parse(id).expect("valid trigger id")
}

fn make_trigger(id: &str, name: &str) -> Trigger {
    Trigger {
        id: trigger_id(id),
        name: name.to_string(),
        enabled: true,
        event: "test-event".to_string(),
        condition: None,
        action: "test-action".to_string(),
        metadata: HashMap::new(),
        version: 1,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[test]
fn get_and_upsert_behave_as_expected() {
    let ds = InMemoryTriggerDatastore::new();
    let trigger = make_trigger("test-id", "Test Trigger");
    ds.upsert(trigger.clone()).expect("upsert succeeds");
    assert_eq!(
        ds.get_trigger_by_id(&trigger.id)
            .expect("trigger exists")
            .name,
        "Test Trigger"
    );

    let missing = ds.get_trigger_by_id(&trigger_id("nonexistent-id"));
    assert!(matches!(
        missing,
        Err(TriggerUpdateError::TriggerNotFound(_))
    ));

    ds.upsert(make_trigger("existing-trigger", "Original Name"))
        .expect("first upsert succeeds");
    ds.upsert(make_trigger("existing-trigger", "Updated Name"))
        .expect("second upsert succeeds");
    assert_eq!(
        ds.get_trigger_by_id(&trigger_id("existing-trigger"))
            .expect("existing trigger present")
            .name,
        "Updated Name"
    );
}

#[test]
fn update_trigger_covers_success_and_failures() {
    let ds = InMemoryTriggerDatastore::new();
    ds.upsert(make_trigger("update-test", "Before Update"))
        .expect("seed trigger");

    let updated = ds.update_trigger(
        &trigger_id("update-test"),
        Box::new(|current| {
            Ok(Trigger {
                name: "After Update".to_string(),
                ..current
            })
        }),
    );
    assert_eq!(updated.expect("updated trigger").name, "After Update");

    let missing = ds.update_trigger(&trigger_id("nonexistent"), Box::new(Ok));
    assert!(matches!(
        missing,
        Err(TriggerUpdateError::TriggerNotFound(_))
    ));

    ds.upsert(make_trigger("fail-test", "Fail Test"))
        .expect("seed fail test");
    let _ = ds.set_fail_next_update(true);
    let persistence = ds.update_trigger(&trigger_id("fail-test"), Box::new(Ok));
    assert!(matches!(
        persistence,
        Err(TriggerUpdateError::Persistence(_))
    ));

    ds.upsert(make_trigger("error-test", "Error Test"))
        .expect("seed error test");
    let closure_error = ds.update_trigger(
        &trigger_id("error-test"),
        Box::new(|_current| {
            Err(TriggerUpdateError::ValidationFailed(
                "intentional error".to_string(),
            ))
        }),
    );
    assert!(matches!(
        closure_error,
        Err(TriggerUpdateError::ValidationFailed(_))
    ));
}

#[test]
fn trigger_store_state_flags_and_multiple_records_work() {
    let ds = InMemoryTriggerDatastore::new();
    assert!(!ds.set_fail_next_update(false));
    assert!(!ds.set_fail_next_update(true));
    assert!(ds.set_fail_next_update(false));

    ds.upsert(make_trigger("id-1", "Trigger One"))
        .expect("insert trigger one");
    ds.upsert(make_trigger("id-2", "Trigger Two"))
        .expect("insert trigger two");

    assert_eq!(
        ds.get_trigger_by_id(&trigger_id("id-1"))
            .expect("trigger one")
            .name,
        "Trigger One"
    );
    assert_eq!(
        ds.get_trigger_by_id(&trigger_id("id-2"))
            .expect("trigger two")
            .name,
        "Trigger Two"
    );
}
