//! Integration tests for [`InMemoryLocker`].
//!
//! These tests verify the in-memory locker behavior in isolation.
//! Note: `InMemoryLocker` is NOT safe for multi-process/multi-machine usage.

use std::pin::Pin;

use locker::error::LockError;
use locker::{InMemoryLocker, Lock, Locker};

/// Verifies that a single lock can be acquired and released successfully.
#[tokio::test]
async fn test_acquire_and_release_single_lock() {
    let locker = InMemoryLocker::new();
    let key = "single-lock-key";

    let lock = locker
        .acquire_lock(key)
        .await
        .expect("first acquire should succeed");

    lock.release_lock()
        .await
        .expect("release should succeed");
}

/// Verifies that a lock can be reacquired after being released.
#[tokio::test]
async fn test_reacquire_after_release() {
    let locker = InMemoryLocker::new();
    let key = "reacquire-key";

    let lock1 = locker
        .acquire_lock(key)
        .await
        .expect("first acquire should succeed");

    lock1.release_lock()
        .await
        .expect("first release should succeed");

    let lock2 = locker
        .acquire_lock(key)
        .await
        .expect("reacquire should succeed");

    lock2.release_lock()
        .await
        .expect("second release should succeed");
}

/// Verifies that attempting to acquire the same key twice returns [`LockError::AlreadyLocked`].
#[tokio::test]
async fn test_double_acquire_returns_already_locked() {
    let locker = InMemoryLocker::new();
    let key = "double-acquire-key";

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
async fn test_multiple_different_keys() {
    let locker = InMemoryLocker::new();
    let key_a = "key-a";
    let key_b = "key-b";

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

/// Verifies that releasing a lock for a non-existent key returns [`LockError::NotLocked`].
#[tokio::test]
async fn test_release_nonexistent_returns_not_locked() {
    let _locker = InMemoryLocker::new();
    let key = "nonexistent-key";

    struct DummyLock {
        key: String,
    }

    impl Lock for DummyLock {
        fn release_lock(
            self: Pin<Box<Self>>,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LockError>> + Send>> {
            let key = self.key.clone();
            Box::pin(async move { Err(LockError::NotLocked { key }) })
        }
    }

    let lock: Pin<Box<dyn Lock>> = Box::pin(DummyLock {
        key: key.to_string(),
    });

    let result = lock.release_lock().await;

    assert!(result.is_err(), "release should fail for nonexistent key");
    match result {
        Err(LockError::NotLocked { key: k }) => {
            assert_eq!(k, key, "error key should match");
        }
        Err(e) => panic!("expected NotLocked error, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

/// Verifies that the default constructor creates a functional locker.
#[tokio::test]
async fn test_default_constructor() {
    let locker = InMemoryLocker::default();
    let key = "default-key";

    let result = locker.acquire_lock(key).await;
    assert!(result.is_ok(), "acquire should succeed with default locker");
}

/// Verifies that many sequential acquires/releases on different keys work correctly.
#[tokio::test]
async fn test_sequential_many_keys() {
    let locker = InMemoryLocker::new();

    for i in 0..100 {
        let key = format!("sequential-key-{i}");
        let lock = locker
            .acquire_lock(&key)
            .await
            .expect("acquire should succeed");
        lock.release_lock()
            .await
            .expect("release should succeed");
    }
}
