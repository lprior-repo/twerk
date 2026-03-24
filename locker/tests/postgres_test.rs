//! Integration tests for [`PostgresLocker`].
//!
//! These tests require a running PostgreSQL instance.
//! Use the `DATABASE_URL` environment variable to configure the connection.
//!
//! # Running Tests
//!
//! To run all tests including database-dependent ones:
//! ```bash
//! DATABASE_URL="postgres://tork:tork@localhost:5432/tork" cargo test --test postgres_test
//! ```
//!
//! To run only unit tests (no database required):
//! ```bash
//! cargo test --test postgres_test -- --skip test_postgres_locker_creation --skip test_acquire_and_release --skip test_reacquire --skip test_double_acquire --skip test_multiple_different --skip test_sequential_many
//! ```

use locker::error::LockError;
use locker::{hash_key, Locker, PostgresLocker};

/// Verifies that `hash_key` produces deterministic results.
#[test]
fn test_hash_key_deterministic() {
    let key = "deterministic-key";
    let hash_a = hash_key(key);
    let hash_b = hash_key(key);
    assert_eq!(
        hash_a, hash_b,
        "hash_key should produce the same result for the same key"
    );
}

/// Verifies that `hash_key` produces different results for different keys.
#[test]
fn test_hash_key_different_keys_different_hashes() {
    let hash_a = hash_key("key-alpha");
    let hash_b = hash_key("key-beta");
    assert_ne!(
        hash_a, hash_b,
        "hash_key should produce different results for different keys"
    );
}

/// Verifies that `hash_key` matches a known reference value.
#[test]
fn test_hash_key_reference_value() {
    let key = "2c7eb7e1951343468ce360c906003a22";
    let hash = hash_key(key);
    // Go reference: int64(-414568140838410356)
    let expected = i64::from_be_bytes([250, 63, 40, 120, 238, 33, 231, 140]);
    assert_eq!(hash, expected, "hash_key should match Go reference value");
}

/// Verifies that a [`PostgresLocker`] can be created with a valid DSN.
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_postgres_locker_creation() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let result = PostgresLocker::new(&dsn).await;
    assert!(
        result.is_ok(),
        "PostgresLocker creation should succeed with valid DSN: {:?}",
        result.err()
    );
}

/// Verifies that acquiring a lock and releasing it works correctly.
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_acquire_and_release_single_lock() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let locker = PostgresLocker::new(&dsn)
        .await
        .expect("locker should be created");
    let key = "pg-single-lock";

    let lock = locker
        .acquire_lock(key)
        .await
        .expect("first acquire should succeed");

    lock.release_lock().await.expect("release should succeed");
}

/// Verifies that a lock can be reacquired after being released.
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_reacquire_after_release() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let locker = PostgresLocker::new(&dsn)
        .await
        .expect("locker should be created");
    let key = "pg-reacquire-key";

    let lock1 = locker
        .acquire_lock(key)
        .await
        .expect("first acquire should succeed");

    lock1
        .release_lock()
        .await
        .expect("first release should succeed");

    let lock2 = locker
        .acquire_lock(key)
        .await
        .expect("reacquire should succeed");

    lock2
        .release_lock()
        .await
        .expect("second release should succeed");
}

/// Verifies that attempting to acquire the same key twice returns [`LockError::AlreadyLocked`].
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_double_acquire_returns_already_locked() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let locker = PostgresLocker::new(&dsn)
        .await
        .expect("locker should be created");
    let key = "pg-double-acquire";

    let _lock = locker
        .acquire_lock(key)
        .await
        .expect("first acquire should succeed");

    let result = locker.acquire_lock(key).await;

    assert!(result.is_err(), "second acquire should fail");
    match result {
        Err(LockError::AlreadyLocked { key: k }) => {
            assert_eq!(k, key, "error key should match");
        }
        Err(e) => panic!("expected AlreadyLocked error, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Verifies that different keys can be acquired simultaneously.
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_multiple_different_keys() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let locker = PostgresLocker::new(&dsn)
        .await
        .expect("locker should be created");
    let key_a = "pg-key-alpha";
    let key_b = "pg-key-beta";

    let lock_a = locker
        .acquire_lock(key_a)
        .await
        .expect("acquire key_a should succeed");

    let lock_b = locker
        .acquire_lock(key_b)
        .await
        .expect("acquire key_b should succeed");

    assert!(
        lock_a.release_lock().await.is_ok(),
        "release lock_a should succeed"
    );
    assert!(
        lock_b.release_lock().await.is_ok(),
        "release lock_b should succeed"
    );
}

/// Verifies that many sequential acquires/releases on different keys work correctly.
#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_sequential_many_keys() {
    let dsn = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tork:tork@localhost:5432/tork".to_string());

    let locker = PostgresLocker::new(&dsn)
        .await
        .expect("locker should be created");

    for i in 0..50 {
        let key = format!("pg-sequential-key-{i}");
        let lock = locker
            .acquire_lock(&key)
            .await
            .expect("acquire should succeed");
        lock.release_lock().await.expect("release should succeed");
    }
}
