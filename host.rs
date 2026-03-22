//! # Host Module
//!
//! Provides host system information retrieval functionality.
//! Parity with Go's `internal/host/host.go`: `GetCPUPercent` returns 0.0
//! on error, matching Go's behavior of logging debug and returning 0.

/// Gets the current CPU usage percentage.
///
/// This function queries the system for CPU usage and returns the percentage
/// of CPU currently in use. Returns 0.0 if the query fails, matching Go's
/// `GetCPUPercent` which returns 0 on error.
///
/// # Returns
///
/// The CPU usage percentage as a f64, or 0.0 on error
pub fn get_cpu_percent() -> f64 {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_cpu_all();
    // Small delay required by sysinfo to measure CPU delta between two samples.
    // Go's gopsutil handles this internally via /proc/stat timestamps.
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0.0;
    }

    cpus.iter().map(|cpu| cpu.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
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
