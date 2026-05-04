//! Environment variable extraction and processing.

use std::collections::HashMap;

/// Extracts environment variables matching the given prefix and converts them to config keys.
#[must_use]
pub fn extract_env_vars_with_prefix(prefix: &str) -> HashMap<String, toml::Value> {
    let mut values = HashMap::new();
    for (key, value) in std::env::vars() {
        if key.starts_with(prefix) {
            let config_key = key
                .trim_start_matches(prefix)
                .to_ascii_lowercase()
                .replace('_', ".");
            values.insert(config_key, toml::Value::String(value));
        }
    }
    values
}

/// Extracts TWERK_ prefixed environment variables and converts them to config keys.
#[must_use]
pub fn extract_env_vars() -> HashMap<String, toml::Value> {
    extract_env_vars_with_prefix("TWERK_")
}
