//! Docker config file structures.
//!
//! Mirrors the on-disk format of the Docker CLI's `~/.docker/config.json`.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::runtime::docker::auth::{get_from_helper, resolve_auth_config, AuthError};
use crate::runtime::docker::config_path;

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
        let content = std::fs::read_to_string(&path)?;
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
                return get_from_helper(helper, hostname)
                    .map_err(|e| AuthError::CredentialHelper(e.to_string()));
            }
        }

        // Fall back to global credentials store
        if let Some(store) = &self.credentials_store {
            return get_from_helper(store, hostname)
                .map_err(|e| AuthError::CredentialHelper(e.to_string()));
        }

        // Check inline auth configs
        if let Some(auth) = self.auth_configs.get(hostname) {
            return resolve_auth_config(auth);
        }

        // Try default helper
        get_from_helper("", hostname).map_err(|e| AuthError::CredentialHelper(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::redundant_pattern_matching)]
    use super::*;
    use crate::runtime::docker::auth::decode_base64_auth;
    use base64::Engine;

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
    fn test_decode_base64_auth_empty() {
        use crate::runtime::docker::auth::decode_base64_auth;
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
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_decode_base64_auth_invalid_format() {
        use base64::Engine as _;
        // No colon separator
        let encoded = base64::engine::general_purpose::STANDARD.encode("invalidformat");
        let result = decode_base64_auth(&encoded);
        assert!(matches!(result, Err(_)));
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
}
