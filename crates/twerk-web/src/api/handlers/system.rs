//! System handlers - API endpoints for system operations.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::node::Node;
use twerk_core::stats::Metrics;

use super::super::domain::{Password, PasswordError, Username, UsernameError};
use super::super::error::ApiError;
use super::super::openapi_types::{HealthResponse, MessageResponse};
use super::{AppState, VERSION};
use tracing::instrument;
use utoipa::ToSchema;

/// Health check handler
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse, content_type = "application/json"),
        (status = 503, description = "Service is unhealthy", body = HealthResponse, content_type = "application/json")
    )
)]
#[instrument(name = "health_handler", skip_all)]
pub async fn health_handler(State(state): State<AppState>) -> Response {
    let ds_ok = state.ds.health_check().await.is_ok();
    let broker_ok = state.broker.health_check().await.is_ok();

    let (status, body) = if ds_ok && broker_ok {
        (StatusCode::OK, json!({"status": "UP", "version": VERSION}))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status": "DOWN", "version": VERSION}),
        )
    };
    (status, axum::Json(body)).into_response()
}

/// GET /nodes
#[utoipa::path(
    get,
    path = "/nodes",
    responses(
        (status = 200, description = "List of active nodes", body = Vec<Node>, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "list_nodes_handler", skip_all)]
pub async fn list_nodes_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let nodes = state.ds.get_active_nodes().await.map_err(ApiError::from)?;
    Ok(axum::Json(nodes).into_response())
}

/// GET /metrics
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "System metrics", body = Metrics, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "get_metrics_handler", skip_all)]
pub async fn get_metrics_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let metrics = state.ds.get_metrics().await.map_err(ApiError::from)?;
    Ok(axum::Json(metrics).into_response())
}

/// User creation body
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserBody {
    pub username: Option<String>,
    pub password: Option<String>,
}

fn username_error_to_string(err: &UsernameError) -> String {
    match err {
        UsernameError::Empty => "username cannot be empty".to_string(),
        UsernameError::LengthOutOfRange => "username must be 3-64 characters".to_string(),
        UsernameError::InvalidCharacter => {
            "username must start with a letter and contain only alphanumeric characters, underscores, or hyphens".to_string()
        }
    }
}

fn password_error_to_string(err: &PasswordError) -> String {
    match err {
        PasswordError::Empty => "password cannot be empty".to_string(),
        PasswordError::TooShort => "password must be at least 8 characters".to_string(),
    }
}

/// POST /users
///
/// # Errors
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserBody,
    responses(
        (status = 200, description = "User created"),
        (status = 400, description = "Missing username or password", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "create_user_handler", skip_all)]
pub async fn create_user_handler(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateUserBody>,
) -> Result<Response, ApiError> {
    let username = body
        .username
        .ok_or_else(|| ApiError::bad_request("username is required"))?;
    let username = Username::new(&username).map_err(|e| {
        ApiError::bad_request(format!(
            "invalid username: {}",
            username_error_to_string(&e)
        ))
    })?;

    let password = body
        .password
        .ok_or_else(|| ApiError::bad_request("password is required"))?;
    let password = Password::new(&password).map_err(|e| {
        ApiError::bad_request(format!(
            "invalid password: {}",
            password_error_to_string(&e)
        ))
    })?;

    let password_hash = bcrypt::hash(password.as_str(), bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let user_id = twerk_core::id::UserId::new(twerk_core::uuid::new_short_uuid())?;

    let user = twerk_core::user::User {
        id: Some(user_id),
        username: Some(username.as_str().to_string()),
        password_hash: Some(password_hash),
        ..Default::default()
    };

    state.ds.create_user(&user).await.map_err(ApiError::from)?;

    Ok(StatusCode::OK.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow;
    use async_trait::async_trait;
    use axum::http::{header, StatusCode};
    use axum::routing::get;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tower::ServiceExt;
    use twerk_core::task::Task;
    use twerk_core::job::Job;
    use twerk_core::node::Node;
    use crate::api::trigger_api::{InMemoryTriggerDatastore, TriggerAppState};
    use crate::api::Config;

    #[derive(Debug, Clone)]
    struct MockDatastore {
        healthy: bool,
    }

    #[async_trait]
    impl twerk_infrastructure::datastore::Datastore for MockDatastore {
        async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
            if self.healthy {
                Ok(())
            } else {
                Err(twerk_infrastructure::datastore::Error::Database(
                    "database unreachable".to_string(),
                ))
            }
        }

        async fn create_task(
            &self,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn update_task(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(twerk_core::task::Task) -> twerk_infrastructure::datastore::Result<Task>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_task_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            Ok(twerk_core::task::Task::default())
        }
        async fn get_active_tasks(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            Ok(Vec::new())
        }
        async fn get_all_tasks_for_job(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            Ok(Vec::new())
        }
        async fn get_next_task(
            &self,
            _parent_task_id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            Ok(twerk_core::task::Task::default())
        }
        async fn create_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_task_log_parts(
            &self,
            _task_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>>
        {
            Ok(twerk_infrastructure::datastore::Page {
                items: Vec::new(),
                number: 0,
                size: 0,
                total_pages: 0,
                total_items: 0,
            })
        }
        async fn create_node(
            &self,
            _node: &twerk_core::node::Node,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn update_node(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(twerk_core::node::Node) -> twerk_infrastructure::datastore::Result<Node>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_node_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::node::Node> {
            Ok(twerk_core::node::Node::default())
        }
        async fn get_active_nodes(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::node::Node>> {
            Ok(Vec::new())
        }
        async fn create_job(
            &self,
            _job: &twerk_core::job::Job,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn update_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(twerk_core::job::Job) -> twerk_infrastructure::datastore::Result<Job>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::Job> {
            Ok(twerk_core::job::Job::default())
        }
        async fn get_job_log_parts(
            &self,
            _job_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>>
        {
            Ok(twerk_infrastructure::datastore::Page {
                items: Vec::new(),
                number: 0,
                size: 0,
                total_pages: 0,
                total_items: 0,
            })
        }
        async fn get_jobs(
            &self,
            _current_user: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<twerk_infrastructure::datastore::Page<twerk_core::job::JobSummary>>
        {
            Ok(twerk_infrastructure::datastore::Page {
                items: Vec::new(),
                number: 0,
                size: 0,
                total_pages: 0,
                total_items: 0,
            })
        }
        async fn create_scheduled_job(
            &self,
            _sj: &twerk_core::job::ScheduledJob,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_active_scheduled_jobs(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::job::ScheduledJob>> {
            Ok(Vec::new())
        }
        async fn get_scheduled_jobs(
            &self,
            _current_user: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<twerk_infrastructure::datastore::Page<twerk_core::job::ScheduledJobSummary>>
        {
            Ok(twerk_infrastructure::datastore::Page {
                items: Vec::new(),
                number: 0,
                size: 0,
                total_pages: 0,
                total_items: 0,
            })
        }
        async fn get_scheduled_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob> {
            Ok(twerk_core::job::ScheduledJob::default())
        }
        async fn update_scheduled_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::ScheduledJob,
                    ) -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn delete_scheduled_job(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn create_user(
            &self,
            _user: &twerk_core::user::User,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_user(
            &self,
            _username: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::user::User> {
            Ok(twerk_core::user::User::default())
        }
        async fn create_role(
            &self,
            _role: &twerk_core::role::Role,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_role(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::role::Role> {
            Ok(twerk_core::role::Role::default())
        }
        async fn get_roles(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            Ok(Vec::new())
        }
        async fn get_user_roles(
            &self,
            _user_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            Ok(Vec::new())
        }
        async fn assign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn unassign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
        async fn get_metrics(
            &self,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
            Ok(twerk_core::stats::Metrics {
                jobs: twerk_core::stats::JobMetrics { running: 0 },
                tasks: twerk_core::stats::TaskMetrics { running: 0 },
                nodes: twerk_core::stats::NodeMetrics {
                    running: 0,
                    cpu_percent: 0.0,
                },
            })
        }
        async fn with_tx(
            &self,
            _f: Box<
                dyn for<'a> FnOnce(
                        &'a dyn twerk_infrastructure::datastore::Datastore,
                    )
                        -> futures_util::future::BoxFuture<'a, twerk_infrastructure::datastore::Result<()>>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct MockBroker {
        healthy: bool,
    }

    impl MockBroker {
        fn new(healthy: bool) -> Self {
            Self { healthy }
        }
    }

    impl twerk_infrastructure::broker::Broker for MockBroker {
        fn publish_task(
            &self,
            _qname: String,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(
            &self,
            _qname: String,
            _handler: twerk_infrastructure::broker::TaskHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(
            &self,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(
            &self,
            _handler: twerk_infrastructure::broker::TaskProgressHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(
            &self,
            _node: twerk_core::node::Node,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(
            &self,
            _handler: twerk_infrastructure::broker::HeartbeatHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(
            &self,
            _job: &twerk_core::job::Job,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(
            &self,
            _handler: twerk_infrastructure::broker::JobHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(
            &self,
            _topic: String,
            _event: serde_json::Value,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(
            &self,
            _pattern: String,
            _handler: twerk_infrastructure::broker::EventHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe(
            &self,
            _pattern: String,
        ) -> twerk_infrastructure::broker::BoxedFuture<tokio::sync::broadcast::Receiver<twerk_core::job::JobEvent>>
        {
            let (tx, rx) = tokio::sync::broadcast::channel(256);
            drop(tx);
            Box::pin(async { Ok(rx) })
        }
        fn publish_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(
            &self,
            _handler: twerk_infrastructure::broker::TaskLogPartHandler,
        ) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
            if self.healthy {
                Box::pin(async { Ok(()) })
            } else {
                Box::pin(async { Err(anyhow::anyhow!("broker unreachable")) })
            }
        }
        fn queues(&self) -> twerk_infrastructure::broker::BoxedFuture<Vec<twerk_infrastructure::broker::QueueInfo>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn queue_info(
            &self,
            _qname: String,
        ) -> twerk_infrastructure::broker::BoxedFuture<twerk_infrastructure::broker::QueueInfo> {
            Box::pin(async {
                Ok(twerk_infrastructure::broker::QueueInfo {
                    name: _qname,
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            })
        }
        fn delete_queue(&self, _qname: String) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    fn create_app_state(
        ds_healthy: bool,
        broker_healthy: bool,
    ) -> AppState {
        AppState {
            ds: Arc::new(MockDatastore { healthy: ds_healthy }),
            broker: Arc::new(MockBroker::new(broker_healthy)),
            trigger_state: TriggerAppState {
                trigger_ds: Arc::new(InMemoryTriggerDatastore::new()),
            },
            config: Config::default(),
        }
    }

    #[tokio::test]
    async fn test_health_handler_healthy_db() {
        let state = create_app_state(true, true);
        let app = axum::Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("application/json"));

        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "UP");
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn test_health_handler_unreachable_db() {
        let state = create_app_state(false, true);
        let app = axum::Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "DOWN");
    }

    #[tokio::test]
    async fn test_health_handler_response_time_under_100ms() {
        let state = create_app_state(true, true);
        let app = axum::Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

        let start = Instant::now();
        let _response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(100),
            "Response time {}ms exceeds 100ms threshold",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_health_handler_content_type_json() {
        let state = create_app_state(true, true);
        let app = axum::Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("application/json"),
            "Expected content-type application/json, got: {}",
            content_type
        );
    }

    #[tokio::test]
    async fn test_health_handler_broker_unreachable() {
        let state = create_app_state(true, false);
        let app = axum::Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "DOWN");
    }
}
