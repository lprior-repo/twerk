//! Locker module tests
//!
//! Tests for locker creation and configuration helpers.

use crate::locker::{
    create_locker, 
    env_string, env_string_default, env_int_default,
};
use crate::locker::InMemoryLocker;
use locker::Locker;

#[tokio::test]
async fn test_create_locker_inmemory() {
    let result = create_locker("inmemory").await;
    assert!(result.is_ok());
    let locker = result.unwrap();
    // Verify we can acquire a lock - result is Pin<Box<dyn Lock>>
    let lock_result = locker.acquire_lock("test-key").await;
    assert!(lock_result.is_ok());
}

#[tokio::test]
async fn test_create_locker_unknown_type() {
    let result = create_locker("unknown").await;
    assert!(result.is_err());
}

// ── InMemoryLocker tests ────────────────────────────────────────

#[tokio::test]
async fn test_inmemory_locker_new() {
    let locker = InMemoryLocker::new();
    // acquire_lock returns Result<Pin<Box<dyn Lock>>, LockError>
    let result = locker.acquire_lock("test-key").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_inmemory_locker_default() {
    let locker = InMemoryLocker::default();
    let result = locker.acquire_lock("test-key").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_inmemory_locker_multiple_locks() {
    let locker = InMemoryLocker::new();
    // Acquire two different locks - should succeed
    let result1 = locker.acquire_lock("key1").await;
    let result2 = locker.acquire_lock("key2").await;
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    
    // Release key1, then acquire it again - should succeed
    if let Ok(lock1) = result1 {
        lock1.release_lock().await.expect("release should succeed");
    }
    let result3 = locker.acquire_lock("key1").await;
    assert!(result3.is_ok());
}

// ── Config helper tests ──────────────────────────────────────────

#[test]
fn test_env_string_unset() {
    std::env::remove_var("TORK_LOCKER_STRING_UNSET");
    assert_eq!(env_string("locker.string.unset"), "");
}

#[test]
fn test_env_string_set() {
    std::env::set_var("TORK_LOCKER_STRING_SET", "locker_value");
    assert_eq!(env_string("locker.string.set"), "locker_value");
    std::env::remove_var("TORK_LOCKER_STRING_SET");
}

#[test]
fn test_env_string_dots_to_underscores() {
    std::env::set_var("TORK_LOCKER_DOT_VALUE", "dot_value");
    assert_eq!(env_string("locker.dot.value"), "dot_value");
    std::env::remove_var("TORK_LOCKER_DOT_VALUE");
}

#[test]
fn test_env_string_default_empty() {
    std::env::remove_var("TORK_LOCKER_DEFAULT_EMPTY");
    assert_eq!(env_string_default("locker.default.empty", "fallback"), "fallback");
}

#[test]
fn test_env_string_default_set() {
    std::env::set_var("TORK_LOCKER_DEFAULT_SET", "custom_locker");
    assert_eq!(env_string_default("locker.default.set", "fallback"), "custom_locker");
    std::env::remove_var("TORK_LOCKER_DEFAULT_SET");
}

#[test]
fn test_env_int_default_zero() {
    std::env::remove_var("TORK_LOCKER_INT_ZERO");
    assert_eq!(env_int_default("locker.int.zero", 42), 42);
}

#[test]
fn test_env_int_default_set() {
    std::env::set_var("TORK_LOCKER_INT_SET", "100");
    assert_eq!(env_int_default("locker.int.set", 42), 100);
    std::env::remove_var("TORK_LOCKER_INT_SET");
}

#[test]
fn test_env_int_default_invalid() {
    std::env::set_var("TORK_LOCKER_INT_INVALID", "not_a_number");
    assert_eq!(env_int_default("locker.int.invalid", 42), 42);
    std::env::remove_var("TORK_LOCKER_INT_INVALID");
}

// ── Locker type constant tests ─────────────────────────────────

#[test]
fn test_locker_type_constants() {
    // These should be available from the locker crate
    assert_eq!(crate::locker::LOCKER_INMEMORY, "inmemory");
    assert_eq!(crate::locker::LOCKER_POSTGRES, "postgres");
}
