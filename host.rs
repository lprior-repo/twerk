//! # Host Module
//!
//! Provides host system information retrieval functionality.

use thiserror::Error;

/// Errors that can occur during host operations.
#[derive(Debug, Error)]
pub enum HostError {
    #[error("failed to get CPU percentage: {0}")]
    CpuPercentError(String),
}

/// Gets the current CPU usage percentage.
///
/// This function queries the system for CPU usage and returns the percentage
/// of CPU currently in use. Returns 0.0 if the query fails.
///
/// # Returns
///
/// The CPU usage percentage as a f64, or 0.0 on error
pub fn get_cpu_percent() -> f64 {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_cpu_all();
    // Wait a bit to get accurate reading
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0.0;
    }

    let total: f64 = cpus.iter().map(|cpu| cpu.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64;
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cpu_percent() {
        let cpu_percent = get_cpu_percent();
        assert!(cpu_percent >= 0.0);
    }
}
