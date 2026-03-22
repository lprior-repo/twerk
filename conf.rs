//! Configuration module for loading TOML config with environment variable overrides.
//!
//! Loads TOML config from default paths, with environment variable overrides.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;
use thiserror::Error;

// Global config state using OnceLock for thread-safe lazy initialization
static CONFIG: OnceLock<ConfigState> = OnceLock::new();

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

#[derive(Clone, Debug, Default, Deserialize)]
struct TomlValue {
    #[serde(flatten)]
    extra: HashMap<String, toml::Value>,
}

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
    fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, key: String, value: toml::Value) {
        self.values.insert(key, value);
    }

    fn get_str(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_integer())
    }

    fn get_array(&self, key: &str) -> Option<&toml::value::Array> {
        self.values.get(key).and_then(|v| v.as_array())
    }

    fn get_table(&self, key: &str) -> Option<&toml::value::Table> {
        self.values.get(key).and_then(|v| v.as_table())
    }

    fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    fn string_map_for_key(&self, key: &str) -> HashMap<String, String> {
        self.get_table(key)
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn int_map_for_key(&self, key: &str) -> HashMap<String, i64> {
        self.get_table(key)
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_integer().map(|i| (k.clone(), i)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn bool_map_for_key(&self, key: &str) -> HashMap<String, bool> {
        self.get_table(key)
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                    .collect()
            })
            .unwrap_or_default()
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
    "~/tork/config.toml",
    "/etc/tork/config.toml",
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
        if key.starts_with("TORK_") {
            let config_key = key
                .trim_start_matches("TORK_")
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
/// Files are searched in order: custom path from TORK_CONFIG env var,
/// then default paths. Environment variables with TORK_ prefix override
/// file settings (e.g., TORK_MAIN_KEY=value sets main.key).
pub fn load_config() -> Result<(), ConfigError> {
    // Determine config paths to try
    let user_config = env::var("TORK_CONFIG").ok();
    let paths: Vec<&str> = if let Some(ref uc) = user_config {
        vec![uc.as_str()]
    } else {
        DEFAULT_CONFIG_PATHS.iter().copied().collect()
    };

    // Try loading from each path
    let mut file_values: HashMap<String, toml::Value> = HashMap::new();
    let mut loaded = false;
    let mut last_not_found: Option<String> = None;

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
            Err(ConfigError::NotFound(ref p)) => {
                last_not_found = Some(p.clone());
                continue;
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    if !loaded {
        if user_config.is_some() {
            return Err(ConfigError::UserConfigNotFound(user_config.unwrap()));
        }
        // Even if no file loaded, continue with empty config
    }

    // Load environment variables
    let env_values = extract_env_vars();

    // Merge: env vars override file values
    let all_values = merge_values(file_values, env_values);

    let state = ConfigState { values: all_values };

    let _ = CONFIG.set(state);

    Ok(())
}

fn get_config() -> Result<&'static ConfigState, ConfigError> {
    CONFIG
        .get()
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
    if s.ends_with("ns") {
        s.trim_end_matches("ns")
            .parse::<u64>()
            .ok()
            .and_then(|n| n.checked_mul(1))
            .map(|_| time::Duration::new(0, 1))
    } else if s.ends_with("us") {
        s.trim_end_matches("us")
            .parse::<u64>()
            .ok()
            .map(time::Duration::microseconds)
    } else if s.ends_with("ms") {
        s.trim_end_matches("ms")
            .parse::<u64>()
            .ok()
            .map(time::Duration::milliseconds)
    } else if s.ends_with('s') && !s.ends_with("ms") {
        s.trim_end_matches('s')
            .parse::<f64>()
            .ok()
            .map(time::Duration::seconds_f64)
    } else if s.ends_with('m') && !s.ends_with("ms") && !s.ends_with("ns") && !s.ends_with("us") {
        s.trim_end_matches("m")
            .parse::<i64>()
            .ok()
            .map(time::Duration::minutes)
    } else if s.ends_with('h') {
        s.trim_end_matches('h')
            .parse::<i64>()
            .ok()
            .map(time::Duration::hours)
    } else if s.ends_with('d') {
        s.trim_end_matches('d')
            .parse::<i64>()
            .ok()
            .map(time::Duration::days)
    } else {
        None
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
                        .try_into()
                        .map_err(|e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()))
                })
                .unwrap_or_else(|| {
                    c.get_str(key)
                        .map(|s| {
                            toml::Value::String(s.to_string()).try_into().map_err(
                                |e: toml::de::Error| ConfigError::UnmarshalError(e.to_string()),
                            )
                        })
                        .unwrap_or_else(|| {
                            c.get_array(key)
                                .map(|a: &toml::value::Array| {
                                    toml::Value::Array(a.clone()).try_into().map_err(
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
        .and_then(|r| r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            // Create test config file
            let config = r#"
[main]
key1 = "value1"
enabled = true
map.key1 = 1
map.key2 = 2
strings = ["a", "b"]
duration = "5m"
"#;
            fs::write("config.toml", config).ok();
        });
    }

    fn cleanup() {
        let _ = fs::remove_file("config.toml");
        let _ = fs::remove_file("config_strings.toml");
        let _ = fs::remove_file("config_with_override.toml");
        let _ = fs::remove_file("myconfig.toml");
        env::remove_var("TORK_CONFIG");
        env::remove_var("TORK_MAIN_STRINGS_KEYS");
        env::remove_var("TORK_BOOLMAP_KEY1");
        env::remove_var("TORK_BOOLMAP_KEY2");
        env::remove_var("TORK_MAIN_KEY1");
        env::remove_var("TORK_HELLO");
    }

    fn reset_config() {
        let _ = CONFIG.set(ConfigState::new());
    }

    #[test]
    fn test_load_config_not_exist() {
        reset_config();
        let result = load_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_config_not_exist_user_defined() {
        reset_config();
        env::set_var("TORK_CONFIG", "no.such.thing");
        let result = load_config();
        assert!(result.is_err());
        if let Err(ConfigError::UserConfigNotFound(path)) = result {
            assert_eq!(path, "no.such.thing");
        } else {
            panic!("expected UserConfigNotFound error");
        }
        cleanup();
    }

    #[test]
    fn test_load_config_bad_contents() {
        reset_config();
        fs::write("config.toml", "xyz").ok();
        let result = load_config();
        assert!(result.is_err());
        cleanup();
    }

    #[test]
    fn test_string() {
        reset_config();
        let config = r#"
[main]
key1 = "value1"
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert_eq!("value1", string("main.key1"));
        cleanup();
    }

    #[test]
    fn test_strings() {
        reset_config();
        let config = r#"
[main]
keys = ["value1"]
"#;
        fs::write("config_strings.toml", config).ok();
        env::set_var("TORK_CONFIG", "config_strings.toml");
        load_config().ok();
        assert_eq!(vec!["value1"], strings("main.keys"));
        cleanup();
    }

    #[test]
    fn test_strings_env() {
        reset_config();
        env::set_var("TORK_MAIN_STRINGS_KEYS", "a,b,c");
        load_config().ok();
        assert_eq!(vec!["a", "b", "c"], strings("main.strings.keys"));
        cleanup();
    }

    #[test]
    fn test_string_default() {
        reset_config();
        let config = r#"
[main]
key1 = "value1"
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert_eq!("v2", string_default("main.key2", "v2"));
        cleanup();
    }

    #[test]
    fn test_int_map() {
        reset_config();
        let config = r#"
[main]
map.key1 = 1
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        let result = int_map("main.map");
        assert_eq!(1, result.get("key1").copied().unwrap_or(0));
        cleanup();
    }

    #[test]
    fn test_bool_true() {
        reset_config();
        let config = r#"
[main]
enabled = true
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert!(bool("main.enabled"));
        cleanup();
    }

    #[test]
    fn test_bool_false() {
        reset_config();
        let config = r#"
[main]
enabled = false
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert!(!bool("main.enabled"));
        cleanup();
    }

    #[test]
    fn test_bool_default() {
        reset_config();
        let config = r#"
[main]
enabled = false
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert!(!bool_default("main.enabled", true));
        assert!(!bool_default("main.enabled", false));
        assert!(bool_default("main.other", true));
        cleanup();
    }

    #[test]
    fn test_duration_default() {
        reset_config();
        let config = r#"
[main]
some.duration = "5m"
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();
        assert_eq!(
            time::Duration::minutes(5),
            duration_default("main.some.duration", time::Duration::seconds(60))
        );
        assert_eq!(
            time::Duration::seconds(60),
            duration_default("main.other.duration", time::Duration::seconds(60))
        );
        cleanup();
    }

    #[test]
    fn test_bool_map() {
        reset_config();
        env::set_var("TORK_BOOLMAP_KEY1", "false");
        env::set_var("TORK_BOOLMAP_KEY2", "true");
        load_config().ok();
        let m = bool_map("boolmap");
        assert_eq!(false, m.get("key1").copied().unwrap_or(true));
        assert_eq!(true, m.get("key2").copied().unwrap_or(false));
        cleanup();
    }

    #[test]
    fn test_load_config_env() {
        reset_config();
        env::set_var("TORK_HELLO", "world");
        load_config().ok();
        assert_eq!("world", string("hello"));
        cleanup();
    }

    #[test]
    fn test_load_config_with_overriding_env() {
        reset_config();
        let config = r#"
[main]
key1 = "value1"
key3 = "value3"
"#;
        fs::write("config_with_override.toml", config).ok();
        env::set_var("TORK_CONFIG", "config_with_override.toml");
        env::set_var("TORK_MAIN_KEY1", "value2");
        load_config().ok();
        assert_eq!("value2", string("main.key1"));
        assert_eq!("value3", string("main.key3"));
        cleanup();
    }

    #[test]
    fn test_unmarshal() {
        reset_config();
        let config = r#"
[main]
str1 = "value1"
bool1 = true
sarr1 = ["a","b"]
"#;
        fs::write("config.toml", config).ok();
        load_config().ok();

        #[derive(Debug, Deserialize, PartialEq)]
        struct MyConfig {
            str1: String,
            #[serde(default)]
            str2: String,
            bool1: bool,
            #[serde(default)]
            sarr1: Vec<String>,
            #[serde(default)]
            sarr2: Vec<String>,
        }

        let result: Result<MyConfig, _> = unmarshal("main");
        assert!(result.is_ok());
        let c = result.unwrap();
        assert_eq!("value1", c.str1);
        assert_eq!("", c.str2);
        assert!(c.bool1);
        assert_eq!(vec!["a", "b"], c.sarr1);
        cleanup();
    }
}
