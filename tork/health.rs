//! Health check system for Tork.
//!
//! Port of Go `/tmp/tork-go/health/health.go`
//!
//! # Architecture
//!
//! This module provides a health check system with registered indicators.
//! Each indicator is a function that returns `Result<()>` - `Ok(())` means
//! the component is healthy, `Err(...)` means it's unhealthy.

use std::collections::HashMap;
use std::sync::Arc;

/// Health indicator function type.
/// Returns `Ok(())` if healthy, `Err(description)` if unhealthy.
pub type HealthIndicator = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// Health check result returned by `HealthCheck::do_check()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheckResult {
    /// Health status - either "UP" or "DOWN"
    pub status: String,
    /// Version of the tork system
    pub version: String,
}

/// Health check system that runs registered indicators.
#[derive(Default)]
pub struct HealthCheck {
    indicators: HashMap<String, HealthIndicator>,
}

impl HealthCheck {
    /// Create a new empty health check.
    #[must_use]
    pub fn new() -> Self {
        Self {
            indicators: HashMap::new(),
        }
    }

    /// Register a health indicator.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty or already registered.
    #[must_use]
    pub fn with_indicator(mut self, name: &str, ind: HealthIndicator) -> Self {
        let name = name.trim();
        if name.is_empty() {
            panic!("health indicator name must not be empty");
        }
        if self.indicators.contains_key(name) {
            panic!("health indicator with name {} already exists", name);
        }
        self.indicators.insert(name.to_string(), ind);
        self
    }

    /// Run all health indicators.
    ///
    /// Returns `HealthCheckResult` with status "UP" if all indicators pass,
    /// or "DOWN" on the first failure.
    #[must_use]
    pub fn do_check(&self) -> HealthCheckResult {
        for (name, ind) in &self.indicators {
            if let Err(e) = ind() {
                tracing::error!(error = %e, name, "health check failed");
                return HealthCheckResult {
                    status: STATUS_DOWN.to_string(),
                    version: crate::version::VERSION.to_string(),
                };
            }
        }
        HealthCheckResult {
            status: STATUS_UP.to_string(),
            version: crate::version::VERSION.to_string(),
        }
    }
}

/// Health status constant for "UP"
pub const STATUS_UP: &str = "UP";
/// Health status constant for "DOWN"
pub const STATUS_DOWN: &str = "DOWN";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_all_healthy() {
        let health = HealthCheck::new()
            .with_indicator("test1", Arc::new(|| Ok(())))
            .with_indicator("test2", Arc::new(|| Ok(())));

        let result = health.do_check();
        assert_eq!(result.status, STATUS_UP);
    }

    #[test]
    fn test_health_check_one_fails() {
        let health = HealthCheck::new()
            .with_indicator("healthy", Arc::new(|| Ok(())))
            .with_indicator("failing", Arc::new(|| Err("something broke".to_string())));

        let result = health.do_check();
        assert_eq!(result.status, STATUS_DOWN);
    }

    #[test]
    fn test_empty_health_check() {
        let health = HealthCheck::new();
        let result = health.do_check();
        assert_eq!(result.status, STATUS_UP);
    }

    #[test]
    #[should_panic(expected = "health indicator name must not be empty")]
    fn test_empty_name_panics() {
        let _ = HealthCheck::new().with_indicator("", Arc::new(|| Ok(())));
    }

    #[test]
    #[should_panic(expected = "health indicator with name")]
    fn test_duplicate_name_panics() {
        let ind = Arc::new(|| Ok(()));
        let _ = HealthCheck::new()
            .with_indicator("dup", ind.clone())
            .with_indicator("dup", ind);
    }
}
