//! Tests for docker::auth module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::redundant_pattern_matching)]

use std::collections::HashMap;

use base64::Engine as _;

use crate::runtime::docker::auth::{
    decode_base64_auth, get_registry_credentials, resolve_auth_config, AuthConfig, AuthError,
    Config,
};

#[test]
fn test_decode_base64_auth_empty() {
    let result = decode_base64_auth("").expect("should not error on empty");
    assert_eq!("", result.0);
    assert_eq!("", result.1);
}

#[test]
fn test_decode_base64_auth_happy() {
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
    // No colon separator
    let encoded = base64::engine::general_purpose::STANDARD.encode("invalidformat");
    let result = decode_base64_auth(&encoded);
    assert!(matches!(result, Err(_)));
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
        "cliPluginsExtraDirs": ["/usr/local/lib/docker/cli-plugins"],
        "psFormat": "table {{.ID}}\t{{.Names}}",
        "imagesFormat": "table {{.Repository}}\t{{.Tag}}",
        "networksFormat": "table {{.Name}}\t{{.Driver}}",
        "pluginsFormat": "table {{.Name}}\t{{.Enabled}}",
        "volumesFormat": "table {{.Name}}\t{{.Driver}}",
        "statsFormat": "table {{.Container}}\t{{.CPUPerc}}",
        "serviceInspectFormat": "json",
        "servicesFormat": "table {{.ID}}\t{{.Name}}",
        "tasksFormat": "table {{.ID}}\t{{.Name}}",
        "secretFormat": "table {{.ID}}\t{{.Name}}",
        "configFormat": "table {{.ID}}\t{{.Name}}",
        "nodesFormat": "table {{.ID}}\t{{.Hostname}}"
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
    assert_eq!(
        config.ps_format.as_deref(),
        Some("table {{.ID}}\t{{.Names}}")
    );
    assert_eq!(
        config.images_format.as_deref(),
        Some("table {{.Repository}}\t{{.Tag}}")
    );
    assert_eq!(
        config.networks_format.as_deref(),
        Some("table {{.Name}}\t{{.Driver}}")
    );
    assert_eq!(
        config.plugins_format.as_deref(),
        Some("table {{.Name}}\t{{.Enabled}}")
    );
    assert_eq!(
        config.volumes_format.as_deref(),
        Some("table {{.Name}}\t{{.Driver}}")
    );
    assert_eq!(
        config.stats_format.as_deref(),
        Some("table {{.Container}}\t{{.CPUPerc}}")
    );
    assert_eq!(config.service_inspect_format.as_deref(), Some("json"));
    assert_eq!(
        config.services_format.as_deref(),
        Some("table {{.ID}}\t{{.Name}}")
    );
    assert_eq!(
        config.tasks_format.as_deref(),
        Some("table {{.ID}}\t{{.Name}}")
    );
    assert_eq!(
        config.secret_format.as_deref(),
        Some("table {{.ID}}\t{{.Name}}")
    );
    assert_eq!(
        config.config_format.as_deref(),
        Some("table {{.ID}}\t{{.Name}}")
    );
    assert_eq!(
        config.nodes_format.as_deref(),
        Some("table {{.ID}}\t{{.Hostname}}")
    );
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
            email: None,
            server_address: None,
            registry_token: None,
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

#[test]
fn test_decode_base64_auth_with_special_chars_in_password() {
    // Passwords may contain special characters like @, :, /
    let encoded = base64::engine::general_purpose::STANDARD.encode("user:p@ss:w0rd/special");
    let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
    assert_eq!("user", user);
    assert_eq!("p@ss:w0rd/special", pass);
}

#[test]
fn test_decode_base64_auth_colon_in_password() {
    // splitn(2, ':') ensures only first colon is the separator
    let encoded = base64::engine::general_purpose::STANDARD.encode("user:pass:with:colons");
    let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
    assert_eq!("user", user);
    assert_eq!("pass:with:colons", pass);
}

#[test]
fn test_decode_base64_auth_empty_password() {
    let encoded = base64::engine::general_purpose::STANDARD.encode("user:");
    let (user, pass) = decode_base64_auth(&encoded).expect("should decode");
    assert_eq!("user", user);
    assert_eq!("", pass);
}

#[test]
fn test_decode_base64_auth_non_utf8_payload() {
    // Encode raw non-UTF8 bytes — base64 decode works but UTF-8 decode fails
    let raw: Vec<u8> = vec![0xff, 0xfe, 0x3a, 0x62]; // \xff\xfe:b
    let encoded = base64::engine::general_purpose::STANDARD.encode(&raw);
    let result = decode_base64_auth(&encoded);
    assert!(matches!(result, Err(_)));
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

    let config =
        Config::load_config(config_path.to_str().expect("should be valid")).expect("should load");
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
    assert!(config.ps_format.is_none());
    assert!(config.images_format.is_none());
    assert!(config.networks_format.is_none());
    assert!(config.plugins_format.is_none());
    assert!(config.volumes_format.is_none());
    assert!(config.stats_format.is_none());
    assert!(config.service_inspect_format.is_none());
    assert!(config.services_format.is_none());
    assert!(config.tasks_format.is_none());
    assert!(config.secret_format.is_none());
    assert!(config.config_format.is_none());
    assert!(config.nodes_format.is_none());
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
    assert!(matches!(result, Err(_)));
}
