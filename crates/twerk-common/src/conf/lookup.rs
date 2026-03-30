//! Configuration lookup functions.

use std::collections::HashMap;

use time::Duration;

use super::types::{ConfigError, ConfigState, WorkerLimits};
use super::CONFIG;

/// Global config state accessor.
fn get_config() -> Result<ConfigState, ConfigError> {
    let guard = CONFIG
        .read()
        .map_err(|_| ConfigError::KeyNotFound("config poisoned".to_string()))?;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| ConfigError::KeyNotFound("config not loaded".to_string()))
}

/// Get a string configuration value.
#[must_use]
pub fn string(key: &str) -> String {
    get_config()
        .ok()
        .and_then(|c| c.get_str(key).map(|v| v.to_string()))
        .unwrap_or_default()
}

/// Get a string configuration value with a default.
#[must_use]
pub fn string_default(key: &str, default: &str) -> String {
    let s = string(key);
    if s.is_empty() {
        default.to_string()
    } else {
        s
    }
}

/// Get a boolean configuration value.
#[must_use]
pub fn bool(key: &str) -> bool {
    get_config()
        .ok()
        .and_then(|c| c.get_bool(key))
        .is_some_and(|v| v)
}

/// Get a boolean configuration value with a default.
#[must_use]
pub fn bool_default(key: &str, default: bool) -> bool {
    get_config()
        .ok()
        .and_then(|c| {
            c.get_bool(key)
                .or_else(|| c.contains_key(key).then_some(default))
        })
        .unwrap_or(default)
}

/// Get an integer configuration value.
#[must_use]
pub fn int(key: &str) -> i64 {
    get_config()
        .ok()
        .and_then(|c| c.get_int(key))
        .map_or(0, |v| v)
}

/// Get an integer configuration value with a default.
#[must_use]
pub fn int_default(key: &str, default: i64) -> i64 {
    get_config()
        .ok()
        .and_then(|c| {
            c.get_int(key)
                .or_else(|| c.contains_key(key).then_some(default))
        })
        .unwrap_or(default)
}

/// Get a string-to-integer map configuration.
#[must_use]
pub fn int_map(key: &str) -> HashMap<String, i64> {
    get_config()
        .map(|c| c.int_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a string-to-boolean map configuration.
#[must_use]
pub fn bool_map(key: &str) -> HashMap<String, bool> {
    get_config()
        .map(|c| c.bool_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a string-to-string map configuration.
#[must_use]
pub fn string_map(key: &str) -> HashMap<String, String> {
    get_config()
        .map(|c| c.string_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a list of strings configuration.
#[must_use]
pub fn strings(key: &str) -> Vec<String> {
    get_config()
        .map(|c| c.strings_for_key_or_string(key))
        .unwrap_or_else(|_| Vec::new())
}

/// Get a list of strings with a default fallback.
#[must_use]
pub fn strings_default(key: &str, default: &[&str]) -> Vec<String> {
    let v = strings(key);
    if v.is_empty() {
        default.iter().map(ToString::to_string).collect()
    } else {
        v
    }
}

/// Parse a duration string (e.g., "1h30m", "30s", "1d").
#[must_use]
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    parse_single_duration(s).or_else(|| parse_complex_duration(s))
}

/// Parse a simple single-unit duration.
#[must_use]
fn parse_single_duration(s: &str) -> Option<Duration> {
    if s.ends_with("ns") {
        parse_single_duration_with_value(s.trim_end_matches("ns"), "ns")
    } else if s.ends_with("us") || s.ends_with("µs") {
        let val = if s.ends_with("us") {
            s.trim_end_matches("us")
        } else {
            s.trim_end_matches("µs")
        };
        parse_single_duration_with_value(val, "us")
    } else if s.ends_with("ms") {
        parse_single_duration_with_value(s.trim_end_matches("ms"), "ms")
    } else if s.ends_with('s') {
        parse_single_duration_with_value(s.trim_end_matches('s'), "s")
    } else if s.ends_with('m') {
        parse_single_duration_with_value(s.trim_end_matches('m'), "m")
    } else if s.ends_with('h') {
        parse_single_duration_with_value(s.trim_end_matches('h'), "h")
    } else if s.ends_with('d') {
        parse_single_duration_with_value(s.trim_end_matches('d'), "d")
    } else {
        None
    }
}

/// Parse a duration with explicit value and unit.
#[must_use]
fn parse_single_duration_with_value(val: &str, unit: &str) -> Option<Duration> {
    let val = val.trim();
    Some(match unit {
        "ns" => Duration::nanoseconds(val.parse::<i64>().ok()?),
        "us" => Duration::microseconds(val.parse::<i64>().ok()?),
        "ms" => Duration::milliseconds(val.parse::<i64>().ok()?),
        "s" => Duration::seconds(val.parse::<f64>().ok()?.round() as i64),
        "m" => Duration::minutes(val.parse::<i64>().ok()?),
        "h" => Duration::hours(val.parse::<i64>().ok()?),
        "d" => Duration::days(val.parse::<i64>().ok()?),
        _ => return None,
    })
}

/// Parse a complex duration string with multiple units (e.g., "1h30m").
#[must_use]
fn parse_complex_duration(s: &str) -> Option<Duration> {
    let chars: Vec<char> = s.chars().collect();
    let mut total_duration = Duration::seconds(0);
    let mut current_num = String::new();
    let mut current_unit = String::new();
    let mut found_part = false;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' || c == '-' {
            if !current_unit.is_empty() {
                total_duration += parse_single_duration_with_value(&current_num, &current_unit)?;
                current_num.clear();
                current_unit.clear();
                found_part = true;
            }
            current_num.push(c);
        } else if c.is_alphabetic() {
            current_unit.push(c);
        }
        i += 1;
    }

    if !current_num.is_empty() && !current_unit.is_empty() {
        total_duration += parse_single_duration_with_value(&current_num, &current_unit)?;
        found_part = true;
    }

    found_part.then_some(total_duration)
}

/// Get a duration configuration value with a default.
#[must_use]
pub fn duration_default(key: &str, default: Duration) -> Duration {
    let s = string(key);
    parse_duration(&s).unwrap_or(default)
}

/// Unmarshal a configuration value into a deserializable type.
pub fn unmarshal<T: for<'de> serde::Deserialize<'de>>(key: &str) -> Result<T, ConfigError> {
    get_config()
        .map_err(|e| ConfigError::UnmarshalError(e.to_string()))
        .and_then(|c| {
            c.get_table(key)
                .map(|t| {
                    toml::Value::Table(t.clone())
                        .try_into::<T>()
                        .map_err(|e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()))
                })
                .unwrap_or_else(|| {
                    let table = c.build_table_from_flat(key);
                    if table.is_empty() {
                        c.get_str(key)
                            .map(|s| {
                                toml::Value::String(s.to_string()).try_into::<T>().map_err(
                                    |e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()),
                                )
                            })
                            .unwrap_or_else(|| {
                                c.get_array(key)
                                    .map(|a: &toml::value::Array| {
                                        toml::Value::Array(a.clone()).try_into::<T>().map_err(
                                            |e: toml::de::Error| {
                                                ConfigError::UnmarshalError(e.to_string())
                                            },
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        Err(ConfigError::UnmarshalError(
                                            "key not found or unsupported type".to_string(),
                                        ))
                                    })
                            })
                    } else {
                        toml::Value::Table(table)
                            .try_into::<T>()
                            .map_err(|e: toml::de::Error| {
                                ConfigError::UnmarshalError(e.to_string())
                            })
                    }
                })
        })
}

// ============================================================================
// Domain-specific configuration helpers
// ============================================================================

/// Get RabbitMQ consumer timeout from config.
#[must_use]
pub fn broker_rabbitmq_consumer_timeout() -> Duration {
    duration_default("broker.rabbitmq.consumer.timeout", Duration::minutes(30))
}

/// Get RabbitMQ durable queues setting from config.
#[must_use]
pub fn broker_rabbitmq_durable_queues() -> bool {
    bool_default("broker.rabbitmq.durable.queues", false)
}

/// Get RabbitMQ queue type from config.
#[must_use]
pub fn broker_rabbitmq_queue_type() -> String {
    string_default("broker.rabbitmq.queue.type", "classic")
}

/// Get worker resource limits from config.
#[must_use]
pub fn worker_limits() -> WorkerLimits {
    WorkerLimits {
        cpus: string_default("worker.limits.cpus", ""),
        memory: string_default("worker.limits.memory", ""),
        timeout: string_default("worker.limits.timeout", ""),
    }
}

/// Check if bind mounts are allowed.
#[must_use]
pub fn mounts_bind_allowed() -> bool {
    bool_default("mounts.bind.allowed", false)
}

/// Get allowed bind mount sources.
#[must_use]
pub fn mounts_bind_sources() -> Vec<String> {
    strings("mounts.bind.sources")
}

/// Get temporary directory for mounts.
#[must_use]
pub fn mounts_temp_dir() -> String {
    string_default("mounts.temp.dir", "/tmp")
}

/// Check if Docker privileged mode is enabled.
#[must_use]
pub fn runtime_docker_privileged() -> bool {
    bool_default("runtime.docker.privileged", false)
}

/// Get Docker image TTL from config.
#[must_use]
pub fn runtime_docker_image_ttl() -> Duration {
    duration_default("runtime.docker.image.ttl", Duration::hours(24))
}

/// Check if Podman privileged mode is enabled.
#[must_use]
pub fn runtime_podman_privileged() -> bool {
    bool_default("runtime.podman.privileged", false)
}

/// Check if Podman host network is enabled.
#[must_use]
pub fn runtime_podman_host_network() -> bool {
    bool_default("runtime.podman.host.network", false)
}

/// Check if web logger middleware is enabled.
#[must_use]
pub fn middleware_web_logger_enabled() -> bool {
    bool_default("middleware.web.logger.enabled", true)
}

/// Get web logger middleware log level.
#[must_use]
pub fn middleware_web_logger_level() -> String {
    string_default("middleware.web.logger.level", "info")
}

/// Get paths to skip in web logger middleware.
#[must_use]
pub fn middleware_web_logger_skip_paths() -> Vec<String> {
    let paths = strings("middleware.web.logger.skip_paths");
    if paths.is_empty() {
        strings("middleware.web.logger.skip")
    } else {
        paths
    }
}
