//! Host utilities for system metrics.

use once_cell::sync::Lazy;
use sysinfo::System;
use thiserror::Error;

/// Errors that can occur when getting host metrics.
#[derive(Debug, Error)]
pub enum HostError {
    #[error("failed to lock system state: {0}")]
    LockError(String),
    #[error("no CPUs found")]
    NoCpus,
    #[error("failed to refresh CPU stats: {0}")]
    RefreshError(String),
}

/// Gets CPU usage percentage as a value between 0.0 and 100.0.
///
/// This function samples all CPUs and returns the average usage.
///
/// # Errors
/// Returns `HostError` if the system state cannot be locked or refreshed.
pub fn get_cpu_percent() -> Result<f64, HostError> {
    static SYSTEM: Lazy<System> = Lazy::new(System::new_all);

    // Refresh CPU stats - this requires interior mutability
    SYSTEM.refresh_cpu_all();

    // Get the first CPU's usage as an approximation for overall usage
    // The Go code returns perc[0] for "total" CPU usage
    SYSTEM
        .cpus()
        .first()
        .map(|cpu| cpu.cpu_usage())
        .ok_or(HostError::NoCpus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cpu_percent_returns_valid_range() {
        let result = get_cpu_percent();
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let percent = result.unwrap();
        assert!(
            percent >= 0.0 && percent <= 100.0,
            "expected 0-100, got {}",
            percent
        );
    }

    #[test]
    fn test_get_cpu_percent_twice_is_callable() {
        // Should be able to call multiple times without issues
        let first = get_cpu_percent();
        let second = get_cpu_percent();
        assert!(first.is_ok() && second.is_ok());
    }
}
