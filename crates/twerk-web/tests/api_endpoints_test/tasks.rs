use axum::http::StatusCode;
use serde_json::json;
use tower::ServiceExt;
use twerk_core::task::TaskLogPart;
use twerk_infrastructure::datastore::Datastore;

use super::shared::{app, body_to_json, setup_state, setup_state_with_direct_task};

#[tokio::test]
async fn get_task_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/non-existent-task")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_log_returns_404_when_task_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/non-existent-task/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_returns_task_when_exists() {
    let (state, _, task_id) = setup_state_with_direct_task().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{task_id}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert_eq!(body["id"], task_id.to_string());
    assert_eq!(body["name"], "Direct Task");
    assert_eq!(body["state"], "RUNNING");
}

#[tokio::test]
async fn get_task_log_returns_empty_when_no_logs() {
    let (state, _, task_id) = setup_state_with_direct_task().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{task_id}/log"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_task_log_returns_logs_when_exist() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    ds.create_task_log_part(&TaskLogPart {
        id: Some("log-1".to_string()),
        number: 1,
        task_id: Some(task_id.clone()),
        contents: Some("First log line".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();
    ds.create_task_log_part(&TaskLogPart {
        id: Some("log-2".to_string()),
        number: 2,
        task_id: Some(task_id.clone()),
        contents: Some("Second log line".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();

    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{task_id}/log"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let items = body_to_json(response).await["items"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["contents"], "First log line");
    assert_eq!(items[1]["contents"], "Second log line");
}

#[tokio::test]
async fn get_task_log_respects_pagination() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    for i in 1..=5 {
        ds.create_task_log_part(&TaskLogPart {
            id: Some(format!("log-{i}")),
            number: i as i64,
            task_id: Some(task_id.clone()),
            contents: Some(format!("Log line {i}")),
            ..Default::default()
        })
        .await
        .unwrap();
    }

    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{task_id}/log?page=1&size=2"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total_items"], 5);
    assert_eq!(body["total_pages"], 3);
}

#[tokio::test]
async fn get_task_log_rejects_non_numeric_page() {
    let (_, _, task_id) = setup_state_with_direct_task().await;
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{task_id}/log?page=abc"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        body_to_json(response).await,
        json!({ "message": "page must be a positive integer" })
    );
}
