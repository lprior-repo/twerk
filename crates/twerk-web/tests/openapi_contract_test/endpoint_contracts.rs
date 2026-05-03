#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::{json, Value};

use super::super::support::{
    assert_empty_body, assert_health_up, assert_job_summary, assert_json_message,
    assert_queue_state, TestHarness,
};

#[tokio::test]
async fn critical_endpoint_contracts_use_exact_statuses_and_json_envelopes() {
    let harness = TestHarness::with_queue("default").await;

    let health_response = harness
        .call(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_health_up(&health_response);

    let queue_response = harness
        .call(
            Request::builder()
                .uri("/queues/default")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_queue_state(&queue_response, "default", 1);

    let queue_delete_response = harness
        .call(
            Request::builder()
                .method(Method::DELETE)
                .uri("/queues/default")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_empty_body(&queue_delete_response, StatusCode::OK);

    let deleted_queue_response = harness
        .call(
            Request::builder()
                .uri("/queues/default")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_json_message(
        &deleted_queue_response,
        StatusCode::NOT_FOUND,
        "queue default not found",
    );

    let deleted_queue_again_response = harness
        .call(
            Request::builder()
                .method(Method::DELETE)
                .uri("/queues/default")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_json_message(
        &deleted_queue_again_response,
        StatusCode::NOT_FOUND,
        "queue default not found",
    );

    let missing_queue_response = harness
        .call(
            Request::builder()
                .uri("/queues/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_json_message(
        &missing_queue_response,
        StatusCode::NOT_FOUND,
        "queue missing not found",
    );

    let create_job_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "contract-job",
                        "tasks": [{ "name": "task-1", "image": "alpine", "run": "echo hi" }]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await;
    assert_job_summary(&create_job_response, "contract-job", "PENDING");

    let create_user_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "username": "ab", "password": "short" })).unwrap(),
                ))
                .unwrap(),
        )
        .await;
    assert_json_message(
        &create_user_response,
        StatusCode::BAD_REQUEST,
        "invalid username: username must be 3-64 characters",
    );

    let create_trigger_response = harness
        .call(
            Request::builder()
                .method(Method::POST)
                .uri("/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "contract-trigger",
                        "enabled": true,
                        "event": "job.completed",
                        "action": "notify"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await;
    create_trigger_response
        .assert_status(StatusCode::CREATED)
        .assert_json_content_type();
    assert_eq!(create_trigger_response.json()["name"], "contract-trigger");
    assert_eq!(create_trigger_response.json()["enabled"], true);
    assert_eq!(create_trigger_response.json()["event"], "job.completed");
    assert_eq!(create_trigger_response.json()["action"], "notify");
    assert_eq!(create_trigger_response.json()["metadata"], json!({}));
    assert_eq!(create_trigger_response.json()["version"], 1);
    assert!(create_trigger_response.json()["id"].as_str().is_some());
    assert_ne!(create_trigger_response.json()["created_at"], Value::Null);
    assert_ne!(create_trigger_response.json()["updated_at"], Value::Null);

    let missing_trigger_response = harness
        .call(
            Request::builder()
                .uri("/triggers/bad$id")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    missing_trigger_response
        .assert_status(StatusCode::BAD_REQUEST)
        .assert_json_content_type();
    assert_eq!(
        missing_trigger_response.json(),
        &json!({ "error": "InvalidIdFormat", "message": "bad$id" })
    );
}
