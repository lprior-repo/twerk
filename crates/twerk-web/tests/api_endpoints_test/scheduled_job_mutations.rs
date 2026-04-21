use axum::http::{header, StatusCode};
use tower::ServiceExt;

use super::shared::{app, body_to_json, scheduled_job_input, setup_state};

async fn create_job_id(app: &axum::Router, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input(name)).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    body_to_json(response).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn pause_scheduled_job_returns_ok_when_active() {
    let app = app(setup_state().await);
    let job_id = create_job_id(&app, "test-scheduled-job-pause").await;
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{job_id}/pause"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_to_json(response).await["status"], "OK");
}

#[tokio::test]
async fn pause_scheduled_job_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/scheduled-jobs/non-existent-id/pause")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn resume_scheduled_job_returns_ok_when_paused() {
    let app = app(setup_state().await);
    let job_id = create_job_id(&app, "test-scheduled-job-resume").await;
    let _ = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{job_id}/pause"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{job_id}/resume"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_to_json(response).await["status"], "OK");
}

#[tokio::test]
async fn resume_scheduled_job_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/scheduled-jobs/non-existent-id/resume")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_scheduled_job_returns_ok_when_exists() {
    let app = app(setup_state().await);
    let job_id = create_job_id(&app, "test-scheduled-job-delete").await;
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/scheduled-jobs/{job_id}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_to_json(response).await["status"], "OK");
}

#[tokio::test]
async fn delete_scheduled_job_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/scheduled-jobs/non-existent-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
