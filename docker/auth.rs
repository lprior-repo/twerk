//! Docker registry authentication following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `Config`, `AuthConfig` structs hold authentication data
//! - **Calc**: Pure credential resolution logic
//! - **Actions**: File I/O and subprocess execution at boundary

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use base64::Engine as _;
use thiserror::Error;

// config and credential_helper are sibling modules in the docker directory
// They are re-exported from mod.rs for public access

// Token username marker from docker CLI
const TOKEN_USERNAME: &str = "<token>";

// ----------------------------------------------------------------------------
// Domain Errors
// ----------------------------------------------------------------------------

/// Errors that can occur during authentication operations.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("credentials not found in native keychain")]
    CredentialsNotFound,

    #[error("no credentials server URL")]
    CredentialsMissingServerUrl,

    #[error("invalid auth string format")]
    InvalidAuthString,

    #[error("decoded value longer than expected: expected {expected}, got {actual}")]
    DecodedLengthMismatch { expected: usize, actual: usize },

    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("credential helper error: {0}")]
    CredentialHelper(String),
}

// ----------------------------------------------------------------------------
// Data structures
// ----------------------------------------------------------------------------

/// Authentication configuration for a single registry.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthConfig {
    /// Base64-encoded auth string.
    pub auth: Option<String>,

    /// Identity token (oauth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identitytoken: Option<String>,

    /// Username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Full Docker config file structure.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    /// Current authenticated user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Password or token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Auth configs by hostname.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub auth_configs: HashMap<String, AuthConfig>,

    /// Credential helper to use (without "docker-credential-" prefix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_helpers: Option<HashMap<String, String>>,

    /// Credential store to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_store: Option<String>,
}

impl Config {
    /// Creates a new empty config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads config from the default path or file.
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if the config cannot be loaded.
    pub fn load() -> Result<Self, AuthError> {
        let path = config_path()?;
        Self::load_from_path(&path)
    }

    /// Loads config from a specific path.
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if the file cannot be read or parsed.
    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, AuthError> {
        let path = path.into();
        let content = fs::read_to_string(&path)?;
        serde_json::from_str(&content).map_err(AuthError::Json)
    }

    /// Gets credentials for the given hostname.
    ///
    /// Returns username and password (or identity token if username is empty).
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if credentials cannot be retrieved.
    pub fn get_credentials(&self, hostname: &str) -> Result<(String, String), AuthError> {
        // Check credential helper for this specific hostname
        if let Some(helpers) = &self.credential_helpers {
            if let Some(helper) = helpers.get(hostname) {
                return crate::docker::credential_helper::get_from_helper(helper, hostname)
                    .map_err(|e| AuthError::CredentialHelper(e.to_string()));
            }
        }

        // Fall back to global credentials store
        if let Some(store) = &self.credentials_store {
            return crate::docker::credential_helper::get_from_helper(store, hostname)
                .map_err(|e| AuthError::CredentialHelper(e.to_string()));
        }

        // Check inline auth configs
        if let Some(auth) = self.auth_configs.get(hostname) {
            return resolve_auth_config(auth);
        }

        // Try default helper
        crate::docker::credential_helper::get_from_helper("", hostname)
            .map_err(|e| AuthError::CredentialHelper(e.to_string()))
    }
}

/// Resolves credentials from an auth config.
fn resolve_auth_config(auth: &AuthConfig) -> Result<(String, String), AuthError> {
    // Check for identity token first
    if let Some(token) = &auth.identitytoken {
        if !token.is_empty() {
            return Ok((String::new(), token.clone()));
        }
    }

    // Check for explicit username/password
    if let (Some(username), Some(password)) = (&auth.username, &auth.password) {
        if !username.is_empty() && !password.is_empty() {
            return Ok((username.clone(), password.clone()));
        }
    }

    // Try decoding base64 auth string
    if let Some(auth_str) = &auth.auth {
        return decode_base64_auth(auth_str);
    }

    Ok((String::new(), String::new()))
}

/// Decodes the legacy file-based auth storage from docker CLI.
///
/// Takes the "Auth" field from AuthConfig and decodes it into username and password.
///
/// If "Auth" is empty, returns empty user/pass without error.
///
/// # Errors
///
/// Returns `AuthError` if decoding fails.
pub fn decode_base64_auth(auth_str: &str) -> Result<(String, String), AuthError> {
    if auth_str.is_empty() {
        return Ok((String::new(), String::new()));
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(auth_str)
        .map_err(AuthError::Base64Decode)?;

    let decoded_str =
        String::from_utf8(decoded).map_err(|e| AuthError::CredentialHelper(e.to_string()))?;

    let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AuthError::InvalidAuthString);
    }

    Ok((
        parts[0].to_string(),
        parts[1].trim_end_matches('\0').to_string(),
    ))
}

/// Gets the docker config path.
///
/// Uses `DOCKER_CONFIG` env var if set, otherwise returns `~/.docker/config.json`.
///
/// # Errors
///
/// Returns `AuthError` if the home directory cannot be determined.
pub fn config_path() -> Result<PathBuf, AuthError> {
    if let Some(config_dir) = env::var_os("DOCKER_CONFIG") {
        return Ok(PathBuf::from(config_dir).join("config.json"));
    }

    user_home_config_path()
}

/// Returns the path to the docker config in the current user's home dir.
///
/// # Errors
///
/// Returns `AuthError` if the home directory cannot be determined.
pub fn user_home_config_path() -> Result<PathBuf, AuthError> {
    let home = dirs::home_dir().ok_or_else(|| {
        AuthError::CredentialHelper("cannot determine home directory".to_string())
    })?;
    Ok(home.join(".docker").join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_base64_auth_empty() {
        let result = decode_base64_auth("").expect("should not error on empty");
        assert_eq!("", result.0);
        assert_eq!("", result.1);
    }

    #[test]
    fn test_decode_base64_auth_happy() {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:pass");
        let result = decode_base64_auth(&encoded).expect("should decode");
        assert_eq!("user", result.0);
        assert_eq!("pass", result.1);
    }

    #[test]
    fn test_decode_base64_auth_invalid() {
        // "not base64" is not valid base64
        let result = decode_base64_auth("not base64");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_base64_auth_invalid_format() {
        use base64::Engine as _;
        // No colon separator
        let encoded = base64::engine::general_purpose::STANDARD.encode("invalidformat");
        let result = decode_base64_auth(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_get_credentials_direct() {
        let mut auth_configs = HashMap::new();
        auth_configs.insert(
            "some.domain".to_string(),
            AuthConfig {
                auth: Some(base64::engine::general_purpose::STANDARD.encode("user:pass")),
                ..Default::default()
            },
        );

        let config = Config {
            auth_configs,
            ..Default::default()
        };

        let (user, pass) = config
            .get_credentials("some.domain")
            .expect("should get creds");
        assert_eq!("user", user);
        assert_eq!("pass", pass);
    }

    #[test]
    fn test_config_get_credentials_identity_token() {
        let mut auth_configs = HashMap::new();
        auth_configs.insert(
            "some.domain".to_string(),
            AuthConfig {
                auth: None,
                identitytoken: Some("my-token".to_string()),
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
            },
        );

        let config = Config {
            auth_configs,
            ..Default::default()
        };

        let (user, pass) = config
            .get_credentials("some.domain")
            .expect("should get creds");
        // Identity token takes precedence, username is empty
        assert_eq!("", user);
        assert_eq!("my-token", pass);
    }
}
