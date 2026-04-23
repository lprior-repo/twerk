//! Worker API server implementation.
//!
//! Provides HTTP endpoints for worker health and status.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tokio::time::Duration;

use crate::broker::Broker;
use crate::datastore::Datastore;
use crate::runtime::Runtime as RuntimeTrait;

use super::types::{
    WorkerApiError, HealthResponse, HealthStatus, MAX_PORT, MIN_PORT, POLLING_DELAY_MS,
    POLLING_MAX_ATTEMPTS,
};

/// `WorkerApi` provides HTTP endpoints for worker health and status
#[derive(Clone)]
pub struct WorkerApi {
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
    port: u16,
    addr: String,
}

impl WorkerApi {
    /// Create a new `WorkerApi`
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

        Router::new().route(
            "/health",
            get(move || {
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
            }),
        )
    }

    /// Start the API server asynchronously
    ///
    /// # Errors
    ///
    /// Returns `WorkerApiError` if the server fails to start.
    pub async fn start(&mut self) -> Result<(), WorkerApiError> {
        self.start_on_port(0).await
    }

    /// Start the API server on a specific port
    ///
    /// # Errors
    ///
    /// Returns `WorkerApiError` if the server fails to start.
    pub async fn start_on_port(&mut self, port: u16) -> Result<(), WorkerApiError> {
        if port == 0 {
            // Dynamic port assignment
            self.start_on_available_port().await
        } else {
            let addr = format!(":{port}");
            self.addr.clone_from(&addr);
            self.port = port;
            self.start_server(&addr).await
        }
    }

    /// Start the API server, finding an available port
    async fn start_on_available_port(&mut self) -> Result<(), WorkerApiError> {
        for port in MIN_PORT..MAX_PORT {
            let addr = format!(":{port}");
            match self.try_start_server(&addr).await {
                Ok(()) => {
                    self.addr.clone_from(&addr);
                    self.port = port;
                    return Ok(());
                }
                Err(WorkerApiError::AddressInUse) => {}
                Err(e) => return Err(e),
            }
        }
        Err(WorkerApiError::AddressInUse)
    }

    /// Try to start the server on the given address
    async fn try_start_server(&mut self, addr: &str) -> Result<(), WorkerApiError> {
        let addr_parsed: SocketAddr = addr
            .parse()
            .map_err(|e| WorkerApiError::BindError(format!("invalid address: {e}")))?;

        let listener = TcpListener::bind(addr_parsed).await.map_err(|e| {
            if e.to_string().contains("address already in use") {
                WorkerApiError::AddressInUse
            } else {
                WorkerApiError::BindError(e.to_string())
            }
        })?;

        self.port = listener
            .local_addr()
            .map_err(|e| WorkerApiError::ServerError(e.to_string()))?
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
    async fn start_server(&mut self, addr: &str) -> Result<(), WorkerApiError> {
        let addr_parsed: SocketAddr = addr
            .parse()
            .map_err(|e| WorkerApiError::BindError(format!("invalid address: {e}")))?;

        let listener = TcpListener::bind(addr_parsed)
            .await
            .map_err(|e| WorkerApiError::BindError(e.to_string()))?;

        self.port = listener
            .local_addr()
            .map_err(|e| WorkerApiError::ServerError(e.to_string()))?
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
    async fn wait_for_server(&self) -> Result<(), WorkerApiError> {
        let addr = format!("127.0.0.1:{}", self.port);
        let delay = Duration::from_millis(POLLING_DELAY_MS);

        for _ in 0..POLLING_MAX_ATTEMPTS {
            if std::net::TcpStream::connect(&addr).is_ok() {
                return Ok(());
            }
            tokio::time::sleep(delay).await;
        }

        Err(WorkerApiError::StartupTimeout)
    }

    /// Shutdown the API server gracefully
    ///
    /// # Errors
    ///
    /// Returns an error if the shutdown operation fails.
    #[allow(clippy::unused_async)]
    pub async fn shutdown(&self) -> Result<(), WorkerApiError> {
        // In this implementation, the server is spawned on a tokio task
        // Graceful shutdown would require storing the shutdown signal
        // For now, this is a placeholder
        Ok(())
    }
}

/// Perform health check implementation
pub(crate) async fn health_check_impl(
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
) -> HealthResponse {
    let broker_result = broker.health_check().await;
    let _datastore_result = datastore.health_check().await;
    let runtime_result = runtime.health_check().await;

    let status = determine_health_status(&broker_result, &runtime_result);
    let runtime_status = format_component_status(&runtime_result);
    let broker_status = format_component_status(&broker_result);

    HealthResponse {
        status,
        runtime: Some(runtime_status),
        broker: Some(broker_status),
    }
}

/// Determine overall health status from component results
fn determine_health_status(
    broker_result: &Result<(), anyhow::Error>,
    runtime_result: &Result<(), anyhow::Error>,
) -> HealthStatus {
    if broker_result.is_ok() && runtime_result.is_ok() {
        HealthStatus::Up
    } else {
        HealthStatus::Down
    }
}

/// Format a component health check result as a status string
fn format_component_status(result: &Result<(), anyhow::Error>) -> String {
    if result.is_ok() {
        String::from("ok")
    } else {
        format!(
            "error: {}",
            result
                .as_ref()
                .err()
                .map_or_else(|| "unknown".to_string(), ToString::to_string)
        )
    }
}

/// Create a new `WorkerApi` with the given broker and datastore
#[must_use]
pub fn new_api(
    broker: Arc<dyn Broker>,
    datastore: Arc<dyn Datastore>,
    runtime: Arc<dyn RuntimeTrait>,
) -> WorkerApi {
    WorkerApi::new(broker, datastore, runtime)
}
