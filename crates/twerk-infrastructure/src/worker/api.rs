//! Worker API module.
//!
//! Provides HTTP API for worker health checks and status.

mod server;
#[cfg(test)]
mod tests;
mod types;

pub use server::{new_api, WorkerApi};
#[allow(unused_imports)]
pub use types::{ApiError, HealthResponse, HealthStatus};
