//! Pure helper functions for Docker runtime.

use std::time::Duration;
use tar::Archive as TarArchive;

use crate::runtime::docker::error::DockerError;

/// Constructs a Docker port key string (e.g., "8080/tcp").
#[must_use]
pub fn port_key(port: u64) -> String {
    format!("{port}/tcp")
}

/// Parses a Go-style duration string (e.g., "5s", "10m", "1h").
///
/// # Errors
///
/// Returns a String error if the duration format is invalid.
pub fn parse_go_duration(input: &str) -> Result<Duration, String> {
    let mut total = Duration::ZERO;
    let mut current = String::new();
    for c in input.chars() {
        if c.is_ascii_digit() || c == '.' {
            current.push(c);
        } else {
            let num: f64 = current
                .parse()
                .map_err(|e| format!("invalid duration '{current}': {e}"))?;
            total += match c {
                'h' => Duration::from_secs_f64(num * 3600.0),
                'm' => Duration::from_secs_f64(num * 60.0),
                's' => Duration::from_secs_f64(num),
                _ => return Err(format!("unknown unit: {c}")),
            };
            current.clear();
        }
    }
    if !current.is_empty() {
        return Err(format!("trailing: {current}"));
    }
    Ok(total)
}

/// Parses a memory size string with units (e.g., "1GB", "512MB", "1TB").
///
/// # Errors
///
/// Returns a String error if the format is invalid.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn parse_memory_bytes(input: &str) -> Result<i64, String> {
    let input = input.trim();
    let (num_str, multiplier) = if let Some(s) = input
        .strip_suffix("TB")
        .or_else(|| input.strip_suffix("tb"))
    {
        (s.trim(), 1_099_511_627_776_i64)
    } else if let Some(s) = input
        .strip_suffix("GB")
        .or_else(|| input.strip_suffix("gb"))
    {
        (s.trim(), 1_073_741_824_i64)
    } else if let Some(s) = input
        .strip_suffix("MB")
        .or_else(|| input.strip_suffix("mb"))
    {
        (s.trim(), 1_048_576_i64)
    } else if let Some(s) = input
        .strip_suffix("KB")
        .or_else(|| input.strip_suffix("kb"))
    {
        (s.trim(), 1024_i64)
    } else if let Some(s) = input.strip_suffix("B").or_else(|| input.strip_suffix("b")) {
        (s.trim(), 1_i64)
    } else {
        return input
            .parse::<i64>()
            .map_err(|e| format!("cannot parse '{input}': {e}"));
    };

    let num = num_str
        .parse::<f64>()
        .map_err(|e| format!("cannot parse '{num_str}': {e}"))?;
    Ok((num * multiplier as f64) as i64)
}

/// Parses tar archive contents and returns the first file as a String.
#[must_use]
pub fn parse_tar_contents(tar_bytes: &[u8]) -> String {
    let mut archive = TarArchive::new(tar_bytes);
    let Ok(entries) = archive.entries() else {
        return String::new();
    };
    for entry in entries {
        let Ok(mut entry) = entry else {
            continue;
        };
        let mut buf = Vec::new();
        if std::io::Read::read_to_end(&mut entry, &mut buf).is_ok() {
            if let Ok(s) = String::from_utf8(buf) {
                return s;
            }
        }
    }
    String::new()
}

/// Parses GPU options string into `DeviceRequest` configuration.
///
/// # Errors
///
/// Returns `DockerError::InvalidGpuOptions` if the GPU options are malformed.
pub fn parse_gpu_options(
    gpu_str: &str,
) -> Result<Vec<bollard::models::DeviceRequest>, DockerError> {
    use bollard::models::DeviceRequest;

    let mut count: Option<i64> = None;
    let mut driver: Option<String> = None;
    let mut capabilities: Vec<String> = Vec::new();
    let mut device_ids: Vec<String> = Vec::new();

    for part in gpu_str.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            match key.trim() {
                "count" => {
                    count = if value.trim() == "all" {
                        Some(-1)
                    } else {
                        Some(value.trim().parse::<i64>().map_err(|_| {
                            DockerError::InvalidGpuOptions(format!("invalid count: {value}"))
                        })?)
                    };
                }
                "driver" => {
                    driver = Some(value.trim().to_string());
                }
                "capabilities" => {
                    for cap in value.split(';') {
                        capabilities.push(cap.trim().to_string());
                    }
                }
                "device" => {
                    for dev in value.split(';') {
                        device_ids.push(dev.trim().to_string());
                    }
                }
                other => {
                    return Err(DockerError::InvalidGpuOptions(format!(
                        "unknown GPU option: {other}"
                    )));
                }
            }
        }
    }

    if capabilities.is_empty() {
        capabilities.push("gpu".to_string());
    }

    Ok(vec![DeviceRequest {
        count,
        driver,
        capabilities: Some(vec![capabilities]),
        device_ids: if device_ids.is_empty() {
            None
        } else {
            Some(device_ids)
        },
        options: None,
    }])
}

#[must_use]
pub fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
