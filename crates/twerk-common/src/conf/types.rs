//! Configuration types and data structures.

use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

/// Error type for configuration operations.
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

/// Helper struct for parsing TOML values with flexible accessors.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct TomlValue {
    #[serde(flatten)]
    extra: HashMap<String, toml::Value>,
}

#[allow(dead_code)]
impl TomlValue {
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|v| v.as_str())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.extra.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.extra.get(key).and_then(|v| v.as_integer())
    }

    pub fn get_array(&self, key: &str) -> Option<&toml::value::Array> {
        self.extra.get(key).and_then(|v| v.as_array())
    }

    pub fn get_table(&self, key: &str) -> Option<&toml::value::Table> {
        self.extra.get(key).and_then(|v| v.as_table())
    }

    pub fn entries_for_key(&self, key: &str) -> HashMap<String, toml::Value> {
        self.get_table(key).map_or_else(HashMap::new, |t| {
            t.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        })
    }
}

/// Internal state holder for loaded configuration.
#[derive(Clone, Debug, Default)]
pub struct ConfigState {
    pub(crate) values: HashMap<String, toml::Value>,
}

impl ConfigState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn insert(&mut self, key: String, value: toml::Value) {
        self.values.insert(key, value);
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| {
            v.as_bool().or_else(|| {
                v.as_str()
                    .and_then(|s| match s.to_ascii_lowercase().as_str() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    })
            })
        })
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| {
            v.as_integer()
                .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
        })
    }

    pub fn get_array(&self, key: &str) -> Option<&toml::value::Array> {
        self.values.get(key).and_then(|v| v.as_array())
    }

    pub fn get_table(&self, key: &str) -> Option<&toml::value::Table> {
        self.values.get(key).and_then(|v| v.as_table())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn string_map_for_key(&self, key: &str) -> HashMap<String, String> {
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
        }
        let prefix = format!("{}.{}", key, "");
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                v.as_str().map(|s| (sub_key, s.to_string()))
            })
            .collect()
    }

    pub fn int_map_for_key(&self, key: &str) -> HashMap<String, i64> {
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_integer().map(|i| (k.clone(), i)))
                .collect();
        }
        let prefix = format!("{}.{}", key, "");
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                v.as_integer()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                    .map(|i| (sub_key, i))
            })
            .collect()
    }

    pub fn bool_map_for_key(&self, key: &str) -> HashMap<String, bool> {
        if let Some(table) = self.get_table(key) {
            return table
                .iter()
                .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                .collect();
        }
        let prefix = format!("{}.{}", key, "");
        self.values
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, v)| {
                let sub_key = k.strip_prefix(&prefix)?.to_string();
                v.as_bool()
                    .or_else(|| {
                        v.as_str()
                            .map(|s| s.to_lowercase())
                            .and_then(|s| match s.as_str() {
                                "true" => Some(true),
                                "false" => Some(false),
                                _ => None,
                            })
                    })
                    .map(|b| (sub_key, b))
            })
            .collect()
    }

    pub fn strings_for_key(&self, key: &str) -> Vec<String> {
        self.get_array(key).map_or_else(Vec::new, |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
    }

    pub fn strings_from_string(&self, key: &str) -> Vec<String> {
        self.get_str(key).map_or_else(Vec::new, |s| {
            s.split(',').map(|p| p.trim().to_string()).collect()
        })
    }

    pub fn strings_for_key_or_string(&self, key: &str) -> Vec<String> {
        let from_array = self.strings_for_key(key);
        if !from_array.is_empty() {
            return from_array;
        }
        self.strings_from_string(key)
    }

    pub fn build_table_from_flat(&self, key: &str) -> toml::value::Table {
        let prefix = format!("{}.{}", key, "");
        let mut table = toml::value::Table::new();
        for (k, v) in &self.values {
            if let Some(stripped) = k.strip_prefix(&prefix) {
                let sub_key = stripped;
                let parts: Vec<&str> = sub_key.split('.').collect();
                if parts.len() == 1 {
                    table.insert(parts[0].to_string(), v.clone());
                } else {
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
            let key = parts[0];
            let nested = table
                .entry(key.to_string())
                .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
            if let toml::Value::Table(ref mut t) = nested {
                self.insert_nested(t, &parts[1..], value);
            }
        }
    }
}

/// Worker resource limits configuration.
#[derive(Debug, Clone, Default)]
pub struct WorkerLimits {
    pub cpus: String,
    pub memory: String,
    pub timeout: String,
}
