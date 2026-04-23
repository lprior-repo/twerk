//! Type definitions for the Worker API.
//!
//! Contains health check types, error types, and configuration constants.

use serde::Serialize;
use thiserror::Error;

/// Default port range for dynamic port assignment
pub(crate) const MIN_PORT: u16 = 8001;
pub(crate) const MAX_PORT: u16 = 8100;

/// Polling configuration for server startup
pub(crate) const POLLING_MAX_ATTEMPTS: u32 = 100;
pub(crate) const POLLING_DELAY_MS: u64 = 100;

/// Health check result status
#[derive(Debug, Clone, Serialize)]
pub(crate) enum HealthStatus {
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
pub enum WorkerApiError {
    #[error("failed to bind to address: {0}")]
    BindError(String),

    #[error("address already in use")]
    AddressInUse,

    #[error("server error: {0}")]
    ServerError(String),

    #[error("timeout waiting for server to start")]
    StartupTimeout,
}
