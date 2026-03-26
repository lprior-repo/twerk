// ----------------------------------------------------------------------------
// Container Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // parse_memory_bytes — Go parity: units.RAMInBytes
    // =========================================================================

    #[test]
    fn parse_memory_bytes_bytes() {
        assert_eq!(1, parse_memory_bytes("1B").unwrap());
        assert_eq!(10, parse_memory_bytes("10B").unwrap());
        assert_eq!(512, parse_memory_bytes("512B").unwrap());
    }

    #[test]
    fn parse_memory_bytes_lowercase_b() {
        assert_eq!(1, parse_memory_bytes("1b").unwrap());
        assert_eq!(42, parse_memory_bytes("42b").unwrap());
    }

    #[test]
    fn parse_memory_bytes_kilobytes() {
        assert_eq!(1024, parse_memory_bytes("1KB").unwrap());
        assert_eq!(512_000, parse_memory_bytes("500KB").unwrap());
        assert_eq!(1024, parse_memory_bytes("1kb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_megabytes() {
        assert_eq!(1_048_576, parse_memory_bytes("1MB").unwrap());
        assert_eq!(10_485_760, parse_memory_bytes("10MB").unwrap());
        assert_eq!(524_288_000, parse_memory_bytes("500MB").unwrap());
        // lowercase
        assert_eq!(1_048_576, parse_memory_bytes("1mb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_gigabytes() {
        assert_eq!(1_073_741_824, parse_memory_bytes("1GB").unwrap());
        assert_eq!(2_147_483_648, parse_memory_bytes("2GB").unwrap());
        // lowercase
        assert_eq!(1_073_741_824, parse_memory_bytes("1gb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_terabytes() {
        assert_eq!(1_099_511_627_776, parse_memory_bytes("1TB").unwrap());
        assert_eq!(2_199_023_255_552, parse_memory_bytes("2TB").unwrap());
        // lowercase
        assert_eq!(1_099_511_627_776, parse_memory_bytes("1tb").unwrap());
    }

    #[test]
    fn parse_memory_bytes_whitespace_tolerance() {
        assert_eq!(1_048_576, parse_memory_bytes(" 1MB ").unwrap());
        assert_eq!(1024, parse_memory_bytes(" 1 KB ").unwrap());
        assert_eq!(1, parse_memory_bytes(" 1B ").unwrap());
    }

    #[test]
    fn parse_memory_bytes_invalid_string() {
        assert!(parse_memory_bytes("invalid").is_err());
        assert!(parse_memory_bytes("").is_err());
        assert!(parse_memory_bytes("B").is_err());
        assert!(parse_memory_bytes("KB").is_err());
        assert!(parse_memory_bytes("MB").is_err());
    }

    #[test]
    fn parse_memory_bytes_negative_is_ok() {
        // The implementation parses -1B as f64(-1.0) * 1 = -1
        // This is technically allowed by the parser (Go parity may differ)
        assert_eq!(-1, parse_memory_bytes("-1B").unwrap());
    }

    #[test]
    fn parse_memory_bytes_fractional_ok() {
        // 0.5 MB = 524288
        let result = parse_memory_bytes("0.5MB").unwrap();
        assert_eq!(524_288, result);
    }

    #[test]
    fn parse_memory_bytes_bare_number() {
        // No suffix = raw bytes
        assert_eq!(1024, parse_memory_bytes("1024").unwrap());
    }

    // =========================================================================
    // parse_limits — Go parity: parseCPUs + parseMemory
    // =========================================================================

    #[test]
    fn parse_limits_none_returns_none_tuple() {
        let result = DockerRuntime::parse_limits(None).unwrap();
        assert_eq!((None, None), result);
    }

    #[test]
    fn parse_limits_empty_cpus_and_memory() {
        let limits = TaskLimits::new(Some(""), Some(""));
        let result = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!((None, None), result);
    }

    #[test]
    fn parse_limits_cpu_integer() {
        let limits = TaskLimits::new(Some("1"), None);
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(1_000_000_000), cpus);
        assert_eq!(None, mem);
    }

    #[test]
    fn parse_limits_cpu_two_cores() {
        let limits = TaskLimits::new(Some("2"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(2_000_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_half() {
        let limits = TaskLimits::new(Some("0.5"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(500_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_quarter() {
        let limits = TaskLimits::new(Some(".25"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(250_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_small_fraction() {
        let limits = TaskLimits::new(Some("0.125"), None);
        let (cpus, _) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(125_000_000), cpus);
    }

    #[test]
    fn parse_limits_cpu_invalid_string() {
        let limits = TaskLimits::new(Some("abc"), None);
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CPUs"), "error should mention CPUs: {err}");
    }

    #[test]
    fn parse_limits_memory_1g() {
        let limits = TaskLimits::new(None, Some("1GB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(None, cpus);
        assert_eq!(Some(1_073_741_824), mem);
    }

    #[test]
    fn parse_limits_memory_512m() {
        let limits = TaskLimits::new(None, Some("512MB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(None, cpus);
        assert_eq!(Some(536_870_912), mem);
    }

    #[test]
    fn parse_limits_memory_256mb_lowercase() {
        let limits = TaskLimits::new(None, Some("256mb"));
        let (_cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(268_435_456), mem);
    }

    #[test]
    fn parse_limits_memory_1g_abbreviation() {
        // "1g" is NOT a recognized suffix (only GB/gb, not G/g alone).
        // Falls through to bare number parse, which fails on "1g".
        let limits = TaskLimits::new(None, Some("1g"));
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(
            result.is_err(),
            "\"1g\" should not parse — only GB/gb is valid"
        );
    }

    #[test]
    fn parse_limits_memory_invalid_string() {
        let limits = TaskLimits::new(None, Some("not-a-size"));
        let result = DockerRuntime::parse_limits(Some(&limits));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("memory"), "error should mention memory: {err}");
    }

    #[test]
    fn parse_limits_both_cpu_and_memory() {
        let limits = TaskLimits::new(Some("2"), Some("1GB"));
        let (cpus, mem) = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!(Some(2_000_000_000), cpus);
        assert_eq!(Some(1_073_741_824), mem);
    }

    #[test]
    fn parse_limits_default_limits() {
        // Default TaskLimits has None for both fields
        let limits = TaskLimits::default();
        let result = DockerRuntime::parse_limits(Some(&limits)).unwrap();
        assert_eq!((None, None), result);
    }

    // =========================================================================
    // parse_gpu_options — Go parity: cliopts.GpuOpts.Set
    // =========================================================================

    #[test]
    fn parse_gpu_options_count_numeric() {
        let reqs = DockerRuntime::parse_gpu_options("count=2").unwrap();
        assert_eq!(1, reqs.len());
        assert_eq!(Some(2), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_count_all() {
        let reqs = DockerRuntime::parse_gpu_options("count=all").unwrap();
        assert_eq!(Some(-1), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_count_one() {
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        assert_eq!(Some(1), reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_default_capabilities() {
        // When no capabilities specified, should default to [["gpu"]]
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_explicit_capabilities() {
        let reqs = DockerRuntime::parse_gpu_options("capabilities=gpu;compute").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string(), "compute".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_single_capability() {
        let reqs = DockerRuntime::parse_gpu_options("capabilities=utility").unwrap();
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["utility".to_string()], &caps[0]);
    }

    #[test]
    fn parse_gpu_options_driver() {
        let reqs = DockerRuntime::parse_gpu_options("driver=nvidia").unwrap();
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
    }

    #[test]
    fn parse_gpu_options_device_ids() {
        let reqs = DockerRuntime::parse_gpu_options("device=0;1").unwrap();
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string(), "1".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_single_device() {
        let reqs = DockerRuntime::parse_gpu_options("device=0").unwrap();
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_full_spec() {
        let reqs = DockerRuntime::parse_gpu_options(
            "count=2,driver=nvidia,capabilities=gpu;compute,device=0;1",
        )
        .unwrap();
        assert_eq!(Some(2), reqs[0].count);
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
        let caps = reqs[0].capabilities.as_ref().unwrap();
        assert_eq!(1, caps.len());
        assert_eq!(&vec!["gpu".to_string(), "compute".to_string()], &caps[0]);
        let ids = reqs[0].device_ids.as_ref().unwrap();
        assert_eq!(&["0".to_string(), "1".to_string()], ids.as_slice());
    }

    #[test]
    fn parse_gpu_options_whitespace_tolerance() {
        let reqs = DockerRuntime::parse_gpu_options(" count = 2 , driver = nvidia ").unwrap();
        assert_eq!(Some(2), reqs[0].count);
        assert_eq!(Some("nvidia".to_string()), reqs[0].driver);
    }

    #[test]
    fn parse_gpu_options_empty_string() {
        let reqs = DockerRuntime::parse_gpu_options("").unwrap();
        assert_eq!(1, reqs.len());
        // count should be None, default capabilities
        assert_eq!(None, reqs[0].count);
    }

    #[test]
    fn parse_gpu_options_invalid_count() {
        let result = DockerRuntime::parse_gpu_options("count=notanumber");
        assert!(result.is_err());
    }

    #[test]
    fn parse_gpu_options_unknown_key() {
        let result = DockerRuntime::parse_gpu_options("foo=bar");
        assert!(result.is_err());
    }

    #[test]
    fn parse_gpu_options_no_device_ids_field() {
        let reqs = DockerRuntime::parse_gpu_options("count=1").unwrap();
        assert!(reqs[0].device_ids.is_none());
    }
}
