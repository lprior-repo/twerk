//! Docker config file handling.
//!
//! This module is re-exported from auth.rs for backwards compatibility.

use std::env;
use std::path::PathBuf;

use thiserror::Error;

/// Errors from config path operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot determine home directory")]
    NoHomeDir,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Gets the docker config path.
///
/// Uses `DOCKER_CONFIG` env var if set, otherwise returns `~/.docker/config.json`.
///
/// # Errors
///
/// Returns `ConfigError` if the home directory cannot be determined.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    if let Some(config_dir) = env::var_os("DOCKER_CONFIG") {
        return Ok(PathBuf::from(config_dir).join("config.json"));
    }

    user_home_config_path()
}

/// Returns the path to the docker config in the current user's home dir.
///
/// # Errors
///
/// Returns `ConfigError` if the home directory cannot be determined.
pub fn user_home_config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
    Ok(home.join(".docker").join("config.json"))
}
