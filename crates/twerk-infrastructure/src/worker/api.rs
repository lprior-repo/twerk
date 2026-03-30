//! Worker API module.
//!
//! Provides HTTP API for worker health checks and status.

use std::sync::Arc;
use std::net::SocketAddr;

use axum::{routing::get, Router};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::time::Duration;

use crate::broker::Broker;
use crate::datastore::Datastore;
use crate::runtime::Runtime as RuntimeTrait;

/// Default port range for dynamic port assignment
const MIN_PORT: u16 = 8001;
const MAX_PORT: u16 = 8100;

/// Polling configuration for server startup
const POLLING_MAX_ATTEMPTS: u32 = 100;
const POLLING_DELAY_MS: u64 = 100;

/// Health check result status
#[derive(Debug, Clone, Serialize)]
pub enum HealthStatus {
    Up,
    Down,
}

/// Health check response
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub runtime: Option<String>,
    pub broker: Option<String>,
}

/// Errors that can occur during worker API operations
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("failed to bind to address: {0}")]
    BindError(String),

    #[error("address already in use")]
    AddressInUse,

    #[error("server error: {0}")]
    ServerError(String),

    #[error("timeout waiting for server to start")]
    StartupTimeout,
}

/// WorkerApi provides HTTP endpoints for worker health and status
#[derive(Clone)]
pub struct WorkerApi {
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
    port: u16,
    addr: String,
}

impl WorkerApi {
    /// Create a new WorkerApi
    #[must_use]
    pub fn new(
        broker: Arc<dyn Broker>,
        datastore: Arc<dyn Datastore>,
        runtime: Arc<dyn RuntimeTrait>,
    ) -> Self {
        Self {
            broker,
            datastore,
            runtime,
            port: 0,
            addr: String::new(),
        }
    }

    /// Get the port the API is listening on
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the address the API is listening on
    #[must_use]
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Create the axum router for this API
    fn create_router(&self) -> Router {
        let broker = self.broker.clone();
        let datastore = self.datastore.clone();
        let runtime = self.runtime.clone();

        Router::new().route("/health", get(move || {
            let broker = broker.clone();
            let datastore = datastore.clone();
            let runtime = runtime.clone();
            async move {
                let response = health_check_impl(broker, datastore, runtime).await;
                let status = match response.status {
                    HealthStatus::Up => axum::http::StatusCode::OK,
                    HealthStatus::Down => axum::http::StatusCode::SERVICE_UNAVAILABLE,
                };
                (status, axum::Json(response))
            }
        }))
    }

    /// Start the API server asynchronously
    pub async fn start(&mut self) -> Result<(), ApiError> {
        self.start_on_port(0).await
    }

    /// Start the API server on a specific port
    pub async fn start_on_port(&mut self, port: u16) -> Result<(), ApiError> {
        if port == 0 {
            // Dynamic port assignment
            self.start_on_available_port().await
        } else {
            let addr = format!(":{}", port);
            self.addr = addr.clone();
            self.port = port;
            self.start_server(&addr).await
        }
    }

    /// Start the API server, finding an available port
    async fn start_on_available_port(&mut self) -> Result<(), ApiError> {
        for port in MIN_PORT..MAX_PORT {
            let addr = format!(":{}", port);
            match self.try_start_server(&addr).await {
                Ok(()) => {
                    self.addr = addr.clone();
                    self.port = port;
                    return Ok(());
                }
                Err(ApiError::AddressInUse) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(ApiError::AddressInUse)
    }

    /// Try to start the server on the given address
    async fn try_start_server(&mut self, addr: &str) -> Result<(), ApiError> {
        let addr_parsed: SocketAddr = addr
            .parse()
            .map_err(|e| ApiError::BindError(format!("invalid address: {}", e)))?;

        let listener = TcpListener::bind(addr_parsed)
            .await
            .map_err(|e| {
                if e.to_string().contains("address already in use") {
                    ApiError::AddressInUse
                } else {
                    ApiError::BindError(e.to_string())
                }
            })?;

        self.port = listener
            .local_addr()
            .map_err(|e| ApiError::ServerError(e.to_string()))?
            .port();

        let router = self.create_router();
        let server = axum::serve(listener, router);

        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("worker API server error: {}", e);
            }
        });

        // Wait for server to be ready
        self.wait_for_server().await?;

        tracing::info!("Worker API listening on http://{}", addr);
        Ok(())
    }

    /// Start the server on the given address, returning error if address is in use
    async fn start_server(&mut self, addr: &str) -> Result<(), ApiError> {
        let addr_parsed: SocketAddr = addr
            .parse()
            .map_err(|e| ApiError::BindError(format!("invalid address: {}", e)))?;

        let listener = TcpListener::bind(addr_parsed)
            .await
            .map_err(|e| ApiError::BindError(e.to_string()))?;

        self.port = listener
            .local_addr()
            .map_err(|e| ApiError::ServerError(e.to_string()))?
            .port();

        let router = self.create_router();
        let server = axum::serve(listener, router);

        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("worker API server error: {}", e);
            }
        });

        // Wait for server to be ready
        self.wait_for_server().await?;

        tracing::info!("Worker API listening on http://{}", addr);
        Ok(())
    }

    /// Wait for the server to be ready by polling
    async fn wait_for_server(&self) -> Result<(), ApiError> {
        let addr = format!("127.0.0.1:{}", self.port);
        let delay = Duration::from_millis(POLLING_DELAY_MS);

        for _ in 0..POLLING_MAX_ATTEMPTS {
            if std::net::TcpStream::connect(&addr).is_ok() {
                return Ok(());
            }
            tokio::time::sleep(delay).await;
        }

        Err(ApiError::StartupTimeout)
    }

    /// Shutdown the API server gracefully
    pub async fn shutdown(&self) -> Result<(), ApiError> {
        // In this implementation, the server is spawned on a tokio task
        // Graceful shutdown would require storing the shutdown signal
        // For now, this is a placeholder
        Ok(())
    }
}

/// Perform health check implementation
async fn health_check_impl(
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
) -> HealthResponse {
    let broker_result = broker.health_check().await;
    let datastore_result = datastore.health_check().await;
    let runtime_result = runtime.health_check().await;

    let all_healthy = broker_result.is_ok() && datastore_result.is_ok() && runtime_result.is_ok();

    HealthResponse {
        status: if all_healthy {
            HealthStatus::Up
        } else {
            HealthStatus::Down
        },
        runtime: Some(if runtime_result.is_ok() {
            "ok".to_string()
        } else {
            format!("error: {:?}", runtime_result.err())
        }),
        broker: Some(if broker_result.is_ok() {
            "ok".to_string()
        } else {
            format!("error: {:?}", broker_result.err())
        }),
    }
}

/// Create a new WorkerApi with the given broker and datastore
#[must_use]
pub fn new_api(
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
) -> WorkerApi {
    WorkerApi::new(broker, datastore, runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::{Broker, BoxedFuture, TaskHandler, TaskProgressHandler, HeartbeatHandler, JobHandler, EventHandler, TaskLogPartHandler};
    use crate::datastore::{Datastore, Result as DatastoreResult};
    use crate::runtime::{Runtime as RuntimeTrait, BoxedFuture as RuntimeBoxedFuture, ShutdownResult};
    use async_trait::async_trait;
    use twerk_core::node::Node;
    use twerk_core::task::Task;
    use std::process::ExitCode;

    #[derive(Debug, Clone, Default)]
    struct MockBroker;

    #[async_trait]
    impl Broker for MockBroker {
        fn publish_task(&self, _qname: String, _task: &Task) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(&self, _qname: String, _handler: TaskHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(&self, _handler: TaskProgressHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _node: Node) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(&self, _handler: HeartbeatHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &twerk_core::job::Job) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(&self, _handler: JobHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(&self, _topic: String, _event: serde_json::Value) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(&self, _pattern: String, _handler: EventHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(&self, _part: &twerk_core::task::TaskLogPart) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(&self, _handler: TaskLogPartHandler) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> crate::broker::BoxedFuture<Vec<crate::broker::QueueInfo>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn queue_info(&self, _qname: String) -> crate::broker::BoxedFuture<crate::broker::QueueInfo> {
            Box::pin(async { 
                Ok(crate::broker::QueueInfo { 
                    name: _qname, 
                    size: 0, 
                    subscribers: 0, 
                    unacked: 0,
                }) 
            })
        }
        fn delete_queue(&self, _qname: String) -> crate::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> crate::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockDatastore;

    #[async_trait]
    impl Datastore for MockDatastore {
        async fn create_task(&self, _task: &Task) -> DatastoreResult<()> { Ok(()) }
        async fn update_task(&self, _id: &str, _modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>) -> DatastoreResult<()> { Ok(()) }
        async fn get_task_by_id(&self, _id: &str) -> DatastoreResult<Task> { Ok(Task::default()) }
        async fn get_active_tasks(&self, _job_id: &str) -> DatastoreResult<Vec<Task>> { Ok(Vec::new()) }
        async fn get_next_task(&self, _parent_task_id: &str) -> DatastoreResult<Task> { Ok(Task::default()) }
        async fn create_task_log_part(&self, _part: &twerk_core::task::TaskLogPart) -> DatastoreResult<()> { Ok(()) }
        async fn get_task_log_parts(&self, _task_id: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<crate::datastore::Page<twerk_core::task::TaskLogPart>> { 
            Ok(crate::datastore::Page { items: Vec::new(), number: 0, size: 0, total_pages: 0, total_items: 0 })
        }
        async fn create_node(&self, _node: &Node) -> DatastoreResult<()> { Ok(()) }
        async fn update_node(&self, _id: &str, _modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>) -> DatastoreResult<()> { Ok(()) }
        async fn get_node_by_id(&self, _id: &str) -> DatastoreResult<Node> { Ok(Node::default()) }
        async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> { Ok(Vec::new()) }
        async fn create_job(&self, _job: &twerk_core::job::Job) -> DatastoreResult<()> { Ok(()) }
        async fn update_job(&self, _id: &str, _modify: Box<dyn FnOnce(twerk_core::job::Job) -> DatastoreResult<twerk_core::job::Job> + Send>) -> DatastoreResult<()> { Ok(()) }
        async fn get_job_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::job::Job> { Ok(twerk_core::job::Job::default()) }
        async fn get_job_log_parts(&self, _job_id: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<crate::datastore::Page<twerk_core::task::TaskLogPart>> { 
            Ok(crate::datastore::Page { items: Vec::new(), number: 0, size: 0, total_pages: 0, total_items: 0 })
        }
        async fn get_jobs(&self, _current_user: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<crate::datastore::Page<twerk_core::job::JobSummary>> { 
            Ok(crate::datastore::Page { items: Vec::new(), number: 0, size: 0, total_pages: 0, total_items: 0 })
        }
        async fn create_scheduled_job(&self, _sj: &twerk_core::job::ScheduledJob) -> DatastoreResult<()> { Ok(()) }
        async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<twerk_core::job::ScheduledJob>> { Ok(Vec::new()) }
        async fn get_scheduled_jobs(&self, _current_user: &str, _page: i64, _size: i64) -> DatastoreResult<crate::datastore::Page<twerk_core::job::ScheduledJobSummary>> { 
            Ok(crate::datastore::Page { items: Vec::new(), number: 0, size: 0, total_pages: 0, total_items: 0 })
        }
        async fn get_scheduled_job_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::job::ScheduledJob> { Ok(twerk_core::job::ScheduledJob::default()) }
        async fn update_scheduled_job(&self, _id: &str, _modify: Box<dyn FnOnce(twerk_core::job::ScheduledJob) -> DatastoreResult<twerk_core::job::ScheduledJob> + Send>) -> DatastoreResult<()> { Ok(()) }
        async fn delete_scheduled_job(&self, _id: &str) -> DatastoreResult<()> { Ok(()) }
        async fn create_user(&self, _user: &twerk_core::user::User) -> DatastoreResult<()> { Ok(()) }
        async fn get_user(&self, _username: &str) -> DatastoreResult<twerk_core::user::User> { Ok(twerk_core::user::User::default()) }
        async fn create_role(&self, _role: &twerk_core::role::Role) -> DatastoreResult<()> { Ok(()) }
        async fn get_role(&self, _id: &str) -> DatastoreResult<twerk_core::role::Role> { Ok(twerk_core::role::Role::default()) }
        async fn get_roles(&self) -> DatastoreResult<Vec<twerk_core::role::Role>> { Ok(Vec::new()) }
        async fn get_user_roles(&self, _user_id: &str) -> DatastoreResult<Vec<twerk_core::role::Role>> { Ok(Vec::new()) }
        async fn assign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> { Ok(()) }
        async fn unassign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> { Ok(()) }
        async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> { 
            Ok(twerk_core::stats::Metrics { 
                jobs: twerk_core::stats::JobMetrics { running: 0 }, 
                tasks: twerk_core::stats::TaskMetrics { running: 0 }, 
                nodes: twerk_core::stats::NodeMetrics { running: 0, cpu_percent: 0.0 } 
            })
        }
        async fn with_tx(&self, _f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, DatastoreResult<()>> + Send>) -> DatastoreResult<()> { Ok(()) }
        async fn health_check(&self) -> DatastoreResult<()> { Ok(()) }
    }

    #[derive(Debug, Clone, Default)]
    struct MockRuntime;

    impl RuntimeTrait for MockRuntime {
        fn run(&self, _task: &Task) -> RuntimeBoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn stop(&self, _task: &Task) -> RuntimeBoxedFuture<ShutdownResult<ExitCode>> {
            Box::pin(async { Ok(Ok(ExitCode::SUCCESS)) })
        }
        fn health_check(&self) -> RuntimeBoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn test_worker_api_creation() {
        let broker = Arc::new(MockBroker);
        let datastore = Arc::new(MockDatastore);
        let runtime = Arc::new(MockRuntime);

        let api = new_api(broker, datastore, runtime);

        assert_eq!(api.port(), 0);
    }

    #[tokio::test]
    async fn test_health_check_impl_all_healthy() {
        let broker = Arc::new(MockBroker);
        let datastore = Arc::new(MockDatastore);
        let runtime = Arc::new(MockRuntime);

        let response = health_check_impl(broker, datastore, runtime).await;

        assert!(matches!(response.status, HealthStatus::Up));
    }
}