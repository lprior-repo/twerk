use proptest::prelude::*;
use std::collections::HashMap;
use twerk_web::api::trigger_api::{
    apply_trigger_update, validate_trigger_update, Trigger, TriggerId, TriggerUpdateError,
    TriggerUpdateRequest, ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, METADATA_KEY_MSG,
    NAME_REQUIRED_MSG, UPDATED_AT_BACKWARDS_MSG,
};

fn base_trigger() -> Trigger {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    Trigger {
        id: TriggerId::parse("trg_1").expect("valid id"),
        name: "old".to_string(),
        enabled: false,
        event: "old.event".to_string(),
        condition: None,
        action: "old_action".to_string(),
        metadata: HashMap::new(),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

fn request(name: String, event: String, action: String) -> TriggerUpdateRequest {
    TriggerUpdateRequest {
        name,
        enabled: true,
        event,
        condition: Some("cond".to_string()),
        action,
        metadata: Some(HashMap::from([("k".to_string(), "v".to_string())])),
        id: None,
        version: Some(1),
    }
}

proptest! {
    #[test]
    fn validate_trigger_update_valid_domain_success_holds_for_generated_inputs(
        path_id in "[A-Za-z0-9_-]{1,16}",
        name in "[A-Za-z0-9_-]{1,32}",
        event in "[A-Za-z0-9_.-]{1,32}",
        action in "[A-Za-z0-9_.-]{1,32}"
    ) {
        let req = request(name, event, action);
        prop_assert_eq!(validate_trigger_update(&path_id, &req), Ok(TriggerId::from(path_id.as_str())));
    }

    #[test]
    fn validate_trigger_update_id_mismatch_always_fails_deterministically(
        path_id in "[A-Za-z0-9_-]{1,16}",
        body_id in "[A-Za-z0-9_-]{1,16}"
    ) {
        prop_assume!(path_id != body_id);
        let mut req = request("n".to_string(), "e".to_string(), "a".to_string());
        req.id = Some(body_id.clone());
        prop_assert_eq!(
            validate_trigger_update(&path_id, &req),
            Err(TriggerUpdateError::IdMismatch { path_id, body_id })
        );
    }

    #[test]
    fn validate_trigger_update_blank_after_trim_rejection_is_field_specific(
        path_id in "[A-Za-z0-9_-]{1,16}",
        blank in "[ \t\n]{1,4}"
    ) {
        let req = request(blank, "ok".to_string(), "ok".to_string());
        prop_assert_eq!(
            validate_trigger_update(&path_id, &req),
            Err(TriggerUpdateError::ValidationFailed(NAME_REQUIRED_MSG.to_string()))
        );
    }

    #[test]
    fn validate_trigger_update_metadata_key_safety_rejects_invalid_keys(
        path_id in "[A-Za-z0-9_-]{1,16}"
    ) {
        let mut req = request("ok".to_string(), "ok".to_string(), "ok".to_string());
        req.metadata = Some(HashMap::from([("ключ".to_string(), "v".to_string())]));
        prop_assert_eq!(
            validate_trigger_update(&path_id, &req),
            Err(TriggerUpdateError::ValidationFailed(METADATA_KEY_MSG.to_string()))
        );
    }

    #[test]
    fn validate_trigger_update_boundary_stability_accepts_max_length_values(
        path_id in "[A-Za-z0-9_-]{1,16}",
        len in 1usize..65usize
    ) {
        let s = "x".repeat(len);
        let req = request(s.clone(), s.clone(), s);
        prop_assert_eq!(
            validate_trigger_update(&path_id, &req),
            Ok(TriggerId::from(path_id.as_str()))
        );
    }

    #[test]
    fn validate_trigger_update_boundary_stability_rejects_max_plus_one_values(
        path_id in "[A-Za-z0-9_-]{1,16}",
        len in 65usize..66usize
    ) {
        let s = "x".repeat(len);
        let req = request(s.clone(), s.clone(), s);
        prop_assert_eq!(
            validate_trigger_update(&path_id, &req),
            Err(TriggerUpdateError::ValidationFailed("name exceeds max length".to_string()))
        );
    }

    #[test]
    fn apply_trigger_update_immutable_preservation_holds_for_valid_inputs(
        name in "[A-Za-z0-9_-]{1,32}",
        event in "[A-Za-z0-9_.-]{1,32}",
        action in "[A-Za-z0-9_.-]{1,32}",
        offset in 0i64..1000i64
    ) {
        let current = base_trigger();
        let req = request(name, event, action);
        let now = current.updated_at + time::Duration::seconds(offset);
        let updated = apply_trigger_update(current.clone(), req, now).expect("valid");
        prop_assert_eq!(updated.id, current.id);
        prop_assert_eq!(updated.created_at, current.created_at);
    }

    #[test]
    fn apply_trigger_update_projection_correctness_matches_normalized_request(
        name in "[A-Za-z0-9_ -]{1,32}",
        event in "[A-Za-z0-9_. -]{1,32}",
        action in "[A-Za-z0-9_. -]{1,32}",
        enabled in any::<bool>()
    ) {
        prop_assume!(!name.trim().is_empty());
        prop_assume!(!event.trim().is_empty());
        prop_assume!(!action.trim().is_empty());
        let current = base_trigger();
        let mut req = request(name.clone(), event.clone(), action.clone());
        req.enabled = enabled;
        let updated = apply_trigger_update(current, req.clone(), time::OffsetDateTime::UNIX_EPOCH).expect("valid");
        prop_assert_eq!(updated.name, name.trim());
        prop_assert_eq!(updated.event, event.trim());
        prop_assert_eq!(updated.action, action.trim());
        prop_assert_eq!(updated.enabled, enabled);
    }

    #[test]
    fn apply_trigger_update_timestamp_equality_is_accepted(
        _tick in 0i64..1000i64
    ) {
        let current = base_trigger();
        let req = request("n".to_string(), "e".to_string(), "a".to_string());
        let result = apply_trigger_update(current.clone(), req, current.updated_at);
        prop_assert_eq!(result.map(|trigger| trigger.updated_at), Ok(current.updated_at));
    }

    #[test]
    fn apply_trigger_update_timestamp_anti_invariant_rejects_backward_time(
        now in 0i64..1000i64,
        delta in 1i64..100i64
    ) {
        let mut current = base_trigger();
        current.updated_at = time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(now);
        let req = request("n".to_string(), "e".to_string(), "a".to_string());
        let result = apply_trigger_update(
            current,
            req,
            time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(now - delta),
        );
        prop_assert_eq!(
            result,
            Err(TriggerUpdateError::ValidationFailed(UPDATED_AT_BACKWARDS_MSG.to_string()))
        );
    }

    #[test]
    fn apply_trigger_update_length_boundary_invariant_accepts_max_values(
        field_len in 1usize..65usize
    ) {
        let s = "x".repeat(field_len);
        let req = request(s.clone(), s.clone(), s);
        let current = base_trigger();
        let result = apply_trigger_update(current, req, time::OffsetDateTime::UNIX_EPOCH);
        prop_assert_eq!(result.map(|trigger| trigger.name), Ok("x".repeat(field_len)));
    }

    #[test]
    fn apply_trigger_update_length_boundary_invariant_rejects_max_plus_one_values(
        field_len in 65usize..66usize
    ) {
        let s = "x".repeat(field_len);
        let req = request(s.clone(), s.clone(), s);
        let current = base_trigger();
        let result = apply_trigger_update(current, req, time::OffsetDateTime::UNIX_EPOCH);
        prop_assert_eq!(
            result,
            Err(TriggerUpdateError::ValidationFailed("name exceeds max length".to_string()))
        );
    }
}

#[test]
fn apply_trigger_update_returns_exact_event_validation_error_when_event_blank_after_trim() {
    let current = base_trigger();
    let req = request("ok".to_string(), "\t".to_string(), "ok".to_string());
    assert_eq!(
        apply_trigger_update(current, req, time::OffsetDateTime::UNIX_EPOCH),
        Err(TriggerUpdateError::ValidationFailed(
            EVENT_REQUIRED_MSG.to_string()
        ))
    );
}

#[test]
fn apply_trigger_update_returns_exact_action_validation_error_when_action_blank_after_trim() {
    let current = base_trigger();
    let req = request("ok".to_string(), "ok".to_string(), " ".to_string());
    assert_eq!(
        apply_trigger_update(current, req, time::OffsetDateTime::UNIX_EPOCH),
        Err(TriggerUpdateError::ValidationFailed(
            ACTION_REQUIRED_MSG.to_string()
        ))
    );
}
