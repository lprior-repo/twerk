//! Configuration lookup functions.

use std::collections::HashMap;

use time::Duration;

use super::types::{ConfigError, ConfigState, WorkerLimits};
use super::CONFIG;

/// Global config state accessor.
fn get_config() -> Result<ConfigState, ConfigError> {
    let guard = CONFIG.read().map_err(|_| ConfigError::Poisoned)?;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| ConfigError::KeyNotFound("config not loaded".to_string()))
}

/// Get a string configuration value.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"section.subsection.key"`).
///
/// **Default value**: Returns an empty string if the key is not found or on error.
///
/// **Error behavior**: Silently returns empty string on any error (config not loaded,
/// key not found, type mismatch). Use `string_default` if you need a non-empty fallback.
#[must_use]
pub fn string(key: &str) -> String {
    get_config()
        .ok()
        .and_then(|c| c.get_str(key).map(|v| v.to_string()))
        .unwrap_or_default()
}

/// Get a string configuration value with a default.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"section.subsection.key"`).
///
/// **Default value**: Returns `default` if key is not found, empty, or on error.
///
/// **Error behavior**: Silently returns `default` on any error.
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
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"feature.flag.enabled"`).
///
/// **Default value**: Returns `false` if key is not found or on error.
///
/// **Error behavior**: Silently returns `false` on any error (config not loaded,
/// key not found, type mismatch).
#[must_use]
pub fn bool(key: &str) -> bool {
    get_config()
        .ok()
        .and_then(|c| c.get_bool(key))
        .is_some_and(|v| v)
}

/// Get a boolean configuration value with a default.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"feature.flag.enabled"`).
///
/// **Default value**: Returns `default` if key is not found or on error.
/// Returns `default` if key exists but value is null/none.
///
/// **Error behavior**: Silently returns `default` on any error.
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
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"server.port"`, `"limits.max_connections"`).
///
/// **Default value**: Returns `0` if key is not found or on error.
///
/// **Error behavior**: Silently returns `0` on any error (config not loaded,
/// key not found, type mismatch).
#[must_use]
pub fn int(key: &str) -> i64 {
    get_config()
        .ok()
        .and_then(|c| c.get_int(key))
        .map_or(0, |v| v)
}

/// Get an integer configuration value with a default.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"server.port"`, `"limits.max_connections"`).
///
/// **Default value**: Returns `default` if key is not found or on error.
/// Returns `default` if key exists but value is null/none.
///
/// **Error behavior**: Silently returns `default` on any error.
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
///
/// **Key format**: Dot-separated hierarchical key prefix (e.g., `"workers.counts"`).
/// Returns all keys nested under this prefix as a map.
///
/// **Default value**: Returns an empty map if key is not found or on error.
///
/// **Error behavior**: Silently returns empty map on any error.
#[must_use]
pub fn int_map(key: &str) -> HashMap<String, i64> {
    get_config()
        .map(|c| c.int_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a string-to-boolean map configuration.
///
/// **Key format**: Dot-separated hierarchical key prefix (e.g., `"features.flags"`).
/// Returns all keys nested under this prefix as a map.
///
/// **Default value**: Returns an empty map if key is not found or on error.
///
/// **Error behavior**: Silently returns empty map on any error.
#[must_use]
pub fn bool_map(key: &str) -> HashMap<String, bool> {
    get_config()
        .map(|c| c.bool_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a string-to-string map configuration.
///
/// **Key format**: Dot-separated hierarchical key prefix (e.g., `"regions.endpoints"`).
/// Returns all keys nested under this prefix as a map.
///
/// **Default value**: Returns an empty map if key is not found or on error.
///
/// **Error behavior**: Silently returns empty map on any error.
#[must_use]
pub fn string_map(key: &str) -> HashMap<String, String> {
    get_config()
        .map(|c| c.string_map_for_key(key))
        .unwrap_or_else(|_| HashMap::new())
}

/// Get a list of strings configuration.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"servers.hosts"`, `" AllowedOrigins"`).
///
/// **Default value**: Returns an empty vector if key is not found or on error.
///
/// **Error behavior**: Silently returns empty vector on any error.
#[must_use]
pub fn strings(key: &str) -> Vec<String> {
    get_config()
        .map(|c| c.strings_for_key_or_string(key))
        .unwrap_or_else(|_| Vec::new())
}

/// Get a list of strings with a default fallback.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"servers.hosts"`, `" AllowedOrigins"`).
///
/// **Default value**: Returns `default` converted to Vec<String> if key is not found,
/// empty, or on error.
///
/// **Error behavior**: Silently returns `default` on any error.
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
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"timeouts.request"`).
///
/// **Duration format**: Parses strings like `"1h30m"`, `"30s"`, `"1d"`, `"500ms"`.
/// Supports: ns, us/µs, ms, s, m, h, d. Complex durations like "1h30m" are supported.
///
/// **Default value**: Returns `default` if key is not found, empty, or cannot be parsed.
///
/// **Error behavior**: Silently returns `default` on any parse error or config error.
#[must_use]
pub fn duration_default(key: &str, default: Duration) -> Duration {
    let s = string(key);
    parse_duration(&s).unwrap_or(default)
}

/// Unmarshal a configuration value into a deserializable type.
///
/// **Key format**: Dot-separated hierarchical key (e.g., `"database.config"`, `"redis.options"`).
///
/// **Type support**: Handles TOML tables, arrays, strings, and primitives.
/// Uses `serde` deserialization for type conversion.
///
/// **Default value**: N/A - returns `Result`. Returns `ConfigError::UnmarshalError`
/// if key not found, type unsupported, or deserialization fails.
///
/// **Error behavior**: Returns `ConfigError::UnmarshalError` with descriptive message.
/// Returns `ConfigError::KeyNotFound` if key does not exist.
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
///
/// **Key**: `"broker.rabbitmq.consumer.timeout"`
///
/// **Default value**: 30 minutes if key not found or on error.
///
/// **Duration format**: Supports `"1h30m"`, `"30s"`, `"1d"`, `"500ms"`, etc.
#[must_use]
pub fn broker_rabbitmq_consumer_timeout() -> Duration {
    duration_default("broker.rabbitmq.consumer.timeout", Duration::minutes(30))
}

/// Get RabbitMQ durable queues setting from config.
///
/// **Key**: `"broker.rabbitmq.durable.queues"`
///
/// **Default value**: `false` if key not found or on error.
#[must_use]
pub fn broker_rabbitmq_durable_queues() -> bool {
    bool_default("broker.rabbitmq.durable.queues", false)
}

/// Get RabbitMQ queue type from config.
///
/// **Key**: `"broker.rabbitmq.queue.type"`
///
/// **Default value**: `"classic"` if key not found or on error.
#[must_use]
pub fn broker_rabbitmq_queue_type() -> String {
    string_default("broker.rabbitmq.queue.type", "classic")
}

/// Get worker resource limits from config.
///
/// **Keys**: `"worker.limits.cpus"`, `"worker.limits.memory"`, `"worker.limits.timeout"`
///
/// **Default values**: Empty string for each limit if key not found or on error.
#[must_use]
pub fn worker_limits() -> WorkerLimits {
    WorkerLimits {
        cpus: string_default("worker.limits.cpus", ""),
        memory: string_default("worker.limits.memory", ""),
        timeout: string_default("worker.limits.timeout", ""),
    }
}

/// Check if bind mounts are allowed.
///
/// **Key**: `"mounts.bind.allowed"`
///
/// **Default value**: `false` if key not found or on error.
#[must_use]
pub fn mounts_bind_allowed() -> bool {
    bool_default("mounts.bind.allowed", false)
}

/// Get allowed bind mount sources.
///
/// **Key**: `"mounts.bind.sources"`
///
/// **Default value**: Empty vector if key not found or on error.
#[must_use]
pub fn mounts_bind_sources() -> Vec<String> {
    strings("mounts.bind.sources")
}

/// Get temporary directory for mounts.
///
/// **Key**: `"mounts.temp.dir"`
///
/// **Default value**: `"/tmp"` if key not found or on error.
#[must_use]
pub fn mounts_temp_dir() -> String {
    string_default("mounts.temp.dir", "/tmp")
}

/// Check if Docker privileged mode is enabled.
///
/// **Key**: `"runtime.docker.privileged"`
///
/// **Default value**: `false` if key not found or on error.
#[must_use]
pub fn runtime_docker_privileged() -> bool {
    bool_default("runtime.docker.privileged", false)
}

/// Get Docker image TTL from config.
///
/// **Key**: `"runtime.docker.image.ttl"`
///
/// **Default value**: 24 hours if key not found or on error.
///
/// **Duration format**: Supports `"1h30m"`, `"30s"`, `"1d"`, `"500ms"`, etc.
#[must_use]
pub fn runtime_docker_image_ttl() -> Duration {
    duration_default("runtime.docker.image.ttl", Duration::hours(24))
}

/// Check if Podman privileged mode is enabled.
///
/// **Key**: `"runtime.podman.privileged"`
///
/// **Default value**: `false` if key not found or on error.
#[must_use]
pub fn runtime_podman_privileged() -> bool {
    bool_default("runtime.podman.privileged", false)
}

/// Check if Podman host network is enabled.
///
/// **Key**: `"runtime.podman.host.network"`
///
/// **Default value**: `false` if key not found or on error.
#[must_use]
pub fn runtime_podman_host_network() -> bool {
    bool_default("runtime.podman.host.network", false)
}

/// Check if web logger middleware is enabled.
///
/// **Key**: `"middleware.web.logger.enabled"`
///
/// **Default value**: `true` if key not found or on error.
#[must_use]
pub fn middleware_web_logger_enabled() -> bool {
    bool_default("middleware.web.logger.enabled", true)
}

/// Get web logger middleware log level.
///
/// **Key**: `"middleware.web.logger.level"`
///
/// **Default value**: `"info"` if key not found or on error.
#[must_use]
pub fn middleware_web_logger_level() -> String {
    string_default("middleware.web.logger.level", "info")
}

/// Get paths to skip in web logger middleware.
///
/// **Keys**: `"middleware.web.logger.skip_paths"` (primary),
/// `"middleware.web.logger.skip"` (fallback)
///
/// **Default value**: Empty vector if both keys not found or on error.
#[must_use]
pub fn middleware_web_logger_skip_paths() -> Vec<String> {
    let paths = strings("middleware.web.logger.skip_paths");
    if paths.is_empty() {
        strings("middleware.web.logger.skip")
    } else {
        paths
    }
}
