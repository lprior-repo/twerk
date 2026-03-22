//! Health check module for Tork
//!
//! Provides health checking functionality with pluggable indicators.

use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;
use tork::version::VERSION;

/// Health status constants
pub const STATUS_UP: &str = "UP";
/// Status indicating the service is down
pub const STATUS_DOWN: &str = "DOWN";

/// Service names for health indicators
pub mod service {
    /// Datastore service identifier
    pub const DATASTORE: &str = "datastore";
    /// Broker service identifier
    pub const BROKER: &str = "broker";
    /// Runtime service identifier
    pub const RUNTIME: &str = "runtime";
}
/// Error types for health checks
#[derive(Debug, Error)]
pub enum HealthError {
    #[error("health check failed: {0}")]
    CheckFailed(String),
    #[error("health check timed out")]
    Timeout,
}

/// Context for health check operations.
///
/// Provides cancellation support for health indicators.
pub trait HealthContext: Send + Sync {
    /// Returns a channel that's closed when the health check should be cancelled.
    fn done(&self) -> Option<oneshot::Receiver<()>>;
}

/// No-op health context that never cancels.
impl HealthContext for () {
    fn done(&self) -> Option<oneshot::Receiver<()>> {
        None
    }
}

/// Health indicator function type
pub type HealthIndicator =
    Box<dyn Fn(&dyn HealthContext) -> Result<(), HealthError> + Send + Sync>;

/// Default timeout for health checks
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of a health check
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckResult {
    /// Current health status
    pub status: String,
    /// Version of the system
    pub version: String,
}

impl HealthCheckResult {
    /// Creates a new health check result with the given status
    #[must_use]
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            version: VERSION.to_string(),
        }
    }

    /// Returns true if the health check indicates the service is up
    #[must_use]
    pub fn is_up(&self) -> bool {
        self.status == STATUS_UP
    }
}

/// HealthCheck holds registered health indicators
pub struct HealthCheck {
    indicators: HashMap<String, HealthIndicator>,
}

impl Default for HealthCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheck {
    /// Creates a new empty health check registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            indicators: HashMap::new(),
        }
    }

    /// Registers a health indicator with the given name
    ///
    /// Returns an error if the name is empty or already registered
    pub fn with_indicator(mut self, name: &str, ind: HealthIndicator) -> Result<Self, HealthError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(HealthError::CheckFailed(
                "health indicator name must not be empty".to_string(),
            ));
        }
        if self.indicators.contains_key(name) {
            return Err(HealthError::CheckFailed(format!(
                "health indicator with name {} already exists",
                name
            )));
        }
        self.indicators.insert(name.to_string(), ind);
        Ok(self)
    }

    /// Adds a health indicator, panics on error (for testing only)
    #[cfg(test)]
    pub fn with_indicator_panic(self, name: &str, ind: HealthIndicator) -> Self {
        self.with_indicator(name, ind)
            .expect("failed to add health indicator")
    }

    /// Performs all health checks
    ///
    /// Returns `STATUS_UP` if all indicators pass, otherwise returns `STATUS_DOWN`
    #[must_use]
    pub fn do_check(&self, ctx: &dyn HealthContext) -> HealthCheckResult {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.do_check_async(ctx))
    }

    /// Async version of health check
    async fn do_check_async(&self, ctx: &dyn HealthContext) -> HealthCheckResult {
        for (name, ind) in &self.indicators {
            let ind = ind.as_ref();
            let result = tokio::time::timeout(
                DEFAULT_TIMEOUT,
                async { ind(ctx) },
            )
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {
                    // In a real implementation, we would log here
                    // For functional style, we return immediately on first failure
                    let _ = name;
                    return HealthCheckResult::new(STATUS_DOWN);
                }
                Err(_) => {
                    // Timeout
                    let _ = name;
                    return HealthCheckResult::new(STATUS_DOWN);
                }
            }
        }
        HealthCheckResult::new(STATUS_UP)
    }

    /// Returns the number of registered indicators
    #[must_use]
    pub fn len(&self) -> usize {
        self.indicators.len()
    }

    /// Returns true if there are no registered indicators
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indicators.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_ok() {
        let ind: HealthIndicator = Box::new(|_ctx: &dyn HealthContext| Ok(()));
        let result = HealthCheck::new()
            .with_indicator_panic("test", ind)
            .do_check(&());
        assert_eq!(result.status, STATUS_UP);
        assert!(result.is_up());
    }

    #[test]
    fn test_health_check_failed() {
        let ind: HealthIndicator = Box::new(|_ctx: &dyn HealthContext| {
            Err(HealthError::CheckFailed("something happened".to_string()))
        });
        let result = HealthCheck::new()
            .with_indicator_panic("test", ind)
            .do_check(&());
        assert_eq!(result.status, STATUS_DOWN);
        assert!(!result.is_up());
    }

    #[test]
    fn test_health_check_empty_name() {
        let ind: HealthIndicator = Box::new(|_ctx: &dyn HealthContext| Ok(()));
        let result = HealthCheck::new().with_indicator("", ind);
        assert!(result.is_err());
    }

    #[test]
    fn test_health_check_duplicate_name() {
        let ind: HealthIndicator = Box::new(|_ctx: &dyn HealthContext| Ok(()));
        let health = HealthCheck::new().with_indicator_panic("test", ind);
        let ind2: HealthIndicator = Box::new(|_ctx: &dyn HealthContext| Ok(()));
        let result = health.with_indicator("test", ind2);
        assert!(result.is_err());
    }
}
