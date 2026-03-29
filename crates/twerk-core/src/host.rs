//! Host utilities for system metrics.

use sysinfo::System;

/// Gets CPU usage percentage as a value between 0.0 and 100.0.
///
/// This function samples all CPUs and returns the average usage.
#[must_use]
pub fn get_cpu_percent() -> f64 {
    let mut sys = System::new_all();

    // Refresh CPU stats
    sys.refresh_cpu_all();

    // Get global CPU usage (average across all CPUs)
    // The Go code returns perc[0] for "total" CPU usage
    sys.global_cpu_usage() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cpu_percent_returns_valid_range() {
        let percent = get_cpu_percent();
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
        assert!(first >= 0.0 && first <= 100.0);
        assert!(second >= 0.0 && second <= 100.0);
    }
}
