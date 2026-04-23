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
    f64::from(sys.global_cpu_usage())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cpu_percent_returns_valid_range() {
        let percent = get_cpu_percent();
        assert!(
            (0.0..=100.0).contains(&percent),
            "expected 0-100, got {percent}"
        );
    }

    #[test]
    fn test_get_cpu_percent_twice_is_callable() {
        // Should be able to call multiple times without issues
        let first = get_cpu_percent();
        let second = get_cpu_percent();
        assert!((0.0..=100.0).contains(&first));
        assert!((0.0..=100.0).contains(&second));
    }

    #[test]
    fn test_get_cpu_percent_returns_finite_value() {
        // Verifies the function returns a real f64 from sysinfo, not NaN or infinity.
        // This exercises the f64::from(sys.global_cpu_usage()) code path.
        let percent = get_cpu_percent();
        assert!(percent.is_finite(), "expected finite f64, got {percent}");
        // Verify the value is actually in the [0, 100] range
        assert!((0.0..=100.0).contains(&percent));
    }
}
