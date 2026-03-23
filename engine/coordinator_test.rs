//! Coordinator and middleware tests
//!
//! Tests for coordinator creation, HTTP middleware (CORS, auth, rate limiting),
//! and utility functions.

use crate::coordinator::{
    Coordinator, CoordinatorImpl, Config, CoordinatorProxy,
    create_coordinator,
    BasicAuthConfig, KeyAuthConfig, RateLimitConfig, BodyLimitConfig, HttpLogConfig,
    basic_auth_layer, key_auth_layer, rate_limit_layer, body_limit_layer, http_log_layer, cors_layer,
    wildcard_match, check_password_hash, parse_body_limit,
    config_string, config_string_default, config_bool, config_bool_default,
    config_int_default, config_strings, config_strings_default,
    UsernameValue,
};
use crate::broker::BrokerProxy;
use crate::datastore::DatastoreProxy;
use std::sync::Arc;

// ── Coordinator trait tests ──────────────────────────────────────

#[tokio::test]
async fn test_coordinator_trait_start() {
    let config = Config::default();
    let coord = CoordinatorImpl::new(config);
    let result = coord.start().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_coordinator_trait_stop() {
    let config = Config::default();
    let coord = CoordinatorImpl::new(config);
    coord.start().await.expect("should start");
    let result = coord.stop().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_coordinator_trait_submit_job() {
    let config = Config::default();
    let coord = CoordinatorImpl::new(config);
    
    let job = tork::job::Job {
        id: Some("test-job".to_string()),
        ..Default::default()
    };
    
    let result = coord.submit_job(job).await;
    assert!(result.is_ok());
    let submitted = result.unwrap();
    assert_eq!(submitted.id.as_deref(), Some("test-job"));
}

// ── CoordinatorImpl tests ────────────────────────────────────────

#[test]
fn test_coordinator_impl_new() {
    let config = Config::default();
    let coord = CoordinatorImpl::new(config);
    // Just verify it constructs without panic
}

#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.name, "Coordinator");
    assert_eq!(config.address, "0.0.0.0:8000");
    assert!(config.queues.is_empty());
    assert!(config.enabled.is_empty());
}

#[test]
fn test_config_debug() {
    let config = Config::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("Coordinator"));
    assert!(debug_str.contains("0.0.0.0:8000"));
}

// ── create_coordinator tests ─────────────────────────────────────

#[tokio::test]
async fn test_create_coordinator() {
    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();
    
    let result = create_coordinator(broker, datastore).await;
    assert!(result.is_ok());
    
    let coord = result.unwrap();
    let started = coord.start().await;
    assert!(started.is_ok());
    
    let stopped = coord.stop().await;
    assert!(stopped.is_ok());
}

// ── UsernameValue tests ──────────────────────────────────────────

#[test]
fn test_username_value_clone() {
    let username = UsernameValue("testuser".to_string());
    let cloned = username.clone();
    assert_eq!(cloned.0, "testuser");
}

#[test]
fn test_username_value_debug() {
    let username = UsernameValue("testuser".to_string());
    let debug_str = format!("{:?}", username);
    assert!(debug_str.contains("testuser"));
}

// ── Wildcard matching tests ──────────────────────────────────────

#[test]
fn test_wildcard_match_exact() {
    assert!(wildcard_match("abc", "abc"));
    assert!(!wildcard_match("abc", "abd"));
    assert!(!wildcard_match("abc", "ab"));
    assert!(!wildcard_match("abc", "abcd"));
}

#[test]
fn test_wildcard_match_star_only() {
    assert!(wildcard_match("*", ""));
    assert!(wildcard_match("*", "anything"));
    assert!(wildcard_match("*", "multiple words here"));
}

#[test]
fn test_wildcard_match_star_prefix() {
    assert!(wildcard_match("*c", "abc"));
    assert!(wildcard_match("*c", "defc"));
    assert!(!wildcard_match("*c", "defd"));
}

#[test]
fn test_wildcard_match_star_suffix() {
    assert!(wildcard_match("a*", "abc"));
    assert!(wildcard_match("a*", "a"));
    assert!(!wildcard_match("a*", "b"));
}

#[test]
fn test_wildcard_match_star_middle() {
    assert!(wildcard_match("a*c", "abc"));
    assert!(wildcard_match("a*c", "aXXc"));
    assert!(wildcard_match("a*c", "alongstringc"));
    assert!(!wildcard_match("a*c", "aXXd"));
}

#[test]
fn test_wildcard_match_multiple_stars() {
    assert!(wildcard_match("*:*", "foo:bar"));
    assert!(wildcard_match("a*b*c", "axbxc"));
    assert!(wildcard_match("a*b*c", "axxbxxc"));
    assert!(!wildcard_match("a*b*c", "axbxd"));
}

#[test]
fn test_wildcard_match_empty_pattern() {
    assert!(wildcard_match("", ""));
    assert!(!wildcard_match("", "a"));
}

#[test]
fn test_wildcard_match_complex() {
    assert!(wildcard_match("GET *", "GET /api/users"));
    assert!(wildcard_match("POST *", "POST /api/users"));
    assert!(!wildcard_match("GET *", "POST /api/users"));
}

// ── Password hashing tests ────────────────────────────────────────

#[test]
fn test_check_password_hash_correct() {
    let hash = bcrypt::hash("secretpassword", 4).expect("should hash");
    assert!(check_password_hash("secretpassword", &hash));
}

#[test]
fn test_check_password_hash_incorrect() {
    let hash = bcrypt::hash("secretpassword", 4).expect("should hash");
    assert!(!check_password_hash("wrongpassword", &hash));
}

#[test]
fn test_check_password_hash_empty_password() {
    let hash = bcrypt::hash("secretpassword", 4).expect("should hash");
    assert!(!check_password_hash("", &hash));
}

#[test]
fn test_check_password_hash_invalid_hash() {
    assert!(!check_password_hash("password", "invalid_hash_format"));
    assert!(!check_password_hash("", ""));
}

// ── Body limit parsing tests ─────────────────────────────────────

#[test]
fn test_parse_body_limit_bytes() {
    assert_eq!(parse_body_limit("500"), Some(500));
    assert_eq!(parse_body_limit("0"), Some(0));
    assert_eq!(parse_body_limit("1000"), Some(1000));
}

#[test]
fn test_parse_body_limit_kilobytes() {
    assert_eq!(parse_body_limit("500K"), Some(500 * 1024));
    assert_eq!(parse_body_limit("1K"), Some(1024));
    assert_eq!(parse_body_limit("10K"), Some(10 * 1024));
}

#[test]
fn test_parse_body_limit_megabytes() {
    assert_eq!(parse_body_limit("1M"), Some(1024 * 1024));
    assert_eq!(parse_body_limit("5M"), Some(5 * 1024 * 1024));
    assert_eq!(parse_body_limit("100M"), Some(100 * 1024 * 1024));
}

#[test]
fn test_parse_body_limit_gigabytes() {
    assert_eq!(parse_body_limit("1G"), Some(1024 * 1024 * 1024));
    assert_eq!(parse_body_limit("2G"), Some(2 * 1024 * 1024 * 1024));
}

#[test]
fn test_parse_body_limit_empty() {
    assert_eq!(parse_body_limit(""), None);
}

#[test]
fn test_parse_body_limit_invalid() {
    assert_eq!(parse_body_limit("invalid"), None);
    assert_eq!(parse_body_limit("K"), None); // no number before suffix
    assert_eq!(parse_body_limit("M"), None);
}

// ── Config helpers tests ─────────────────────────────────────────

#[test]
fn test_config_string_unset() {
    std::env::remove_var("TORK_TEST_COORD_UNSET");
    assert_eq!(config_string("test.coord.unset"), "");
}

#[test]
fn test_config_string_set() {
    std::env::set_var("TORK_TEST_COORD_SET", "coord_value");
    assert_eq!(config_string("test.coord.set"), "coord_value");
    std::env::remove_var("TORK_TEST_COORD_SET");
}

#[test]
fn test_config_string_dots_to_underscores() {
    std::env::set_var("TORK_TEST_COORD_DOT_VALUE", "dot_value");
    assert_eq!(config_string("test.coord.dot.value"), "dot_value");
    std::env::remove_var("TORK_TEST_COORD_DOT_VALUE");
}

#[test]
fn test_config_string_default_empty() {
    assert_eq!(config_string_default("test.coord.default", "fallback"), "fallback");
}

#[test]
fn test_config_string_default_set() {
    std::env::set_var("TORK_TEST_COORD_DEFAULT", "custom_coord");
    assert_eq!(config_string_default("test.coord.default", "fallback"), "custom_coord");
    std::env::remove_var("TORK_TEST_COORD_DEFAULT");
}

#[test]
fn test_config_bool_true() {
    std::env::set_var("TORK_TEST_COORD_BOOL_TRUE", "true");
    assert!(config_bool("test.coord.bool.true"));
    std::env::remove_var("TORK_TEST_COORD_BOOL_TRUE");
}

#[test]
fn test_config_bool_false() {
    std::env::set_var("TORK_TEST_COORD_BOOL_FALSE", "false");
    assert!(!config_bool("test.coord.bool.false"));
    std::env::remove_var("TORK_TEST_COORD_BOOL_FALSE");
}

#[test]
fn test_config_bool_one() {
    std::env::set_var("TORK_TEST_COORD_BOOL_ONE", "1");
    assert!(config_bool("test.coord.bool.one"));
    std::env::remove_var("TORK_TEST_COORD_BOOL_ONE");
}

#[test]
fn test_config_bool_default_false() {
    std::env::remove_var("TORK_TEST_COORD_BOOL_DEFAULT_FALSE");
    assert!(!config_bool_default("test.coord.bool.default.false", false));
}

#[test]
fn test_config_bool_default_true() {
    std::env::remove_var("TORK_TEST_COORD_BOOL_DEFAULT_TRUE");
    assert!(config_bool_default("test.coord.bool.default.true", true));
}

#[test]
fn test_config_int_default_zero() {
    std::env::remove_var("TORK_TEST_COORD_INT_ZERO");
    assert_eq!(config_int_default("test.coord.int.zero", 42), 42);
}

#[test]
fn test_config_int_default_set() {
    std::env::set_var("TORK_TEST_COORD_INT_SET", "100");
    assert_eq!(config_int_default("test.coord.int.set", 42), 100);
    std::env::remove_var("TORK_TEST_COORD_INT_SET");
}

#[test]
fn test_config_strings_empty() {
    std::env::remove_var("TORK_TEST_COORD_STRINGS_EMPTY");
    let strings = config_strings("test.coord.strings.empty");
    assert!(strings.is_empty());
}

#[test]
fn test_config_strings_comma_separated() {
    std::env::set_var("TORK_TEST_COORD_STRINGS_COMMA", "a,b,c");
    let strings = config_strings("test.coord.strings.comma");
    assert_eq!(strings, vec!["a", "b", "c"]);
    std::env::remove_var("TORK_TEST_COORD_STRINGS_COMMA");
}

#[test]
fn test_config_strings_with_spaces() {
    std::env::set_var("TORK_TEST_COORD_STRINGS_SPACES", "a, b, c");
    let strings = config_strings("test.coord.strings.spaces");
    assert_eq!(strings, vec!["a", "b", "c"]);
    std::env::remove_var("TORK_TEST_COORD_STRINGS_SPACES");
}

#[test]
fn test_config_strings_array_format() {
    std::env::set_var("TORK_TEST_COORD_STRINGS_ARRAY", r#"["x", "y", "z"]"#);
    let strings = config_strings("test.coord.strings.array");
    assert_eq!(strings, vec!["x", "y", "z"]);
    std::env::remove_var("TORK_TEST_COORD_STRINGS_ARRAY");
}

#[test]
fn test_config_strings_default_empty() {
    std::env::remove_var("TORK_TEST_COORD_STRINGS_DEFAULT");
    let strings = config_strings_default("test.coord.strings.default", &["a", "b"]);
    assert_eq!(strings, vec!["a", "b"]);
}

#[test]
fn test_config_strings_default_set() {
    std::env::set_var("TORK_TEST_COORD_STRINGS_DEFAULT", "x,y");
    let strings = config_strings_default("test.coord.strings.default", &["a", "b"]);
    assert_eq!(strings, vec!["x", "y"]);
    std::env::remove_var("TORK_TEST_COORD_STRINGS_DEFAULT");
}

// ── Middleware config tests ──────────────────────────────────────

#[test]
fn test_basic_auth_config_new() {
    let ds = crate::datastore::new_inmemory_datastore_arc();
    let config = BasicAuthConfig::new(ds);
    // Construction succeeds without panic
}

#[test]
fn test_key_auth_config_new() {
    let config = KeyAuthConfig::new("test-key".to_string());
    assert_eq!(config.key, "test-key");
}

#[test]
fn test_key_auth_config_with_skip_paths() {
    let config = KeyAuthConfig::new("key".to_string())
        .with_skip_paths(vec!["GET /health".to_string()]);
    assert!(config.skip_paths.contains(&"GET /health".to_string()));
}

#[test]
fn test_rate_limit_config_new() {
    let config = RateLimitConfig::new(50);
    assert_eq!(config.rps, 50);
}

#[test]
fn test_rate_limit_config_zero() {
    let config = RateLimitConfig::new(0);
    assert_eq!(config.rps, 0);
}

#[test]
fn test_body_limit_config_new() {
    let config = BodyLimitConfig::new(1024);
    assert_eq!(config.limit, 1024);
}

#[test]
fn test_http_log_config_default() {
    let config = HttpLogConfig::default();
    assert_eq!(config.level, "DEBUG");
    assert!(config.skip_paths.contains(&"GET /health".to_string()));
}

#[test]
fn test_http_log_config_new() {
    let config = HttpLogConfig::new();
    assert_eq!(config.level, "DEBUG");
}

#[test]
fn test_http_log_config_with_level() {
    let config = HttpLogConfig::new().with_level("INFO");
    assert_eq!(config.level, "INFO");
}

#[test]
fn test_http_log_config_with_skip_paths() {
    let config = HttpLogConfig::new()
        .with_skip_paths(vec!["POST /api".to_string()]);
    assert_eq!(config.skip_paths, vec!["POST /api".to_string()]);
}

// ── Middleware layer creation tests ──────────────────────────────

#[test]
fn test_cors_layer_creation() {
    let _layer = cors_layer();
    // CorsLayer constructs without panic
}

#[test]
fn test_basic_auth_layer_creation() {
    let ds = crate::datastore::new_inmemory_datastore_arc();
    let config = BasicAuthConfig::new(ds);
    let _layer = basic_auth_layer(config);
    // Layer constructs without panic
}

#[test]
fn test_key_auth_layer_creation() {
    let config = KeyAuthConfig::new("test-key".to_string());
    let _layer = key_auth_layer(config);
    // Layer constructs without panic
}

#[test]
fn test_rate_limit_layer_creation() {
    let config = RateLimitConfig::new(10);
    let _layer = rate_limit_layer(config);
    // Layer constructs without panic
}

#[test]
fn test_body_limit_layer_creation() {
    let config = BodyLimitConfig::new(2048);
    let _layer = body_limit_layer(config);
    // Layer constructs without panic
}

#[test]
fn test_http_log_layer_creation() {
    let config = HttpLogConfig::new();
    let _layer = http_log_layer(config);
    // Layer constructs without panic
}

// ── InMemoryLocker tests ────────────────────────────────────────

use crate::coordinator::InMemoryLocker;

#[tokio::test]
async fn test_in_memory_locker_new() {
    let locker = InMemoryLocker::new();
    let result = locker.acquire_lock("test-key").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_locker_default() {
    let locker = InMemoryLocker::default();
    let result = locker.acquire_lock("test-key").await;
    assert!(result.is_ok());
}