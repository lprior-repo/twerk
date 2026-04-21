use axum::http::StatusCode;

#[tokio::test]
async fn list_queues_returns_queues() {
    let response = crate::support::TestHarness::new()
        .await
        .get("/queues")
        .await;
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().is_array());
}

#[tokio::test]
async fn get_queue_returns_queue_info_when_exists() {
    let response = crate::support::TestHarness::with_queue("default")
        .await
        .get("/queues/default")
        .await;
    assert_eq!(
        response.json(),
        &serde_json::json!({"name": "default", "size": 1, "subscribers": 0, "unacked": 0})
    );
}

#[tokio::test]
async fn get_queue_returns_not_found_payload_when_missing() {
    let response = crate::support::TestHarness::new()
        .await
        .get("/queues/test-queue")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.json()["message"], "queue test-queue not found");
}

#[tokio::test]
async fn delete_queue_returns_ok_when_exists() {
    let harness = crate::support::TestHarness::with_queue("default").await;
    let response = harness.delete("/queues/default").await;
    let get_after_delete = harness.get("/queues/default").await;
    let delete_after_delete = harness.delete("/queues/default").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(get_after_delete.status(), StatusCode::NOT_FOUND);
    assert_eq!(delete_after_delete.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        get_after_delete.json()["message"],
        "queue default not found"
    );
    assert_eq!(
        delete_after_delete.json()["message"],
        "queue default not found"
    );
}

#[tokio::test]
async fn delete_queue_returns_not_found_payload_when_missing() {
    let response = crate::support::TestHarness::new()
        .await
        .delete("/queues/test-queue")
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.json()["message"], "queue test-queue not found");
}
