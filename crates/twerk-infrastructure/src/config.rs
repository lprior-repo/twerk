//! Configuration module for loading TOML config with environment variable overrides.
//!
//! Loads TOML config from default paths, with environment variable overrides.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![allow(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::Deserialize;
use thiserror::Error;

// Global config state using RwLock for thread-safe lazy initialization and updates
static CONFIG: RwLock<Option<ConfigState>> = RwLock::new(None);

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),

    #[error("error parsing config from {path}: {source}")]
    ParseError {
        path: String,
        source: toml::de::Error,
    },

    #[error("error reading config file {path}: {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[error("could not find config file in: {0}")]
    UserConfigNotFound(String),

    #[error("config key not found: {0}")]
    KeyNotFound(String),

    #[error("error loading config from env: {0}")]
    EnvError(String),

    #[error("error unmarshaling config: {0}")]
    UnmarshalError(String),
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct TomlValue {
    #[serde(flatten)]
    extra: HashMap<String, toml::Value>,
}

#[allow(dead_code)]
impl TomlValue {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|v| v.as_str())
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.extra.get(key).and_then(|v| v.as_bool())
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|v| v.as_integer())
    }

    fn get_array(&self, key: &str) -> Option<&toml::value::Array> {
        self.extra.get(key).and_then(|v| v.as_array())
    }

    fn get_table(&self, key: &str) -> Option<&toml::value::Table> {
        self.extra.get(key).and_then(|v| v.as_table())
    }

    fn entries_for_key(&self, key: &str) -> HashMap<String, toml::Value> {
        self.get_table(key)
            .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default)]
struct ConfigState {
    values: HashMap<String, toml::Value>,
}

impl ConfigState {
    #[allow(dead_code)]
    fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    fn insert(&mut self, key: String, value: toml::Value) {
        self.values.insert(key, value);
    }

    fn get_str(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| {
            v.as_bool().or_else(|| {
                v.as_str().and_then(|s| match s.to_lowercase().as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                })
            })
        })
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| {
            v.as_integer()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
    }

    fn get_array(&self, key: &str) -> Option<&toml::value::Array> {
        self.values.get(key).and_then(|v| v.as_array())
    }

    fn get_table(&self, key: &str) -> Option<&toml::value::Table> {
        self.values.get(key).and_then(|v| v.as_table())
    }

    /// Build a table from flat keys with the given prefix.
    /// For example, if key="main" and we have "main.str1", "main.bool1",
    /// this returns a Table with str1 and bool1 entries.
    fn build_table_from_flat(&self, key: &str) -> toml::value::Table {
        let prefix = format!("{}.", key);
        let mut table = toml::value::Table::new();
        for (k, v) in &self.values {
            if let Some(stripped) = k.strip_prefix(&prefix) {
                let sub_key = stripped;
                // Handle nested keys (e.g., main.nested.key -> nested = { key = ... })
                let parts: Vec<&str> = sub_key.split('.').collect();
                if parts.len() == 1 {
                    table.insert(parts[0].to_string(), v.clone());
                } else {
                    // For nested keys, build intermediate structure
                    self.insert_nested(&mut table, &parts, v.clone());
                }
            }
        }
        table
    }

    fn insert_nested(&self, table: &mut toml::value::Table, parts: &[&str], value: toml::Value) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            table.insert(parts[0].to_string(), value);
        } else {
            let key = parts[0].to_string();
            let nested = table
                .entry(key.clone())
                .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
            if let toml::Value::Table(ref mut t) = nested {
                self.insert_nested(t, &parts[1..], value);
            }
        }
    }

    fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    fn string_map_for_key(&self, key: &str) -> HashMap<String, String> {
        // First try to get as a nested table
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
        }
        // Fall back to flat keys with the prefix
        let prefix = format!("{}.", key);
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                v.as_str().map(|s| (sub_key, s.to_string()))
            })
            .collect()
    }

    fn int_map_for_key(&self, key: &str) -> HashMap<String, i64> {
        // First try to get as a nested table
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_integer().map(|i| (k.clone(), i)))
                .collect();
        }
        // Fall back to flat keys with the prefix
        let prefix = format!("{}.", key);
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                // Try integer first, then try parsing string as integer
                v.as_integer()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                    .map(|i| (sub_key, i))
            })
            .collect()
    }

    fn bool_map_for_key(&self, key: &str) -> HashMap<String, bool> {
        // First try to get as a nested table
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                .collect();
        }
        // Fall back to flat keys with the prefix
        let prefix = format!("{}.", key);
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                // Try boolean first, then try parsing string as boolean
                v.as_bool()
                    .or_else(|| {
                        v.as_str().and_then(|s| match s.to_lowercase().as_str() {
                            "true" => Some(true),
                            "false" => Some(false),
                            _ => None,
                        })
                    })
                    .map(|b| (sub_key, b))
            })
            .collect()
    }

    fn strings_for_key(&self, key: &str) -> Vec<String> {
        self.get_array(key)
            .map(|arr: &toml::value::Array| {
                arr.iter()
                    .filter_map(|v: &toml::Value| v.as_str().map(|s: &str| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn strings_from_string(&self, key: &str) -> Vec<String> {
        self.get_str(key)
            .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
            .unwrap_or_default()
    }

    fn strings_for_key_or_string(&self, key: &str) -> Vec<String> {
        let from_array = self.strings_for_key(key);
        if !from_array.is_empty() {
            return from_array;
        }
        self.strings_from_string(key)
    }
}

const DEFAULT_CONFIG_PATHS: &[&str] = &[
    "config.local.toml",
    "config.toml",
    "~/twerk/config.toml",
    "/etc/twerk/config.toml",
];

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(path.trim_start_matches("~/")))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

fn parse_toml_file(path: &str) -> Result<toml::Value, ConfigError> {
    let expanded = expand_path(path);
    if !expanded.exists() {
        return Err(ConfigError::NotFound(path.to_string()));
    }
    let content = fs::read_to_string(&expanded).map_err(|e| ConfigError::IoError {
        path: path.to_string(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::ParseError {
        path: path.to_string(),
        source: e,
    })
}

fn flatten_table(prefix: &str, table: &toml::value::Table) -> HashMap<String, toml::Value> {
    let mut result = HashMap::new();
    for (key, value) in table {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        match value {
            toml::Value::Table(t) => {
                result.extend(flatten_table(&full_key, t));
            }
            _ => {
                result.insert(full_key, value.clone());
            }
        }
    }
    result
}

fn extract_env_vars() -> HashMap<String, toml::Value> {
    let mut values = HashMap::new();
    for (key, value) in env::vars() {
        if key.starts_with("TWERK_") {
            let config_key = key
                .trim_start_matches("TWERK_")
                .to_lowercase()
                .replace('_', ".");
            values.insert(config_key, toml::Value::String(value));
        }
    }
    values
}

fn merge_values(
    mut base: HashMap<String, toml::Value>,
    override_vals: HashMap<String, toml::Value>,
) -> HashMap<String, toml::Value> {
    for (key, value) in override_vals {
        base.insert(key, value);
    }
    base
}

/// Load configuration from files and environment variables.
///
/// Files are searched in order: custom path from TWERK_CONFIG env var,
/// then default paths. Environment variables with TWERK_ prefix override
/// file settings (e.g., TWERK_MAIN_KEY=value sets main.key).
pub fn load_config() -> Result<(), ConfigError> {
    // Determine config paths to try
    let user_config = env::var("TWERK_CONFIG").ok();
    let paths: Vec<&str> = if let Some(ref uc) = user_config {
        vec![uc.as_str()]
    } else {
        DEFAULT_CONFIG_PATHS.iter().copied().collect()
    };

    // Try loading from each path
    let mut file_values: HashMap<String, toml::Value> = HashMap::new();
    let mut loaded = false;
    let _last_not_found: Option<String> = None;

    for path in &paths {
        match parse_toml_file(path) {
            Ok(toml::Value::Table(table)) => {
                tracing::info!("Config loaded from {}", path);
                file_values = flatten_table("", &table);
                loaded = true;
                break;
            }
            Ok(_) => {
                // TOML file parsed but root is not a table - shouldn't happen
            }
            Err(ConfigError::NotFound(ref _p)) => {
                continue;
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    if !loaded {
        if let Some(config) = user_config {
            return Err(ConfigError::UserConfigNotFound(config));
        }
        // Even if no file loaded, continue with empty config
    }

    // Load environment variables
    let env_values = extract_env_vars();

    // Merge: env vars override file values
    let all_values = merge_values(file_values, env_values);

    let state = ConfigState { values: all_values };

    let mut guard = CONFIG
        .write()
        .map_err(|_| ConfigError::KeyNotFound("config poisoned".to_string()))?;
    *guard = Some(state);

    Ok(())
}

fn get_config() -> Result<ConfigState, ConfigError> {
    let guard = CONFIG
        .read()
        .map_err(|_| ConfigError::KeyNotFound("config poisoned".to_string()))?;
    guard
        .as_ref()
        .map(|cs| cs.clone())
        .ok_or_else(|| ConfigError::KeyNotFound("config not loaded".to_string()))
}

/// Get a string value from config.
pub fn string(key: &str) -> String {
    get_config()
        .ok()
        .and_then(|c| c.get_str(key).map(|s| s.to_string()))
        .unwrap_or_default()
}

/// Get a string value with default.
pub fn string_default(key: &str, default: &str) -> String {
    string(key)
        .is_empty()
        .then_some(default.to_string())
        .unwrap_or_else(|| string(key))
}

/// Get a boolean value from config.
pub fn bool(key: &str) -> bool {
    get_config()
        .ok()
        .and_then(|c| c.get_bool(key))
        .unwrap_or(false)
}

/// Get a boolean value with default.
pub fn bool_default(key: &str, default: bool) -> bool {
    get_config()
        .ok()
        .and_then(|c| {
            if c.contains_key(key) {
                c.get_bool(key)
            } else {
                None
            }
        })
        .unwrap_or(default)
}

/// Get an integer value from config.
pub fn int(key: &str) -> i64 {
    get_config().ok().and_then(|c| c.get_int(key)).unwrap_or(0)
}

/// Get an integer value with default.
pub fn int_default(key: &str, default: i64) -> i64 {
    get_config()
        .ok()
        .and_then(|c| {
            if c.contains_key(key) {
                c.get_int(key)
            } else {
                None
            }
        })
        .unwrap_or(default)
}

/// Get a map of strings to ints from config.
pub fn int_map(key: &str) -> HashMap<String, i64> {
    get_config()
        .map(|c| c.int_map_for_key(key))
        .unwrap_or_default()
}

/// Get a map of strings to bools from config.
pub fn bool_map(key: &str) -> HashMap<String, bool> {
    get_config()
        .map(|c| c.bool_map_for_key(key))
        .unwrap_or_default()
}

/// Get a map of strings to strings from config.
pub fn string_map(key: &str) -> HashMap<String, String> {
    get_config()
        .map(|c| c.string_map_for_key(key))
        .unwrap_or_default()
}

/// Get a list of strings from config.
/// First tries to get as a list, then as a comma-separated string.
pub fn strings(key: &str) -> Vec<String> {
    get_config()
        .map(|c| c.strings_for_key_or_string(key))
        .unwrap_or_default()
}

/// Get a list of strings with default.
pub fn strings_default(key: &str, default: &[&str]) -> Vec<String> {
    let v = strings(key);
    if v.is_empty() {
        default.iter().map(|s| s.to_string()).collect()
    } else {
        v
    }
}

/// Get a duration value from config (parses strings like "5m", "1h").
pub fn duration_default(key: &str, default: time::Duration) -> time::Duration {
    let s = string(key);
    parse_duration(&s).unwrap_or(default)
}

fn parse_duration(s: &str) -> Option<time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try parsing as a single unit first (the simple case)
    if let Some(d) = parse_single_duration(s) {
        return Some(d);
    }

    // Handle complex durations like "1h30m"
    let mut total_duration = time::Duration::seconds(0);
    let mut current_num = String::new();
    let mut current_unit = String::new();
    let mut found_part = false;

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' || c == '-' {
            if !current_unit.is_empty() {
                let unit_duration = parse_single_duration_with_value(&current_num, &current_unit)?;
                total_duration = total_duration + unit_duration;
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
        let unit_duration = parse_single_duration_with_value(&current_num, &current_unit)?;
        total_duration = total_duration + unit_duration;
        found_part = true;
    }

    if found_part {
        Some(total_duration)
    } else {
        None
    }
}

fn parse_single_duration(s: &str) -> Option<time::Duration> {
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

fn parse_single_duration_with_value(val: &str, unit: &str) -> Option<time::Duration> {
    let val = val.trim();
    match unit {
        "ns" => val
            .parse::<i64>()
            .ok()
            .map(|n| time::Duration::nanoseconds(n)),
        "us" => val
            .parse::<i64>()
            .ok()
            .map(|n| time::Duration::microseconds(n)),
        "ms" => val
            .parse::<i64>()
            .ok()
            .map(|n| time::Duration::milliseconds(n)),
        "s" => val.parse::<f64>().ok().map(time::Duration::seconds_f64),
        "m" => val.parse::<i64>().ok().map(time::Duration::minutes),
        "h" => val.parse::<i64>().ok().map(time::Duration::hours),
        "d" => val.parse::<i64>().ok().map(time::Duration::days),
        _ => None,
    }
}

/// Unmarshal a config key into a struct.
pub fn unmarshal<T: for<'de> Deserialize<'de>>(key: &str) -> Result<T, ConfigError> {
    get_config()
        .map_err(|e| ConfigError::UnmarshalError(e.to_string()))
        .and_then(|c| {
            // Get the value at key and deserialize it
            c.get_table(key)
                .map(|t| {
                    toml::Value::Table(t.clone())
                        .try_into::<T>()
                        .map_err(|e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()))
                })
                .unwrap_or_else(|| {
                    // Try flat keys with the prefix
                    let table = c.build_table_from_flat(key);
                    if !table.is_empty() {
                        return toml::Value::Table(table).try_into::<T>().map_err(
                            |e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()),
                        );
                    }
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
                })
        })
}

// =============================================================================
// Broker RabbitMQ Configuration
// =============================================================================

/// Get RabbitMQ consumer timeout (default: 30 minutes).
#[must_use]
pub fn broker_rabbitmq_consumer_timeout() -> time::Duration {
    duration_default(
        "broker.rabbitmq.consumer.timeout",
        time::Duration::minutes(30),
    )
}

/// Get RabbitMQ durable queues setting (default: false).
#[must_use]
pub fn broker_rabbitmq_durable_queues() -> bool {
    bool_default("broker.rabbitmq.durable.queues", false)
}

/// Get RabbitMQ queue type (default: "classic").
#[must_use]
pub fn broker_rabbitmq_queue_type() -> String {
    string_default("broker.rabbitmq.queue.type", "classic")
}

// =============================================================================
// Worker Limits Configuration
// =============================================================================

/// Get worker limits configuration.
#[must_use]
pub fn worker_limits() -> WorkerLimits {
    WorkerLimits {
        cpus: string_default("worker.limits.cpus", ""),
        memory: string_default("worker.limits.memory", ""),
        timeout: string_default("worker.limits.timeout", ""),
    }
}

/// Worker resource limits configuration.
#[derive(Debug, Clone, Default)]
pub struct WorkerLimits {
    /// CPU limit (e.g., "1", "2", "0.5")
    pub cpus: String,
    /// Memory limit (e.g., "512m", "1g")
    pub memory: String,
    /// Timeout duration (e.g., "5m", "1h")
    pub timeout: String,
}

// =============================================================================
// Mounts Configuration
// =============================================================================

/// Get mounts.bind.allowed setting (default: false).
#[must_use]
pub fn mounts_bind_allowed() -> bool {
    bool_default("mounts.bind.allowed", false)
}

/// Get mounts.bind.sources list (default: empty = all sources allowed).
#[must_use]
pub fn mounts_bind_sources() -> Vec<String> {
    strings("mounts.bind.sources")
}

/// Get mounts.temp.dir setting (default: "/tmp").
#[must_use]
pub fn mounts_temp_dir() -> String {
    string_default("mounts.temp.dir", "/tmp")
}

// =============================================================================
// Runtime Docker Configuration
// =============================================================================

/// Get runtime.docker.privileged setting (default: false).
#[must_use]
pub fn runtime_docker_privileged() -> bool {
    bool_default("runtime.docker.privileged", false)
}

/// Get runtime.docker.image.ttl duration (default: 24 hours).
#[must_use]
pub fn runtime_docker_image_ttl() -> time::Duration {
    duration_default("runtime.docker.image.ttl", time::Duration::hours(24))
}

// =============================================================================
// Runtime Podman Configuration
// =============================================================================

/// Get runtime.podman.privileged setting (default: false).
#[must_use]
pub fn runtime_podman_privileged() -> bool {
    bool_default("runtime.podman.privileged", false)
}

/// Get runtime.podman.host.network setting (default: false).
#[must_use]
pub fn runtime_podman_host_network() -> bool {
    bool_default("runtime.podman.host.network", false)
}

// =============================================================================
// Middleware Web Logger Configuration
// =============================================================================

/// Get middleware.web.logger.enabled setting (default: true).
#[must_use]
pub fn middleware_web_logger_enabled() -> bool {
    bool_default("middleware.web.logger.enabled", true)
}

/// Get middleware.web.logger.level setting (default: "info").
#[must_use]
pub fn middleware_web_logger_level() -> String {
    string_default("middleware.web.logger.level", "info")
}

/// Get middleware.web.logger.skip_paths list (default: empty).
/// Note: Go config uses "skip" key, this reads both "skip" and "skip_paths".
#[must_use]
pub fn middleware_web_logger_skip_paths() -> Vec<String> {
    // Try skip_paths first (Rust convention), then fall back to skip (Go convention)
    let paths = strings("middleware.web.logger.skip_paths");
    if !paths.is_empty() {
        return paths;
    }
    strings("middleware.web.logger.skip")
}
