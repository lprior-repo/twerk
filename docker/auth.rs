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

/// Proxy configuration settings for a host.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ftp_proxy: Option<String>,
}

/// Kubernetes orchestrator settings.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_namespaces: Option<String>,
}

/// Full Docker config file structure.
///
/// Mirrors the on-disk format of the Docker CLI's `~/.docker/config.json`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    /// Auth configs by hostname (maps to `auths` in JSON).
    #[serde(default, alias = "auths", skip_serializing_if = "HashMap::is_empty")]
    pub auth_configs: HashMap<String, AuthConfig>,

    /// Custom HTTP headers sent with registry requests.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub http_headers: HashMap<String, String>,

    /// Credential helpers per hostname (without "docker-credential-" prefix).
    #[serde(
        default,
        alias = "credHelpers",
        skip_serializing_if = "Option::is_none"
    )]
    pub credential_helpers: Option<HashMap<String, String>>,

    /// Global credential store (without "docker-credential-" prefix).
    #[serde(default, alias = "credsStore", skip_serializing_if = "Option::is_none")]
    pub credentials_store: Option<String>,

    /// Proxy settings per host.
    #[serde(default, alias = "proxies", skip_serializing_if = "HashMap::is_empty")]
    pub proxies: HashMap<String, ProxyConfig>,

    /// Detach keys for container interaction.
    #[serde(default, alias = "detachKeys", skip_serializing_if = "Option::is_none")]
    pub detach_keys: Option<String>,

    /// Experimental features flag.
    #[serde(
        default,
        alias = "experimental",
        skip_serializing_if = "Option::is_none"
    )]
    pub experimental: Option<String>,

    /// Stack orchestrator preference.
    #[serde(
        default,
        alias = "stackOrchestrator",
        skip_serializing_if = "Option::is_none"
    )]
    pub stack_orchestrator: Option<String>,

    /// Current Docker context.
    #[serde(
        default,
        alias = "currentContext",
        skip_serializing_if = "Option::is_none"
    )]
    pub current_context: Option<String>,

    /// Kubernetes settings.
    #[serde(default, alias = "kubernetes", skip_serializing_if = "Option::is_none")]
    pub kubernetes: Option<KubernetesConfig>,

    /// Command aliases.
    #[serde(default, alias = "aliases", skip_serializing_if = "HashMap::is_empty")]
    pub aliases: HashMap<String, String>,

    /// Prune filters.
    #[serde(default, alias = "pruneFilters", skip_serializing_if = "Vec::is_empty")]
    pub prune_filters: Vec<String>,

    /// CLI plugin extra directories.
    #[serde(
        default,
        alias = "cliPluginsExtraDirs",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub cli_plugins_extra_dirs: Vec<String>,
}

impl Config {
    /// Creates a new empty config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads config from the default path (`DOCKER_CONFIG` or `~/.docker/config.json`).
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if the config cannot be loaded.
    pub fn load() -> Result<Self, AuthError> {
        let path = config_path()?;
        Self::load_from_path(&path)
    }

    /// Loads config from a specific path, or the default path if empty.
    ///
    /// Mirrors Go's `loadConfig(configFile string)` — if `config_file` is empty,
    /// uses the default config path.
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if the file cannot be read or parsed.
    pub fn load_config(config_file: &str) -> Result<Self, AuthError> {
        let path = if config_file.is_empty() {
            config_path()?
        } else {
            PathBuf::from(config_file)
        };
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
        Err(AuthError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
            // Config file doesn't exist — fall through to default helper
            crate::docker::credential_helper::get_from_helper("", hostname)
                .map_err(|e| AuthError::CredentialHelper(e.to_string()))
        }
        Err(e) => Err(e),
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

    #[test]
    fn test_config_deserialize_full() {
        let json = r#"{
            "auths": {
                "https://index.docker.io/v1/": {
                    "auth": "dXNlcjpwYXNz",
                    "email": "user@example.com"
                }
            },
            "HttpHeaders": {
                "X-Custom": "value"
            },
            "credsStore": "osxkeychain",
            "credHelpers": {
                "custom.registry": "ecr-login"
            },
            "proxies": {
                "default": {
                    "httpProxy": "http://proxy:3128",
                    "httpsProxy": "https://proxy:3128",
                    "noProxy": "localhost,127.0.0.1",
                    "ftpProxy": "http://proxy:3128"
                }
            },
            "detachKeys": "ctrl-q,ctrl-q",
            "experimental": "enabled",
            "stackOrchestrator": "swarm",
            "currentContext": "default",
            "kubernetes": {
                "allNamespaces": "enabled"
            },
            "aliases": {
                "ls": "ps"
            },
            "pruneFilters": [
                "label!=maintainer=*"
            ],
            "cliPluginsExtraDirs": ["/usr/local/lib/docker/cli-plugins"]
        }"#;

        let config: Config = serde_json::from_str(json).expect("should deserialize");
        assert!(config
            .auth_configs
            .contains_key("https://index.docker.io/v1/"));
        assert_eq!(
            config.http_headers.get("X-Custom").map(String::as_str),
            Some("value")
        );
        assert_eq!(config.credentials_store.as_deref(), Some("osxkeychain"));
        assert_eq!(
            config
                .credential_helpers
                .as_ref()
                .and_then(|h| h.get("custom.registry"))
                .map(String::as_str),
            Some("ecr-login")
        );
        assert!(config.proxies.contains_key("default"));
        let proxy = &config.proxies["default"];
        assert_eq!(proxy.http_proxy.as_deref(), Some("http://proxy:3128"));
        assert_eq!(proxy.https_proxy.as_deref(), Some("https://proxy:3128"));
        assert_eq!(proxy.no_proxy.as_deref(), Some("localhost,127.0.0.1"));
        assert_eq!(proxy.ftp_proxy.as_deref(), Some("http://proxy:3128"));
        assert_eq!(config.detach_keys.as_deref(), Some("ctrl-q,ctrl-q"));
        assert_eq!(config.experimental.as_deref(), Some("enabled"));
        assert_eq!(config.stack_orchestrator.as_deref(), Some("swarm"));
        assert_eq!(config.current_context.as_deref(), Some("default"));
        assert_eq!(
            config
                .kubernetes
                .as_ref()
                .and_then(|k| k.all_namespaces.as_deref()),
            Some("enabled")
        );
        assert_eq!(config.aliases.get("ls").map(String::as_str), Some("ps"));
        assert_eq!(config.prune_filters.len(), 1);
        assert_eq!(config.cli_plugins_extra_dirs.len(), 1);
    }

    #[test]
    fn test_config_get_credentials_explicit_user_pass() {
        let mut auth_configs = HashMap::new();
        auth_configs.insert(
            "registry.example.com".to_string(),
            AuthConfig {
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                auth: None,
                identitytoken: None,
            },
        );

        let config = Config {
            auth_configs,
            ..Default::default()
        };

        let (user, pass) = config
            .get_credentials("registry.example.com")
            .expect("should get creds");
        assert_eq!("admin", user);
        assert_eq!("secret", pass);
    }

    #[test]
    fn test_config_get_credentials_unknown_hostname() {
        let config = Config::default();
        // Unknown hostname should fall through to default helper (returns empty)
        let (user, pass) = config
            .get_credentials("unknown.registry.example.com")
            .expect("should fall through to helper");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_decode_base64_auth_null_trim() {
        use base64::Engine as _;
        // Go trims \0 bytes from the password side
        let payload = "user:pass\x00\x00";
        let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
        let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
        assert_eq!("user", user);
        assert_eq!("pass", pass);
    }

    #[test]
    fn test_get_registry_credentials_missing_config_falls_through() {
        // Use a non-existent config file — should fall through to helper (empty result)
        let (user, pass) =
            get_registry_credentials("/nonexistent/path/config.json", "registry.example.com")
                .expect("should fall through to helper");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    // -- Additional tests matching Go coverage --------------------------------

    #[test]
    fn test_decode_base64_auth_with_special_chars_in_password() {
        use base64::Engine as _;
        // Passwords may contain special characters like @, :, /
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:p@ss:w0rd/special");
        let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
        assert_eq!("user", user);
        assert_eq!("p@ss:w0rd/special", pass);
    }

    #[test]
    fn test_decode_base64_auth_colon_in_password() {
        use base64::Engine as _;
        // splitn(2, ':') ensures only first colon is the separator
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:pass:with:colons");
        let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
        assert_eq!("user", user);
        assert_eq!("pass:with:colons", pass);
    }

    #[test]
    fn test_decode_base64_auth_empty_password() {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:");
        let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
        assert_eq!("user", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_decode_base64_auth_non_utf8_payload() {
        use base64::Engine as _;
        // Encode raw non-UTF8 bytes — base64 decode works but UTF-8 decode fails
        let raw: Vec<u8> = vec![0xff, 0xfe, 0x3a, 0x62]; // \xff\xfe:b
        let encoded = base64::engine::general_purpose::STANDARD.encode(&raw);
        let result = decode_base64_auth(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_auth_config_empty_config_returns_empty() {
        let auth = AuthConfig::default();
        let (user, pass) = resolve_auth_config(&auth).expect("should not error");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_resolve_auth_config_empty_identity_token_falls_through() {
        let auth = AuthConfig {
            identitytoken: Some(String::new()),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            auth: None,
        };
        let (user, pass) = resolve_auth_config(&auth).expect("should not error");
        // Empty identity token should NOT take precedence — falls to username/password
        assert_eq!("user", user);
        assert_eq!("pass", pass);
    }

    #[test]
    fn test_resolve_auth_config_auth_field_takes_precedence_over_empty_user_pass() {
        use base64::Engine as _;
        let auth = AuthConfig {
            identitytoken: None,
            username: Some(String::new()),
            password: Some(String::new()),
            auth: Some(base64::engine::general_purpose::STANDARD.encode("real:creds")),
        };
        let (user, pass) = resolve_auth_config(&auth).expect("should not error");
        assert_eq!("real", user);
        assert_eq!("creds", pass);
    }

    #[test]
    fn test_config_load_from_path_valid_json() {
        use std::io::Write;
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
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::Json(_)));
    }

    #[test]
    fn test_config_load_from_path_missing_file() {
        let result = Config::load_from_path("/nonexistent/config.json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::Io(_)));
    }

    #[test]
    fn test_config_load_from_path_empty_file() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, "").expect("should write");

        let result = Config::load_from_path(&config_path);
        assert!(result.is_err());
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
        assert!(result.is_ok());
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
        use std::io::Write;
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
        assert!(result.is_err());
    }
}
