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
    ApiError, HealthResponse, HealthStatus, MAX_PORT, MIN_PORT, POLLING_DELAY_MS,
    POLLING_MAX_ATTEMPTS,
};

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

        let listener = TcpListener::bind(addr_parsed).await.map_err(|e| {
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
pub(crate) async fn health_check_impl(
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
