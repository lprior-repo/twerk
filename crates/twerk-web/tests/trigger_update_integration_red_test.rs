#![allow(clippy::panic)]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use time::OffsetDateTime;
use tower::ServiceExt;
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::trigger_api::{InMemoryTriggerDatastore, Trigger, TriggerAppState, TriggerId};
use twerk_web::api::{create_router, AppState, Config};

fn trigger(id: &str) -> Trigger {
    let now = OffsetDateTime::UNIX_EPOCH;
    Trigger {
        id: TriggerId::parse(id).expect("valid id"),
        name: "before".to_string(),
        enabled: false,
        event: "before.event".to_string(),
        condition: Some("x == 1".to_string()),
        action: "before_action".to_string(),
        metadata: std::collections::HashMap::from([("k".to_string(), "v".to_string())]),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

fn body_ok(id: &str) -> Value {
    json!({
        "id": id,
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "condition": "amount > 10",
        "action": "send_email",
        "metadata": {"env":"prod"},
        "version": 1
    })
}

fn build_state(trigger_ds: Arc<InMemoryTriggerDatastore>) -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState {
        trigger_state: TriggerAppState { trigger_ds },
        ..AppState::new(broker, ds, Config::default())
    }
}

async fn send_put(
    app: axum::Router,
    path: &str,
    content_type: &str,
    payload: Body,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header(header::CONTENT_TYPE, content_type)
                .body(payload)
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).expect("json body");
    (status, json)
}

#[tokio::test]
async fn update_trigger_handler_returns_400_invalid_id_format_when_path_id_is_unparseable() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/bad$id",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("bad$id")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error":"InvalidIdFormat","message":"bad$id"}));
}

#[tokio::test]
async fn update_trigger_handler_returns_400_unsupported_content_type_when_content_type_is_text_plain(
) {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "text/plain",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error":"UnsupportedContentType","message":"text/plain"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_400_malformed_json_when_body_is_truncated_json() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from("{\"name\":"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error":"MalformedJson","message":"malformed JSON body"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_400_validation_failed_when_body_is_empty_object() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from("{}"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error":"ValidationFailed","message":"name must be non-empty after trim"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_400_id_mismatch_when_body_id_differs_from_path_id() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_path")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_path",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_body")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error":"IdMismatch","message":"id mismatch","path_id":"trg_path","body_id":"trg_body"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_404_trigger_not_found_when_trigger_missing() {
    let app = create_router(build_state(Arc::new(InMemoryTriggerDatastore::new())));
    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_missing",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_missing")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({"error":"TriggerNotFound","message":"trg_missing"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_409_version_conflict_when_stale_version_supplied() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));
    let mut body_conflict = body_ok("trg_abc");
    body_conflict["version"] = json!(0);

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_conflict).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({"error":"VersionConflict","message":"stale version supplied"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_500_persistence_when_datastore_update_fails() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let previous = trigger_ds.set_fail_next_update(true);
    assert!(
        !previous,
        "fail_next_update should be false before enabling"
    );
    let app = create_router(build_state(trigger_ds));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        json!({"error":"Persistence","message":"internal persistence failure"})
    );
}

#[tokio::test]
async fn update_trigger_handler_returns_200_and_trigger_view_equal_to_committed_trigger() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds.clone()));

    let (status, body) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let persisted = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("persisted");
    assert_eq!(body["id"], json!(persisted.id.as_str()));
    assert_eq!(body["name"], json!(persisted.name));
    assert_eq!(body["enabled"], json!(persisted.enabled));
    assert_eq!(body["event"], json!(persisted.event));
    assert_eq!(body["condition"], json!(persisted.condition));
    assert_eq!(body["action"], json!(persisted.action));
    assert_eq!(body["metadata"], json!(persisted.metadata));
}

#[tokio::test]
async fn update_trigger_handler_keeps_same_mutable_state_when_same_request_applied_twice() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app_one = create_router(build_state(trigger_ds.clone()));
    let app_two = create_router(build_state(trigger_ds.clone()));

    let (first_status, first_body) = send_put(
        app_one,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;
    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first_body["id"], json!("trg_abc"));
    let first_version = first_body["version"].as_u64().expect("version as u64");
    let after_first = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("trigger");

    let mut second_request = body_ok("trg_abc");
    second_request["version"] = json!(first_version);
    let (second_status, second_body) = send_put(
        app_two,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&second_request).expect("serialize")),
    )
    .await;
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(second_body["id"], json!("trg_abc"));
    let after_second = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("trigger");

    assert_eq!(after_first.name, after_second.name);
    assert_eq!(after_first.enabled, after_second.enabled);
    assert_eq!(after_first.event, after_second.event);
    assert_eq!(after_first.condition, after_second.condition);
    assert_eq!(after_first.action, after_second.action);
    assert_eq!(after_first.metadata, after_second.metadata);
}

#[tokio::test]
async fn update_trigger_handler_preserves_preupdate_state_when_modify_closure_returns_error() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let before = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("before");
    let app = create_router(build_state(trigger_ds.clone()));
    let bad = json!({
        "id": "trg_abc",
        "name": "ok",
        "enabled": true,
        "event": "ok",
        "condition": null,
        "action": " ",
        "metadata": {"env":"prod"},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&bad).expect("serialize")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let after = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("after");
    assert_eq!(before, after);
}

#[tokio::test]
async fn update_trigger_handler_accepts_min_path_id_length_when_id_length_equals_min() {
    let id = "aaa";
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger(id)).unwrap();
    let app = create_router(build_state(trigger_ds));
    let (status, _) = send_put(
        app,
        "/api/v1/triggers/aaa",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok(id)).expect("serialize")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn update_trigger_handler_accepts_max_path_id_length_when_id_length_equals_max() {
    let id = "a".repeat(twerk_web::api::trigger_api::TRIGGER_ID_MAX_LEN);
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger(&id)).unwrap();
    let app = create_router(build_state(trigger_ds));
    let path = format!("/api/v1/triggers/{id}");
    let (status, _) = send_put(
        app,
        &path,
        "application/json",
        Body::from(serde_json::to_vec(&body_ok(&id)).expect("serialize")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn update_trigger_handler_returns_400_invalid_id_format_when_path_id_length_exceeds_max_by_one(
) {
    let id = "a".repeat(twerk_web::api::trigger_api::TRIGGER_ID_MAX_LEN + 1);
    let app = create_router(build_state(Arc::new(InMemoryTriggerDatastore::new())));
    let path = format!("/api/v1/triggers/{id}");
    let (status, body) = send_put(
        app,
        &path,
        "application/json",
        Body::from(serde_json::to_vec(&body_ok(&id)).expect("serialize")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error":"InvalidIdFormat","message":id}));
}
