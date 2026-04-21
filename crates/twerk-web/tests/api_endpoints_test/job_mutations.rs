use axum::http::StatusCode;
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::{AppState, Config};

use super::shared::{
    app, body_to_json, setup_state, setup_state_with_jobs, setup_state_with_tasks,
};

#[tokio::test]
async fn cancel_job_returns_ok_when_job_is_running() {
    let (state, _, _) = setup_state_with_tasks().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000001/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(body_to_json(response).await["status"], "OK");
}

#[tokio::test]
async fn cancel_job_returns_404_when_job_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000404/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_job_returns_400_when_job_not_cancellable() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());
    ds.create_job(&Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000020").unwrap()),
        name: Some("Completed Job".to_string()),
        state: JobState::Completed,
        ..Default::default()
    })
    .await
    .unwrap();

    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000020/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn restart_job_returns_ok_when_job_is_failed() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());
    let job_id = "00000000-0000-0000-0000-000000000099";
    ds.create_job(&Job {
        id: Some(JobId::new(job_id).unwrap()),
        name: Some("Failed Job".to_string()),
        state: JobState::Failed,
        ..Default::default()
    })
    .await
    .unwrap();

    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/jobs/{job_id}/restart"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(body_to_json(response).await["status"], "OK");
}

#[tokio::test]
async fn restart_job_returns_404_when_job_not_found() {
    let response = app(setup_state().await)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000404/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn restart_job_returns_400_when_job_not_restartable() {
    let (state, _) = setup_state_with_jobs().await;
    let response = app(state)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000010/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
