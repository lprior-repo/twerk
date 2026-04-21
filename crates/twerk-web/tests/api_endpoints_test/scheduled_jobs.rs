use axum::http::{header, StatusCode};
use tower::ServiceExt;

use super::shared::{app, body_to_json, scheduled_job_input, setup_state};

#[tokio::test]
async fn create_scheduled_job_returns_ok_with_valid_json() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input("test-scheduled-job")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-scheduled-job");
}

#[tokio::test]
async fn create_scheduled_job_returns_400_with_invalid_cron() {
    let input = serde_json::json!({
        "name": "test-scheduled-job",
        "cron": "invalid-cron-expression",
        "tasks": [{"name": "task-1", "image": "alpine", "run": "echo hello"}]
    });
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&input).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_without_cron() {
    let input = serde_json::json!({
        "name": "test-scheduled-job",
        "tasks": [{"name": "task-1", "image": "alpine", "run": "echo hello"}]
    });
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&input).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_without_tasks() {
    let input = serde_json::json!({"name": "test-scheduled-job", "cron": "0 0 * * * *"});
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&input).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_scheduled_jobs_returns_empty_list_when_no_jobs() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(body_to_json(response).await["items"].is_array());
}

#[tokio::test]
async fn list_scheduled_jobs_returns_jobs_when_exist() {
    let app = app(setup_state().await);
    let _ = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input("test-scheduled-job")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs")
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
async fn get_scheduled_job_returns_job_when_exists() {
    let app = app(setup_state().await);
    let create_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input("test-scheduled-job-get")).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let job_id = body_to_json(create_response).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/scheduled-jobs/{job_id}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    assert_eq!(body["id"], job_id);
    assert_eq!(body["name"], "test-scheduled-job-get");
}

#[tokio::test]
async fn get_scheduled_job_returns_404_when_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs/non-existent-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
