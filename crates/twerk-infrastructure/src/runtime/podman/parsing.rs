//! Parsing utilities for Podman runtime

use std::time::Duration;

use super::errors::PodmanError;
use super::types::PodmanRuntime;

#[allow(dead_code)]
impl PodmanRuntime {
    /// Parse CPU limits from string to f64
    pub(crate) fn parse_cpus(cpus: &str) -> Result<f64, PodmanError> {
        let nanos: f64 = cpus.parse().map_err(|e| {
            PodmanError::InvalidCpusLimit(format!("failed to parse '{}' as CPU limit: {}", cpus, e))
        })?;
        if nanos < 0.0 {
            return Err(PodmanError::InvalidCpusLimit(
                "CPU limit must be non-negative".to_string(),
            ));
        }
        Ok(nanos)
    }

    /// Parse memory limits from string to bytes (u64)
    pub(crate) fn parse_memory(memory: &str) -> Result<u64, PodmanError> {
        let memory = memory.trim();

        let (num_str, multiplier) = if let Some(suffix) = memory.strip_suffix("gb") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("g") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("mb") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("m") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("kb") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("k") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("b") {
            (suffix.trim_end(), 1u64)
        } else {
            (memory, 1u64)
        };

        let value: f64 = num_str.parse().map_err(|e| {
            PodmanError::InvalidMemoryLimit(format!(
                "failed to parse '{}' as memory limit: {}",
                memory, e
            ))
        })?;

        Ok((value * multiplier as f64) as u64)
    }

    /// Parse duration string (e.g., "1h", "30m", "45s")
    pub(crate) fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        let (num_str, suffix) = if let Some(rest) = s.strip_suffix("h") {
            (rest, 'h')
        } else if let Some(rest) = s.strip_suffix("m") {
            (rest, 'm')
        } else if let Some(rest) = s.strip_suffix("s") {
            (rest, 's')
        } else {
            return Err(format!("invalid duration: {}", s));
        };

        let value: u64 = num_str
            .parse()
            .map_err(|e| format!("invalid duration number '{}': {}", num_str, e))?;

        Ok(match suffix {
            'h' => Duration::from_secs(value * 3600),
            'm' => Duration::from_secs(value * 60),
            's' => Duration::from_secs(value),
            _ => return Err(format!("unknown duration suffix: {}", suffix)),
        })
    }
}
