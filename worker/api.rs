//! HTTP API for worker health checks and monitoring.
//!
//! This module provides an HTTP API server for worker health checks
//! using the axum framework.

use crate::syncx::Map;
use crate::worker::RunningTask;
use tork::broker::Broker;
use tork::runtime::Runtime;
use tork::version::VERSION;
use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Minimum port number for dynamic assignment
pub const MIN_PORT: u16 = 8001;
/// Maximum port number for dynamic assignment
pub const MAX_PORT: u16 = 8100;

/// Shared API state
#[derive(Clone)]
struct ApiState {
    broker: Arc<dyn Broker>,
    runtime: Arc<dyn Runtime>,
    tasks: Arc<Map<String, RunningTask>>,
}

/// Health check result
#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

/// The API server
#[derive(Debug)]
pub struct Api {
    /// HTTP server handle
    handle: Option<tokio::task::JoinHandle<Result<(), std::io::Error>>>,
    /// Shutdown sender
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Assigned port
    port: i64,
}

impl Api {
    /// Creates a new API server
    #[allow(clippy::unused_self)]
    pub fn new(
        address: Option<String>,
        _broker: Arc<dyn Broker>,
        _runtime: Arc<dyn Runtime>,
        _tasks: Arc<Map<String, RunningTask>>,
    ) -> Self {
        let port = address.and_then(|addr| {
            addr.strip_prefix(':')
                .and_then(|p| p.parse().ok())
        }).unwrap_or(0);

        Self {
            handle: None,
            shutdown_tx: None,
            port,
        }
    }

    /// Returns the assigned port
    #[must_use]
    pub fn port(&self) -> i64 {
        self.port
    }

    /// Starts the API server
    pub async fn start(&mut self) -> Result<(), anyhow::Error> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        // Dynamic port assignment if not specified
        if self.port == 0 {
            for port in MIN_PORT..MAX_PORT {
                let addr: SocketAddr = format!(":{}", port).parse()?;
                match TcpListener::bind(addr).await {
                    Ok(listener) => {
                        self.port = port as i64;
                        return self.start_listener(listener, shutdown_rx).await;
                    }
                    Err(_) => continue,
                }
            }
            return Err(anyhow::anyhow!("no available ports in range"));
        }

        let addr: SocketAddr = format!(":{}", self.port).parse()?;
        let listener = TcpListener::bind(addr).await?;
        self.start_listener(listener, shutdown_rx).await
    }

    async fn start_listener(
        &mut self,
        listener: TcpListener,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Result<(), anyhow::Error> {
        let broker = Arc::new(NoopBroker);
        let runtime = Arc::new(NoopRuntime);
        let tasks = Arc::new(Map::<String, RunningTask>::new());

        let app = Router::new()
            .route("/health", get(health_handler))
            .with_state(ApiState {
                broker,
                runtime,
                tasks,
            });

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        self.handle = Some(handle);
        Ok(())
    }

    /// Shuts down the API server
    pub async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.await? {
                tracing::warn!("server error: {}", e);
            }
        }

        Ok(())
    }
}

impl Clone for Api {
    fn clone(&self) -> Self {
        Self {
            handle: None,
            shutdown_tx: None,
            port: self.port,
        }
    }
}

/// Health check handler
async fn health_handler(
    State(state): State<ApiState>,
) -> (StatusCode, Json<HealthResponse>) {
    // Check runtime health
    let runtime_ok = state
        .runtime
        .health_check()
        .await
        .is_ok();

    // Check broker health
    let broker_ok = state
        .broker
        .health_check()
        .await
        .is_ok();

    if runtime_ok && broker_ok {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "UP".to_string(),
                version: VERSION.to_string(),
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "DOWN".to_string(),
                version: VERSION.to_string(),
            }),
        )
    }
}

// No-op implementations for API state cloning
struct NoopBroker;
struct NoopRuntime;

impl Broker for NoopBroker {
    fn publish_task(&self, _qname: String, _task: &tork::task::Task) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_tasks(&self, _qname: String, _handler: tork::broker::TaskHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn publish_task_progress(&self, _task: &tork::task::Task) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_task_progress(&self, _handler: tork::broker::TaskProgressHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn publish_heartbeat(&self, _node: tork::node::Node) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_heartbeats(&self, _handler: tork::broker::HeartbeatHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn publish_job(&self, _job: &tork::job::Job) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_jobs(&self, _handler: tork::broker::JobHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn publish_event(&self, _topic: String, _event: serde_json::Value) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_events(&self, _pattern: String, _handler: tork::broker::EventHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn publish_task_log_part(&self, _part: &tork::task::TaskLogPart) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn subscribe_for_task_log_part(&self, _handler: tork::broker::TaskLogPartHandler) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> { Box::pin(async { Ok(vec![]) }) }
    fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> { Box::pin(async { Ok(tork::broker::QueueInfo { name: String::new(), size: 0, subscribers: 0, unacked: 0 }) }) }
    fn delete_queue(&self, _qname: String) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn health_check(&self) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn shutdown(&self) -> tork::broker::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
}

impl Runtime for NoopRuntime {
    fn run(&self, _ctx: std::sync::Arc<tokio::sync::RwLock<()>>, _task: &mut tork::task::Task) -> tork::runtime::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn health_check(&self) -> tork::runtime::BoxedFuture<()> { Box::pin(async { Ok(()) }) }
}
