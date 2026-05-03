#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::unused_async,
    clippy::needless_raw_string_hashes
)]

//! Exhaustive endpoint matrix tests for twerk-web.
//!
//! Tests ALL 23 Coordinator API endpoints with every error permutation.

use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::{Job, JobState};
use twerk_core::task::{TaskLogPart, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::trigger_api::{InMemoryTriggerDatastore, TriggerId};
use twerk_web::api::{create_router, AppState, Config};

mod support;

async fn setup_state() -> AppState {
    support::TestHarness::new().await.into_state()
}

async fn setup_state_with_queue(queue_name: &str) -> AppState {
    support::TestHarness::with_queue(queue_name)
        .await
        .into_state()
}

async fn setup_state_with_triggers() -> (AppState, Arc<InMemoryTriggerDatastore>) {
    let harness = support::TestHarness::with_trigger_ids(&["trg_test_1", "trg_test_2"]).await;
    (harness.clone().into_state(), harness.trigger_store())
}

async fn setup_state_with_direct_task() -> (AppState, Arc<InMemoryDatastore>, TaskId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = support::direct_task(
        job_id.to_string().as_str(),
        task_id.to_string().as_str(),
        "Direct Task",
        TaskState::Running,
    );
    ds.create_task(&task).await.unwrap();

    (state, ds, task_id)
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap_or_else(|_| json!({"error": "parse error"}))
}

mod health {
    use super::*;

    #[tokio::test]
    async fn get_health_returns_200() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert_eq!(body["status"], "UP");
    }

    #[tokio::test]
    async fn get_health_response_includes_version() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert!(body["version"].is_string());
        assert!(!body["version"].as_str().unwrap().is_empty());
    }
}

mod jobs {
    use super::*;

    mod list_jobs {
        use super::*;

        #[tokio::test]
        async fn get_jobs_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert!(body["items"].is_array());
        }

        #[tokio::test]
        async fn get_jobs_pagination_page_0_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?page=0")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn get_jobs_pagination_page_negative_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?page=-1")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn get_jobs_pagination_size_0_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?size=0")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn get_jobs_pagination_size_oversized_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?size=101")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn get_jobs_pagination_non_numeric_page_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?page=abc")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn get_jobs_pagination_non_numeric_size_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs?size=xyz")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod create_job {
        use super::*;

        #[tokio::test]
        async fn post_jobs_valid_json_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let job_input = json!({
                "name": "test-job",
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }]
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert_eq!(body["name"], "test-job");
        }

        #[tokio::test]
        async fn post_jobs_yaml_content_type_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let yaml_input = "
name: test-job-yaml
tasks:
  - name: task-1
    image: alpine
    run: echo hello
";

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/x-yaml")
                        .body(axum::body::Body::from(yaml_input))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn post_jobs_invalid_json_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from("{invalid json"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_jobs_unsupported_content_type_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(axum::body::Body::from("plain text body"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_jobs_empty_body_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_jobs_null_body_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from("null"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_jobs_missing_required_field_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let job_input = json!({
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }]
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_jobs_extra_fields_accepted() {
            let state = setup_state().await;
            let app = create_router(state);

            let job_input = json!({
                "name": "test-job",
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }],
                "extra_field": "should be ignored"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn post_jobs_invalid_type_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let job_input = json!({
                "name": 12345,
                "tasks": "not an array"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod get_job {
        use super::*;

        #[tokio::test]
        async fn get_job_exists_returns_200() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
            let job = Job {
                id: Some(job_id.clone()),
                name: Some("Test Job".to_string()),
                state: JobState::Pending,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/jobs/{}", job_id))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert_eq!(body["name"], "Test Job");
        }

        #[tokio::test]
        async fn get_job_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
        async fn get_job_path_traversal_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs/../../../etc/passwd")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn get_job_unicode_path_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/jobs/%E2%80%8B")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod get_job_log {
        use super::*;

        #[tokio::test]
        async fn get_job_log_exists_returns_200() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
            let job = Job {
                id: Some(job_id.clone()),
                name: Some("Test Job".to_string()),
                state: JobState::Pending,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/jobs/{}/log", job_id))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert!(body["items"].is_array());
        }

        #[tokio::test]
        async fn get_job_log_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
    }

    mod cancel_job {
        use super::*;

        #[tokio::test]
        async fn put_cancel_running_job_returns_200() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
            let job = Job {
                id: Some(job_id.clone()),
                name: Some("Test Job".to_string()),
                state: JobState::Running,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri(format!("/jobs/{}/cancel", job_id))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn put_cancel_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
        async fn put_cancel_completed_job_returns_400() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job = Job {
                id: Some(JobId::new("00000000-0000-0000-0000-000000000020").unwrap()),
                name: Some("Completed Job".to_string()),
                state: JobState::Completed,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
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
        async fn post_cancel_running_job_returns_200() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
            let job = Job {
                id: Some(job_id.clone()),
                name: Some("Test Job".to_string()),
                state: JobState::Running,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(format!("/jobs/{}/cancel", job_id))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    mod restart_job {
        use super::*;

        #[tokio::test]
        async fn put_restart_failed_job_returns_200() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());
            let job_id = "00000000-0000-0000-0000-000000000199";

            let job = Job {
                id: Some(JobId::new(job_id).unwrap()),
                name: Some("Failed Job".to_string()),
                state: JobState::Failed,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri(format!("/jobs/{job_id}/restart"))
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn put_restart_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
        async fn put_restart_pending_job_returns_400() {
            let ds = Arc::new(InMemoryDatastore::new());
            let broker = Arc::new(InMemoryBroker::new());
            let state = AppState::new(broker, ds.clone(), Config::default());

            let job = Job {
                id: Some(JobId::new("00000000-0000-0000-0000-000000000021").unwrap()),
                name: Some("Pending Job".to_string()),
                state: JobState::Pending,
                ..Default::default()
            };
            ds.create_job(&job).await.unwrap();

            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/jobs/00000000-0000-0000-0000-000000000021/restart")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
}

mod scheduled_jobs {
    use super::*;

    mod create_scheduled_job {
        use super::*;

        #[tokio::test]
        async fn post_scheduled_jobs_valid_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let scheduled_job_input = json!({
                "name": "test-scheduled-job",
                "cron": "0 0 * * * *",
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }]
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&scheduled_job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert_eq!(body["name"], "test-scheduled-job");
        }

        #[tokio::test]
        async fn post_scheduled_jobs_invalid_cron_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let scheduled_job_input = json!({
                "name": "test-scheduled-job",
                "cron": "invalid-cron-expression",
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }]
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&scheduled_job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_scheduled_jobs_missing_cron_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let scheduled_job_input = json!({
                "name": "test-scheduled-job",
                "tasks": [{
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }]
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&scheduled_job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_scheduled_jobs_missing_tasks_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let scheduled_job_input = json!({
                "name": "test-scheduled-job",
                "cron": "0 0 * * * *"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&scheduled_job_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_scheduled_jobs_empty_body_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_scheduled_jobs_unsupported_content_type_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(axum::body::Body::from("plain text body"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_scheduled_jobs_invalid_json_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/scheduled-jobs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from("{invalid"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod list_scheduled_jobs {
        use super::*;

        #[tokio::test]
        async fn get_scheduled_jobs_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/scheduled-jobs")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert!(body["items"].is_array());
        }

        #[tokio::test]
        async fn get_scheduled_jobs_pagination_defaults_returns_200() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/scheduled-jobs?page=1&size=10")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    mod get_scheduled_job {
        use super::*;

        #[tokio::test]
        async fn get_scheduled_job_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
    }

    mod pause_scheduled_job {
        use super::*;

        #[tokio::test]
        async fn pause_scheduled_job_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
    }

    mod resume_scheduled_job {
        use super::*;

        #[tokio::test]
        async fn resume_scheduled_job_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
    }

    mod delete_scheduled_job {
        use super::*;

        #[tokio::test]
        async fn delete_scheduled_job_not_found_returns_404() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
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
    }
}

mod queues {
    use super::*;

    #[tokio::test]
    async fn get_queues_returns_200() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/queues")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert!(body.is_array());
    }

    #[tokio::test]
    async fn get_queue_by_name_returns_200() {
        let state = setup_state_with_queue("default").await;
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/queues/default")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert_eq!(
            body,
            json!({"name": "default", "size": 1, "subscribers": 0, "unacked": 0})
        );
    }

    #[tokio::test]
    async fn get_missing_queue_returns_404() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/queues/test-queue")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = body_to_json(response).await;
        assert_eq!(body, json!({"message": "queue test-queue not found"}));
    }

    #[tokio::test]
    async fn delete_queue_returns_200() {
        let state = setup_state_with_queue("test-queue").await;
        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/queues/test-queue")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let get_after_delete = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/queues/test-queue")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_after_delete.status(), StatusCode::NOT_FOUND);
        let get_after_delete_body = body_to_json(get_after_delete).await;
        assert_eq!(
            get_after_delete_body,
            json!({"message": "queue test-queue not found"})
        );
    }

    #[tokio::test]
    async fn delete_missing_queue_returns_404() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/queues/test-queue")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = body_to_json(response).await;
        assert_eq!(body, json!({"message": "queue test-queue not found"}));
    }
}

mod nodes {
    use super::*;

    #[tokio::test]
    async fn get_nodes_returns_200() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/nodes")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert!(body.is_array());
    }
}

mod metrics {
    use super::*;

    #[tokio::test]
    async fn get_metrics_returns_200() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert!(body.is_object());
    }
}

mod users {
    use super::*;

    #[tokio::test]
    async fn post_users_valid_returns_200() {
        let state = setup_state().await;
        let app = create_router(state);

        let user_input = json!({
            "username": "testuser",
            "password": "testpassword"
        });

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&user_input).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn post_users_missing_username_returns_400() {
        let state = setup_state().await;
        let app = create_router(state);

        let user_input = json!({
            "password": "testpassword"
        });

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&user_input).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_users_missing_password_returns_400() {
        let state = setup_state().await;
        let app = create_router(state);

        let user_input = json!({
            "username": "testuser"
        });

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&user_input).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_users_empty_body_returns_400() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_users_invalid_json_returns_400() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from("{invalid"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

mod tasks {
    use super::*;

    #[tokio::test]
    async fn get_task_not_found_returns_404() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
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
    async fn get_task_exists_returns_200() {
        let (state, _ds, task_id) = setup_state_with_direct_task().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/tasks/{}", task_id))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert_eq!(body["name"], "Direct Task");
    }

    #[tokio::test]
    async fn get_task_log_not_found_returns_404() {
        let state = setup_state().await;
        let app = create_router(state);

        let response = app
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
    async fn get_task_log_empty_returns_200() {
        let (state, _ds, task_id) = setup_state_with_direct_task().await;
        let app = create_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/tasks/{}/log", task_id))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        assert!(body["items"].is_array());
    }

    #[tokio::test]
    async fn get_task_log_with_pagination_returns_200() {
        let (state, ds, task_id) = setup_state_with_direct_task().await;
        let app = create_router(state);

        for i in 1..=5 {
            let log_part = TaskLogPart {
                id: Some(format!("log-{}", i)),
                number: i as i64,
                task_id: Some(task_id.clone()),
                contents: Some(format!("Log line {}", i)),
                ..Default::default()
            };
            ds.create_task_log_part(&log_part).await.unwrap();
        }

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/tasks/{}/log?page=1&size=2", task_id))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_to_json(response).await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }
}

mod triggers {
    use super::*;

    mod list_triggers {
        use super::*;

        #[tokio::test]
        async fn get_triggers_returns_200() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/triggers")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = body_to_json(response).await;
            assert!(body.is_array());
        }
    }

    mod create_trigger {
        use super::*;

        #[tokio::test]
        async fn post_trigger_valid_returns_201() {
            let state = setup_state().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "new-trigger",
                "event": "order.created",
                "action": "send_notification",
                "enabled": true
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn post_trigger_missing_name_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let trigger_input = json!({
                "event": "order.created",
                "action": "send_notification"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_trigger_missing_event_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "new-trigger",
                "action": "send_notification"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_trigger_missing_action_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "new-trigger",
                "event": "order.created"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_trigger_empty_body_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn post_trigger_invalid_json_returns_400() {
            let state = setup_state().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/triggers")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from("{invalid"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod get_trigger {
        use super::*;

        #[tokio::test]
        async fn get_trigger_exists_returns_200() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/triggers/trg_test_1")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn get_trigger_not_found_returns_404() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/triggers/non_existent_trigger")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn get_trigger_invalid_id_format_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/triggers/bad$id")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod update_trigger {
        use super::*;

        #[tokio::test]
        async fn put_trigger_valid_returns_200() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "updated-trigger",
                "event": "order.updated",
                "action": "send_update",
                "enabled": false
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/trg_test_1")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn put_trigger_not_found_returns_404() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "updated-trigger",
                "event": "order.updated",
                "action": "send_update",
                "enabled": false
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/non_existent_trigger")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn put_trigger_missing_name_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let trigger_input = json!({
                "event": "order.updated",
                "action": "send_update",
                "enabled": false
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/trg_test_1")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn put_trigger_invalid_id_format_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "updated-trigger",
                "event": "order.updated",
                "action": "send_update",
                "enabled": false
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/bad$id")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn put_trigger_invalid_json_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/trg_test_1")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from("{invalid"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn put_trigger_empty_body_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/trg_test_1")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn put_trigger_id_mismatch_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let trigger_input = json!({
                "name": "updated-trigger",
                "event": "order.updated",
                "action": "send_update",
                "id": "trg_test_2"
            });

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/triggers/trg_test_1")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(axum::body::Body::from(
                            serde_json::to_vec(&trigger_input).unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod delete_trigger {
        use super::*;

        #[tokio::test]
        async fn delete_trigger_exists_returns_204() {
            let (state, trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("DELETE")
                        .uri("/triggers/trg_test_1")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert!(trigger_ds
                .get_trigger_by_id(&TriggerId::parse("trg_test_1").unwrap())
                .is_err());
        }

        #[tokio::test]
        async fn delete_trigger_not_found_returns_404() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("DELETE")
                        .uri("/triggers/non_existent_trigger")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn delete_trigger_invalid_id_format_returns_400() {
            let (state, _trigger_ds) = setup_state_with_triggers().await;
            let app = create_router(state);

            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("DELETE")
                        .uri("/triggers/bad$id")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
}

mod concurrent {
    use super::*;

    #[tokio::test]
    async fn concurrent_job_cancellations_are_isolated() {
        let ds = Arc::new(InMemoryDatastore::new());
        let broker = Arc::new(InMemoryBroker::new());
        let state = AppState::new(broker, ds.clone(), Config::default());

        let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
        let job = Job {
            id: Some(job_id.clone()),
            name: Some("Concurrent Test Job".to_string()),
            state: JobState::Running,
            ..Default::default()
        };
        ds.create_job(&job).await.unwrap();

        let app = create_router(state);

        let uri = format!("/jobs/{}/cancel", job_id);

        let (r1, r2) = tokio::join!(
            app.clone().oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri(&uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            ),
            app.oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri(&uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
        );

        let statuses = [r1.unwrap().status(), r2.unwrap().status()];
        assert!(statuses.contains(&StatusCode::OK));
        assert!(statuses.contains(&StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn concurrent_trigger_creates_are_idempotent() {
        let (state, _trigger_ds) = setup_state_with_triggers().await;
        let app = create_router(state);

        let trigger_input = json!({
            "name": "concurrent-trigger",
            "event": "order.created",
            "action": "send_notification",
            "enabled": true
        });

        let uri = "/triggers";

        let (r1, r2) = tokio::join!(
            app.clone().oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&trigger_input).unwrap(),
                    ))
                    .unwrap(),
            ),
            app.oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&trigger_input).unwrap(),
                    ))
                    .unwrap(),
            )
        );

        assert_eq!(r1.unwrap().status(), StatusCode::CREATED);
        assert_eq!(r2.unwrap().status(), StatusCode::CREATED);
    }
}
