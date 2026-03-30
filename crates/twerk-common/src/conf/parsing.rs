//! Configuration parsing and loading logic.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::types::{ConfigError, ConfigState};
use super::CONFIG;

const DEFAULT_CONFIG_PATHS: &[&str] = &[
    "config.local.yaml",
    "config.yaml",
    "config.local.yml",
    "config.yml",
    "config.local.toml",
    "config.toml",
    "~/twerk/config.yaml",
    "~/twerk/config.toml",
    "/etc/twerk/config.yaml",
    "/etc/twerk/config.toml",
];

/// Expands `~` to home directory in a path.
#[must_use]
pub fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        dirs::home_dir().map_or_else(
            || PathBuf::from(path),
            |h| h.join(path.trim_start_matches("~/")),
        )
    } else {
        PathBuf::from(path)
    }
}

/// Parses a TOML file and returns its contents.
pub fn parse_toml_file(path: &str) -> Result<toml::Value, ConfigError> {
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

/// Flattens a nested TOML table into a flat key-value map.
#[must_use]
pub fn flatten_table(prefix: &str, table: &toml::value::Table) -> HashMap<String, toml::Value> {
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

/// Merges override values into base values (overrides take precedence).
#[must_use]
pub fn merge_values(
    mut base: HashMap<String, toml::Value>,
    override_vals: HashMap<String, toml::Value>,
) -> HashMap<String, toml::Value> {
    for (key, value) in override_vals {
        base.insert(key, value);
    }
    base
}

/// Loads configuration from files and environment variables.
pub fn load_config() -> Result<(), ConfigError> {
    let user_config = match std::env::var("TWERK_CONFIG") {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    };
    let paths: Vec<&str> = user_config
        .as_ref()
        .map_or_else(|| DEFAULT_CONFIG_PATHS.to_vec(), |uc| vec![uc.as_str()]);

    let mut file_values = HashMap::new();
    let mut loaded = false;

    for path in &paths {
        match parse_toml_file(path) {
            Ok(toml::Value::Table(table)) => {
                tracing::info!("Config loaded from {}", path);
                file_values = flatten_table("", &table);
                loaded = true;
                break;
            }
            Ok(_) => {}
            Err(ConfigError::NotFound(_)) => continue,
            Err(e) => return Err(e),
        }
    }

    if !loaded {
        if let Some(config) = user_config {
            return Err(ConfigError::UserConfigNotFound(config));
        }
    }

    let env_values = super::env::extract_env_vars();
    let all_values = merge_values(file_values, env_values);
    let state = ConfigState { values: all_values };

    *CONFIG
        .write()
        .map_err(|_| ConfigError::KeyNotFound("config poisoned".to_string()))? = Some(state);

    Ok(())
}
