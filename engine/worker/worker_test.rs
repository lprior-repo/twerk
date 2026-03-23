//! Worker module tests
//!
//! Tests for worker initialization, runtime creation, and configuration helpers.

use crate::worker::{
    config_string, config_string_default, config_bool, config_strings,
    runtime_type, RuntimeConfig, BindConfig,
    create_runtime_from_config, read_runtime_config,
    create_hostenv_middleware,
    is_none_or_empty, is_none_or_empty_vec,
};
use std::collections::HashMap;

#[test]
fn test_runtime_type_constants() {
    assert_eq!(runtime_type::DOCKER, "docker");
    assert_eq!(runtime_type::SHELL, "shell");
    assert_eq!(runtime_type::PODMAN, "podman");
    assert_eq!(runtime_type::DEFAULT, "docker");
}

#[test]
fn test_runtime_config_default() {
    let config = RuntimeConfig::default();
    assert_eq!(config.runtime_type, "docker");
    assert!(!config.docker_privileged);
    assert_eq!(config.docker_image_ttl_secs, 72 * 60 * 60);
    assert!(!config.docker_image_verify);
    assert!(config.docker_config.is_empty());
    assert_eq!(config.shell_cmd, vec!["bash".to_string(), "-c".to_string()]);
    assert_eq!(config.shell_uid, "-");
    assert_eq!(config.shell_gid, "-");
    assert!(!config.podman_privileged);
    assert!(!config.podman_host_network);
    assert!(!config.bind_allowed);
    assert!(config.bind_sources.is_empty());
    assert!(config.hostenv_vars.is_empty());
}

#[test]
fn test_runtime_config_debug() {
    let config = RuntimeConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("docker"));
}

#[test]
fn test_bind_config_default() {
    let config = BindConfig::default();
    assert!(!config.allowed);
    assert!(config.sources.is_empty());
}

#[test]
fn test_bind_config_with_sources() {
    let config = BindConfig {
        allowed: true,
        sources: vec!["/data".to_string(), "/config".to_string()],
    };
    assert!(config.allowed);
    assert_eq!(config.sources.len(), 2);
}

#[test]
fn test_is_none_or_empty_true_cases() {
    assert!(is_none_or_empty(&None));
    assert!(is_none_or_empty(&Some(String::new())));
    assert!(is_none_or_empty(&Some("".to_string())));
}

#[test]
fn test_is_none_or_empty_false_cases() {
    assert!(!is_none_or_empty(&Some("value".to_string())));
    assert!(!is_none_or_empty(&Some("  ".to_string())));
}

#[test]
fn test_is_none_or_empty_vec_true_cases() {
    assert!(is_none_or_empty_vec::<String>(&None));
    assert!(is_none_or_empty_vec(&Some(vec![])));
}

#[test]
fn test_is_none_or_empty_vec_false_cases() {
    assert!(!is_none_or_empty_vec(&Some(vec!["item".to_string()])));
}

// ── Config helper tests ──────────────────────────────────────────

#[test]
fn test_config_string_unset() {
    std::env::remove_var("TORK_WORKER_TEST_UNSET");
    assert_eq!(config_string("worker.test.unset"), "");
}

#[test]
fn test_config_string_set() {
    std::env::set_var("TORK_WORKER_TEST_SET", "test_value");
    assert_eq!(config_string("worker.test.set"), "test_value");
    std::env::remove_var("TORK_WORKER_TEST_SET");
}

#[test]
fn test_config_string_dots_to_underscores() {
    std::env::set_var("TORK_WORKER_TEST_DOT_VALUE", "dot_value");
    assert_eq!(config_string("worker.test.dot.value"), "dot_value");
    std::env::remove_var("TORK_WORKER_TEST_DOT_VALUE");
}

#[test]
fn test_config_string_default_empty() {
    std::env::remove_var("TORK_WORKER_TEST_DEFAULT");
    assert_eq!(config_string_default("worker.test.default", "fallback"), "fallback");
}

#[test]
fn test_config_string_default_set() {
    std::env::set_var("TORK_WORKER_TEST_DEFAULT", "custom");
    assert_eq!(config_string_default("worker.test.default", "fallback"), "custom");
    std::env::remove_var("TORK_WORKER_TEST_DEFAULT");
}

#[test]
fn test_config_bool_true() {
    std::env::set_var("TORK_WORKER_TEST_BOOL_TRUE", "true");
    assert!(config_bool("worker.test.bool.true"));
    std::env::remove_var("TORK_WORKER_TEST_BOOL_TRUE");
}

#[test]
fn test_config_bool_false() {
    std::env::set_var("TORK_WORKER_TEST_BOOL_FALSE", "false");
    assert!(!config_bool("worker.test.bool.false"));
    std::env::remove_var("TORK_WORKER_TEST_BOOL_FALSE");
}

#[test]
fn test_config_bool_one() {
    std::env::set_var("TORK_WORKER_TEST_BOOL_ONE", "1");
    assert!(config_bool("worker.test.bool.one"));
    std::env::remove_var("TORK_WORKER_TEST_BOOL_ONE");
}

#[test]
fn test_config_strings_empty() {
    std::env::remove_var("TORK_WORKER_TEST_STRINGS_EMPTY");
    let strings = config_strings("worker.test.strings.empty");
    assert!(strings.is_empty());
}

#[test]
fn test_config_strings_comma_separated() {
    std::env::set_var("TORK_WORKER_TEST_STRINGS_COMMA", "a,b,c");
    let strings = config_strings("worker.test.strings.comma");
    assert_eq!(strings, vec!["a", "b", "c"]);
    std::env::remove_var("TORK_WORKER_TEST_STRINGS_COMMA");
}

#[test]
fn test_config_strings_with_spaces() {
    std::env::set_var("TORK_WORKER_TEST_STRINGS_SPACES", "a, b, c");
    let strings = config_strings("worker.test.strings.spaces");
    assert_eq!(strings, vec!["a", "b", "c"]);
    std::env::remove_var("TORK_WORKER_TEST_STRINGS_SPACES");
}

#[test]
fn test_config_strings_array_format() {
    std::env::set_var("TORK_WORKER_TEST_STRINGS_ARRAY", r#"["x", "y", "z"]"#);
    let strings = config_strings("worker.test.strings.array");
    assert_eq!(strings, vec!["x", "y", "z"]);
    std::env::remove_var("TORK_WORKER_TEST_STRINGS_ARRAY");
}

// ── Hostenv middleware tests ────────────────────────────────────

#[test]
fn test_create_hostenv_middleware_empty() {
    let result = create_hostenv_middleware(&[]);
    assert!(result.is_none());
}

#[test]
fn test_create_hostenv_middleware_single_var() {
    let result = create_hostenv_middleware(&["PATH".to_string()]);
    assert!(result.is_some());
    let boxed = result.unwrap();
    let map = boxed.downcast_ref::<HashMap<String, String>>();
    assert!(map.is_some());
    let map = map.unwrap();
    assert_eq!(map.get("PATH"), Some(&"PATH".to_string()));
}

#[test]
fn test_create_hostenv_middleware_with_alias() {
    let result = create_hostenv_middleware(&["HOST_VAR:TASK_VAR".to_string()]);
    assert!(result.is_some());
    let boxed = result.unwrap();
    let map = boxed.downcast_ref::<HashMap<String, String>>();
    assert!(map.is_some());
    let map = map.unwrap();
    assert_eq!(map.get("HOST_VAR"), Some(&"TASK_VAR".to_string()));
}

#[test]
fn test_create_hostenv_middleware_multiple() {
    let vars = vec![
        "HOME".to_string(),
        "HOST_A: TASK_A".to_string(),
        "HOST_B:TASK_B".to_string(),
    ];
    let result = create_hostenv_middleware(&vars);
    assert!(result.is_some());
    let boxed = result.unwrap();
    let map = boxed.downcast_ref::<HashMap<String, String>>();
    assert!(map.is_some());
    let map = map.unwrap();
    assert_eq!(map.len(), 3);
}

#[test]
fn test_create_hostenv_middleware_empty_var_ignored() {
    let vars = vec![
        "VALID".to_string(),
        "".to_string(),
        "ALSO_VALID".to_string(),
    ];
    let result = create_hostenv_middleware(&vars);
    assert!(result.is_some());
    let boxed = result.unwrap();
    let map = boxed.downcast_ref::<HashMap<String, String>>();
    assert!(map.is_some());
    let map = map.unwrap();
    assert_eq!(map.len(), 2);
}

#[test]
fn test_create_hostenv_middleware_invalid_format_ignored() {
    let vars = vec![
        "VALID".to_string(),
        ":".to_string(),
        "ALSO_VALID".to_string(),
    ];
    let result = create_hostenv_middleware(&vars);
    assert!(result.is_some());
    let boxed = result.unwrap();
    let map = boxed.downcast_ref::<HashMap<String, String>>();
    assert!(map.is_some());
    let map = map.unwrap();
    // ":" splits into ["", ""] which is invalid since both are empty
    assert_eq!(map.len(), 2);
}

// ── Runtime creation tests ─────────────────────────────────────

#[tokio::test]
async fn test_create_runtime_from_config_docker() {
    let config = RuntimeConfig {
        runtime_type: runtime_type::DOCKER.to_string(),
        ..Default::default()
    };
    let result = create_runtime_from_config(&config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_runtime_from_config_shell() {
    let config = RuntimeConfig {
        runtime_type: runtime_type::SHELL.to_string(),
        shell_cmd: vec!["bash".to_string(), "-c".to_string()],
        ..Default::default()
    };
    let result = create_runtime_from_config(&config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_runtime_from_config_podman() {
    let config = RuntimeConfig {
        runtime_type: runtime_type::PODMAN.to_string(),
        ..Default::default()
    };
    let result = create_runtime_from_config(&config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_runtime_from_config_unknown() {
    let config = RuntimeConfig {
        runtime_type: "unknown".to_string(),
        ..Default::default()
    };
    let result = create_runtime_from_config(&config).await;
    assert!(result.is_err());
}

// ── Read runtime config tests ─────────────────────────────────

#[test]
fn test_read_runtime_config_default() {
    // Clear relevant env vars first
    std::env::remove_var("TORK_RUNTIME_TYPE");
    std::env::remove_var("TORK_RUNTIME_DOCKER_PRIVILEGED");
    std::env::remove_var("TORK_RUNTIME_SHELL_CMD");
    std::env::remove_var("TORK_MOUNTS_BIND_ALLOWED");
    
    let config = read_runtime_config();
    assert_eq!(config.runtime_type, "docker");
    assert!(!config.docker_privileged);
}

#[test]
fn test_read_runtime_config_custom_shell() {
    std::env::set_var("TORK_RUNTIME_TYPE", "shell");
    std::env::set_var("TORK_RUNTIME_SHELL_CMD", "sh,-c");
    std::env::set_var("TORK_RUNTIME_SHELL_UID", "1000");
    std::env::set_var("TORK_RUNTIME_SHELL_GID", "1000");
    
    let config = read_runtime_config();
    assert_eq!(config.runtime_type, "shell");
    assert!(config.shell_cmd.contains(&"sh".to_string()));
    assert_eq!(config.shell_uid, "1000");
    assert_eq!(config.shell_gid, "1000");
    
    std::env::remove_var("TORK_RUNTIME_TYPE");
    std::env::remove_var("TORK_RUNTIME_SHELL_CMD");
    std::env::remove_var("TORK_RUNTIME_SHELL_UID");
    std::env::remove_var("TORK_RUNTIME_SHELL_GID");
}

#[test]
fn test_read_runtime_config_bind_mounts() {
    std::env::set_var("TORK_MOUNTS_BIND_ALLOWED", "true");
    std::env::set_var("TORK_MOUNTS_BIND_SOURCES", "/data,/config");
    
    let config = read_runtime_config();
    assert!(config.bind_allowed);
    assert_eq!(config.bind_sources, vec!["/data", "/config"]);
    
    std::env::remove_var("TORK_MOUNTS_BIND_ALLOWED");
    std::env::remove_var("TORK_MOUNTS_BIND_SOURCES");
}
