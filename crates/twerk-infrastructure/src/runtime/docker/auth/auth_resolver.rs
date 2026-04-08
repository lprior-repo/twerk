//! Credential resolution logic.
//!
//! Pure functions for resolving credentials from various sources.

use base64::Engine as _;
use std::io::ErrorKind;

use crate::runtime::docker::auth::{get_from_helper, AuthError};
use crate::runtime::docker::{AuthConfig, Config};

/// Resolves credentials from an auth config.
///
/// # Errors
///
/// Returns `AuthError` if credentials cannot be resolved.
pub fn resolve_auth_config(auth: &AuthConfig) -> Result<(String, String), AuthError> {
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
/// Takes the "Auth" field from `AuthConfig` and decodes it into username and password.
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

/// Gets registry credentials for the given hostname.
///
/// This mirrors Go's `getRegistryCredentials(configFile, hostname)`:
/// - Loads the docker config from `config_file` (or default path if empty).
/// - If the config file doesn't exist, falls through to the default credential helper.
/// - Otherwise delegates to [`Config::get_credentials`].
///
/// Hostnames should already be resolved using `ResolveRegistryAuth`.
/// If the returned username is empty, the password is an identity token.
///
/// # Errors
///
/// Returns `AuthError` if credentials cannot be retrieved (except for file-not-found,
/// which falls through to the credential helper).
pub fn get_registry_credentials(
    config_file: &str,
    hostname: &str,
) -> Result<(String, String), AuthError> {
    match Config::load_config(config_file) {
        Ok(cfg) => cfg.get_credentials(hostname),
        Err(AuthError::Io(e)) if e.kind() == ErrorKind::NotFound => {
            // Config file doesn't exist — fall through to default helper
            get_from_helper("", hostname).map_err(|e| AuthError::CredentialHelper(e.to_string()))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::redundant_pattern_matching)]
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    #[test]
    fn test_get_registry_credentials_missing_config_falls_through() {
        // Use a non-existent config file — should fall through to helper (empty result)
        let (user, pass) =
            get_registry_credentials("/nonexistent/path/config.json", "registry.example.com")
                .expect("should fall through to helper");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_config_load_from_path_valid_json() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        let mut file = std::fs::File::create(&config_path).expect("should create file");
        write!(
            file,
            r#"{{
                "auths": {{
                    "registry.example.com": {{
                        "auth": "{auth}"
                    }}
                }}
            }}"#,
            auth = base64::engine::general_purpose::STANDARD.encode("admin:secret")
        )
        .expect("should write");

        let config = Config::load_from_path(&config_path).expect("should load");
        assert!(config.auth_configs.contains_key("registry.example.com"));
    }

    #[test]
    fn test_config_load_from_path_invalid_json() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "not json at all").expect("should write");

        let result = Config::load_from_path(&config_path);
        assert!(matches!(result, Err(_)));
        assert!(matches!(result.unwrap_err(), AuthError::Json(_)));
    }

    #[test]
    fn test_config_load_from_path_missing_file() {
        let result = Config::load_from_path("/nonexistent/config.json");
        assert!(matches!(result, Err(_)));
        assert!(matches!(result.unwrap_err(), AuthError::Io(_)));
    }

    #[test]
    fn test_config_load_from_path_empty_file() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "").expect("should write");

        let result = Config::load_from_path(&config_path);
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_config_load_config_uses_default_path_when_empty() {
        // DOCKER_CONFIG is a directory; config_path() joins "config.json"
        let dir = tempfile::tempdir().expect("should create tempdir");
        let docker_config_dir = dir.path();
        std::fs::write(docker_config_dir.join("config.json"), r#"{"auths": {}}"#)
            .expect("should write");

        std::env::set_var("DOCKER_CONFIG", docker_config_dir);
        let result = Config::load_config("");
        // Restore env
        std::env::remove_var("DOCKER_CONFIG");
        assert!(matches!(result, Ok(_)));
    }

    #[test]
    fn test_config_load_config_uses_explicit_path() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("custom-config.json");
        std::fs::write(
            &config_path,
            r#"{"auths": {"my.registry": {"auth": "dXNlcjpwYXNz"}}}"#,
        )
        .expect("should write");

        let config = Config::load_config(config_path.to_str().expect("should be valid"))
            .expect("should load");
        assert!(config.auth_configs.contains_key("my.registry"));
    }

    #[test]
    fn test_config_deserialize_empty_object() {
        let config: Config = serde_json::from_str("{}").expect("should deserialize");
        assert!(config.auth_configs.is_empty());
        assert!(config.http_headers.is_empty());
        assert!(config.credential_helpers.is_none());
        assert!(config.credentials_store.is_none());
        assert!(config.proxies.is_empty());
        assert!(config.detach_keys.is_none());
        assert!(config.experimental.is_none());
        assert!(config.stack_orchestrator.is_none());
        assert!(config.current_context.is_none());
        assert!(config.kubernetes.is_none());
        assert!(config.aliases.is_empty());
        assert!(config.prune_filters.is_empty());
        assert!(config.cli_plugins_extra_dirs.is_empty());
    }

    #[test]
    fn test_config_deserialize_minimal_auth() {
        // AuthConfig uses PascalCase rename, so JSON key is "Auth"
        let json = r#"{"auths": {"index.docker.io": {"Auth": "dXNlcjpwYXNz"}}}"#;
        let config: Config = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(config.auth_configs.len(), 1);
        let auth = &config.auth_configs["index.docker.io"];
        assert_eq!(auth.auth.as_deref(), Some("dXNlcjpwYXNz"));
    }

    #[test]
    fn test_config_serialize_roundtrip() {
        let original = Config {
            credentials_store: Some("osxkeychain".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).expect("should serialize");
        let deserialized: Config = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(
            deserialized.credentials_store.as_deref(),
            Some("osxkeychain")
        );
    }

    #[test]
    fn test_config_new_is_default() {
        let config = Config::new();
        let default = Config::default();
        assert_eq!(config.credentials_store, default.credentials_store);
        assert_eq!(config.auth_configs.len(), default.auth_configs.len());
    }

    #[test]
    fn test_config_get_credentials_with_credential_helper_configured() {
        // When a credential helper is configured but the helper doesn't exist,
        // get_credentials falls through to default helper (returns empty)
        let mut helpers = HashMap::new();
        helpers.insert(
            "custom.registry".to_string(),
            "nonexistent-helper".to_string(),
        );
        let config = Config {
            credential_helpers: Some(helpers),
            ..Default::default()
        };

        let (user, pass) = config
            .get_credentials("custom.registry")
            .expect("should fall through");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_get_registry_credentials_with_valid_config_file() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        let mut file = std::fs::File::create(&config_path).expect("should create file");
        write!(
            file,
            r#"{{
                "auths": {{
                    "private.registry": {{
                        "Auth": "{auth}"
                    }}
                }}
            }}"#,
            auth = base64::engine::general_purpose::STANDARD.encode("admin:secret")
        )
        .expect("should write");

        let (user, pass) =
            get_registry_credentials(config_path.to_str().expect("path"), "private.registry")
                .expect("should get creds");
        assert_eq!("admin", user);
        assert_eq!("secret", pass);
    }

    #[test]
    fn test_get_registry_credentials_with_invalid_config_file_returns_error() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "invalid json{{{").expect("should write");

        let result = get_registry_credentials(config_path.to_str().expect("path"), "some.host");
        assert!(matches!(result, Err(_)));
    }
}
