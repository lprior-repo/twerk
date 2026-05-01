//! Configuration parsing for TOML config files.

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("error parsing TOML: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("error reading file: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub workers: Option<usize>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

impl Config {
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let mut config: Config = toml::from_str(content)?;

        if config.workers.is_none() {
            return Err(ConfigError::MissingField("workers".to_string()));
        }

        Ok(config)
    }

    pub fn from_toml_with_extra(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content).map_err(ConfigError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_with_workers_integer() {
        let toml_content = r#"
workers = 4
"#;
        let config = Config::from_toml(toml_content).unwrap();
        assert_eq!(config.workers, Some(4));
    }

    #[test]
    fn test_parse_config_with_allowed_origins_array() {
        let toml_content = r#"
workers = 4
allowed_origins = ["http://a.com", "http://b.com"]
"#;
        let config = Config::from_toml(toml_content).unwrap();
        assert_eq!(
            config.allowed_origins,
            vec!["http://a.com".to_string(), "http://b.com".to_string()]
        );
    }

    #[test]
    fn test_parse_config_with_nested_database_table() {
        let toml_content = r#"
workers = 4
[database]
host = "localhost"
port = 5432
"#;
        let config = Config::from_toml(toml_content).unwrap();
        assert_eq!(config.database.host, Some("localhost".to_string()));
        assert_eq!(config.database.port, Some(5432));
    }

    #[test]
    fn test_missing_required_field_returns_error_with_field_name() {
        let toml_content = r#"
allowed_origins = ["http://a.com"]
"#;
        let result = Config::from_toml(toml_content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("workers"));
    }

    #[test]
    fn test_unknown_field_does_not_cause_error() {
        let toml_content = r#"
workers = 4
unknown_field = "should be ignored"
another_unknown = 123
"#;
        let config = Config::from_toml_with_extra(toml_content).unwrap();
        assert_eq!(config.workers, Some(4));
        assert!(config.extra.contains_key("unknown_field"));
        assert!(config.extra.contains_key("another_unknown"));
    }

    #[test]
    fn test_complete_toml_parsing() {
        let toml_content = r#"
workers = 4
allowed_origins = ["http://a.com", "http://b.com"]

[database]
host = "localhost"
port = 5432
"#;
        let config = Config::from_toml(toml_content).unwrap();
        assert_eq!(config.workers, Some(4));
        assert_eq!(
            config.allowed_origins,
            vec!["http://a.com".to_string(), "http://b.com".to_string()]
        );
        assert_eq!(config.database.host, Some("localhost".to_string()));
        assert_eq!(config.database.port, Some(5432));
    }
}
