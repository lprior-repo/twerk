//! HTTP API for worker health checks and monitoring.
//!
//! This module provides an HTTP API server for worker health checks
//! using the axum framework.

use crate::syncx::Map;
use crate::worker::RunningTask;
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tork::broker::Broker;
use tork::runtime::Runtime;
use tork::version::VERSION;

/// Minimum port number for dynamic assignment
pub const MIN_PORT: u16 = 8001;
/// Maximum port number for dynamic assignment
pub const MAX_PORT: u16 = 8100;

/// Shared API state (Clone-friendly because Broker/Runtime are behind Arc)
#[derive(Clone)]
struct ApiState {
    broker: Arc<dyn Broker>,
    runtime: Arc<dyn Runtime>,
    #[allow(dead_code)]
    tasks: Arc<Map<String, RunningTask>>,
}

/// Health check result
#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

/// The API server
pub struct Api {
    /// HTTP server handle
    handle: Option<tokio::task::JoinHandle<Result<(), std::io::Error>>>,
    /// Shutdown sender
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Assigned port
    port: i64,
    /// Broker reference (stored for start_listener)
    broker: Arc<dyn Broker>,
    /// Runtime reference (stored for start_listener)
    runtime: Arc<dyn Runtime>,
    /// Tasks reference (stored for start_listener)
    tasks: Arc<Map<String, RunningTask>>,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api").field("port", &self.port).finish()
    }
}

impl Api {
    /// Creates a new API server, storing broker/runtime/tasks for the health endpoint
    pub fn new(
        address: Option<String>,
        broker: Arc<dyn Broker>,
        runtime: Arc<dyn Runtime>,
        tasks: Arc<Map<String, RunningTask>>,
    ) -> Self {
        let port = address
            .and_then(|addr| addr.strip_prefix(':').and_then(|p| p.parse().ok()))
            .unwrap_or(0);

        Self {
            handle: None,
            shutdown_tx: None,
            port,
            broker,
            runtime,
            tasks,
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

        // Helper to build a valid SocketAddr (Go accepts ":port" but Rust needs "ip:port")
        let make_addr = |port: u16| -> SocketAddr {
            format!("127.0.0.1:{}", port)
                .parse()
                .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], port)))
        };

        // Dynamic port assignment if not specified
        if self.port == 0 {
            for port in MIN_PORT..MAX_PORT {
                let addr = make_addr(port);
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

        let addr = make_addr(self.port as u16);
        let listener = TcpListener::bind(addr).await?;
        self.start_listener(listener, shutdown_rx).await
    }

    async fn start_listener(
        &mut self,
        listener: TcpListener,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Result<(), anyhow::Error> {
        // Use the real broker and runtime (not Noop) for accurate health checks
        let state = ApiState {
            broker: Arc::clone(&self.broker),
            runtime: Arc::clone(&self.runtime),
            tasks: Arc::clone(&self.tasks),
        };

        let app = Router::new()
            .route("/health", get(health_handler))
            .with_state(state);

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
            broker: Arc::clone(&self.broker),
            runtime: Arc::clone(&self.runtime),
            tasks: Arc::clone(&self.tasks),
        }
    }
}

/// Health check handler — mirrors Go's health check with runtime + broker indicators
async fn health_handler(State(state): State<ApiState>) -> (StatusCode, Json<HealthResponse>) {
    // Check runtime health
    let runtime_ok = state.runtime.health_check().await.is_ok();

    // Check broker health
    let broker_ok = state.broker.health_check().await.is_ok();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::inmemory::new_in_memory_broker;
    use tower::ServiceExt;

    /// A no-op runtime that always reports healthy for testing
    struct HealthyRuntime;

    impl Runtime for HealthyRuntime {
        fn run(
            &self,
            _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
            _task: &mut tork::task::Task,
        ) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }

        fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    /// A runtime that always reports unhealthy for testing
    struct UnhealthyRuntime;

    impl Runtime for UnhealthyRuntime {
        fn run(
            &self,
            _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
            _task: &mut tork::task::Task,
        ) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }

        fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Err(anyhow::anyhow!("runtime unhealthy")) })
        }
    }

    /// Builds the health check router with the given state
    fn health_app(state: ApiState) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health_up() {
        let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());
        let runtime: Arc<dyn Runtime> = Arc::new(HealthyRuntime);
        let tasks = Arc::new(Map::new());

        let state = ApiState {
            broker: Arc::clone(&broker),
            runtime: Arc::clone(&runtime),
            tasks: Arc::clone(&tasks),
        };

        let app = health_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, response.status());

        let body_bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("\"status\":\"UP\""));
        assert!(body_str.contains(&format!("\"version\":\"{}\"", VERSION)));
    }

    #[tokio::test]
    async fn test_health_down_unhealthy_runtime() {
        let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());
        let runtime: Arc<dyn Runtime> = Arc::new(UnhealthyRuntime);
        let tasks = Arc::new(Map::new());

        let state = ApiState {
            broker: Arc::clone(&broker),
            runtime: Arc::clone(&runtime),
            tasks: Arc::clone(&tasks),
        };

        let app = health_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());

        let body_bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("\"status\":\"DOWN\""));
    }

    #[tokio::test]
    async fn test_health_down_unhealthy_broker() {
        let broker = new_in_memory_broker();
        broker.shutdown().await.unwrap();
        let broker: Arc<dyn Broker> = Arc::new(broker);
        let runtime: Arc<dyn Runtime> = Arc::new(HealthyRuntime);
        let tasks = Arc::new(Map::new());

        let state = ApiState {
            broker: Arc::clone(&broker),
            runtime: Arc::clone(&runtime),
            tasks: Arc::clone(&tasks),
        };

        let app = health_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::SERVICE_UNAVAILABLE, response.status());

        let body_bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("\"status\":\"DOWN\""));
    }
}
