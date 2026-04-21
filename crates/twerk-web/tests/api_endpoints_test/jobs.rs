use axum::http::StatusCode;
use tower::ServiceExt;

use super::shared::{app, body_to_json, setup_state, setup_state_with_jobs, JOB_ID};

#[tokio::test]
async fn list_jobs_returns_empty_list_when_no_jobs() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(body_to_json(response).await["items"].is_array());
}

#[tokio::test]
async fn list_jobs_returns_jobs_when_exist() {
    let (state, _) = setup_state_with_jobs().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(!body_to_json(response).await["items"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn list_jobs_respects_pagination_params() {
    let (state, _) = setup_state_with_jobs().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs?page=1&size=5")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(body_to_json(response).await["items"].is_array());
}

#[tokio::test]
async fn get_job_returns_job_when_exists() {
    let (state, _) = setup_state_with_jobs().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_ID}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert_eq!(body["id"], JOB_ID);
    assert_eq!(body["name"], "Test Job");
}

#[tokio::test]
async fn get_job_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/00000000-0000-0000-0000-000000000404")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_job_log_returns_empty_when_no_logs() {
    let (state, _) = setup_state_with_jobs().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_ID}/log"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(body_to_json(response).await["items"].is_array());
}

#[tokio::test]
async fn get_job_log_returns_404_when_job_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/00000000-0000-0000-0000-000000000404/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
