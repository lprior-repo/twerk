//! Health check module for Tork
//!
//! Provides health checking functionality with pluggable indicators.

use std::collections::HashMap;
use thiserror::Error;
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

/// Health indicator function type.
///
/// A synchronous check that returns `Ok(())` on success or `Err(HealthError)` on failure.
/// Matches the Go `HealthIndicator func(ctx context.Context) error` signature.
pub type HealthIndicator = Box<dyn Fn() -> Result<(), HealthError> + Send + Sync>;

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

    /// Registers a health indicator with the given name.
    ///
    /// Returns an error if the name is empty or already registered.
    /// Builds a new map to avoid `mut self`.
    pub fn with_indicator(self, name: &str, ind: HealthIndicator) -> Result<Self, HealthError> {
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
        let new_indicators = self
            .indicators
            .into_iter()
            .chain(std::iter::once((name.to_string(), ind)))
            .collect();
        Ok(Self {
            indicators: new_indicators,
        })
    }

    /// Adds a health indicator, panics on error (for testing only).
    #[cfg(test)]
    pub fn with_indicator_panic(self, name: &str, ind: HealthIndicator) -> Self {
        match self.with_indicator(name, ind) {
            Ok(health) => health,
            Err(e) => panic!("failed to add health indicator: {e}"),
        }
    }

    /// Performs all health checks synchronously.
    ///
    /// Returns `STATUS_UP` if all indicators pass, otherwise returns `STATUS_DOWN`.
    /// Short-circuits on the first failure (matches Go behavior).
    #[must_use]
    pub fn do_check(&self) -> HealthCheckResult {
        let all_pass = self.indicators.values().all(|ind| ind().is_ok());
        if all_pass {
            HealthCheckResult::new(STATUS_UP)
        } else {
            HealthCheckResult::new(STATUS_DOWN)
        }
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
        let ind: HealthIndicator = Box::new(|| Ok(()));
        let result = HealthCheck::new()
            .with_indicator_panic("test", ind)
            .do_check();
        assert_eq!(result.status, STATUS_UP);
        assert!(result.is_up());
    }

    #[test]
    fn test_health_check_failed() {
        let ind: HealthIndicator =
            Box::new(|| Err(HealthError::CheckFailed("something happened".to_string())));
        let result = HealthCheck::new()
            .with_indicator_panic("test", ind)
            .do_check();
        assert_eq!(result.status, STATUS_DOWN);
        assert!(!result.is_up());
    }

    #[test]
    fn test_health_check_empty_name() {
        let ind: HealthIndicator = Box::new(|| Ok(()));
        let result = HealthCheck::new().with_indicator("", ind);
        assert!(result.is_err());
    }

    #[test]
    fn test_health_check_duplicate_name() {
        let ind: HealthIndicator = Box::new(|| Ok(()));
        let health = HealthCheck::new().with_indicator_panic("test", ind);
        let ind2: HealthIndicator = Box::new(|| Ok(()));
        let result = health.with_indicator("test", ind2);
        assert!(result.is_err());
    }

    #[test]
    fn test_health_check_multiple_indicators_all_pass() {
        let ind1: HealthIndicator = Box::new(|| Ok(()));
        let ind2: HealthIndicator = Box::new(|| Ok(()));
        let result = HealthCheck::new()
            .with_indicator_panic("datastore", ind1)
            .with_indicator_panic("broker", ind2)
            .do_check();
        assert_eq!(result.status, STATUS_UP);
    }

    #[test]
    fn test_health_check_multiple_indicators_one_fails() {
        let ind1: HealthIndicator = Box::new(|| Ok(()));
        let ind2: HealthIndicator =
            Box::new(|| Err(HealthError::CheckFailed("broker down".to_string())));
        let result = HealthCheck::new()
            .with_indicator_panic("datastore", ind1)
            .with_indicator_panic("broker", ind2)
            .do_check();
        assert_eq!(result.status, STATUS_DOWN);
    }

    #[test]
    fn test_health_check_result_version() {
        let result = HealthCheckResult::new(STATUS_UP);
        assert_eq!(result.version, VERSION);
    }
}
