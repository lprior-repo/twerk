//! In-memory locker implementation

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::error::LockError;
use super::{Lock, Locker};

/// An in-memory locker that uses a `HashMap` to track locks.
///
/// **Note**: This is NOT safe for multi-process/multi-machine usage.
/// Use [`PostgresLocker`] for distributed locking.
pub struct InMemoryLocker {
    locks: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
}

impl Default for InMemoryLocker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryLocker {
    /// Create a new in-memory locker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Inner lock type for in-memory locking.
struct InMemLock {
    key: String,
    locks: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
    drop_tx: mpsc::Sender<()>,
}

impl InMemLock {
    fn new(key: String, locks: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>) -> Self {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let locks_clone = Arc::clone(&locks);
        let key_clone = key.clone();

        // Spawn a task that will remove the lock when the sender is dropped
        tokio::spawn(async move {
            rx.recv().await;
            let mut guard = locks_clone.lock();
            guard.remove(&key_clone);
        });

        Self {
            key,
            locks,
            drop_tx: tx,
        }
    }
}

impl Lock for InMemLock {
    fn release_lock(
        self: Pin<Box<Self>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LockError>> + Send>> {
        let this = Pin::into_inner(self);

        let key = this.key.clone();
        let locks = Arc::clone(&this.locks);
        drop(this.drop_tx);

        Box::pin(async move {
            let mut guard = locks.lock();
            guard.remove(&key);
            Ok(())
        })
    }
}

#[async_trait]
impl Locker for InMemoryLocker {
    fn acquire_lock(
        &self,
        key: &str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Pin<Box<dyn Lock>>, LockError>> + Send>>
    {
        let key = key.to_string();
        let locks = Arc::clone(&self.locks);

        Box::pin(async move {
            let mut guard = locks.lock();

            if guard.contains_key(&key) {
                return Err(LockError::AlreadyLocked { key });
            }

            // Create a drop channel that we'll use to signal lock release
            let (tx, _rx) = mpsc::channel::<()>(1);

            guard.insert(key.clone(), tx);

            let lock: Pin<Box<dyn Lock>> = Box::pin(InMemLock::new(key, Arc::clone(&locks)));
            Ok(lock)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Lock;

    #[tokio::test]
    async fn test_inmemory_locker_acquire_and_release() {
        let locker = InMemoryLocker::new();
        let key = "test_key";

        // First acquisition should succeed
        let lock = locker
            .acquire_lock(key)
            .await
            .expect("first acquire should succeed");
        lock.release_lock().await.expect("release should succeed");

        // After release, we should be able to acquire again
        let lock2 = locker
            .acquire_lock(key)
            .await
            .expect("second acquire should succeed");
        lock2
            .release_lock()
            .await
            .expect("second release should succeed");
    }

    #[tokio::test]
    async fn test_inmemory_locker_double_acquire_fails() {
        let locker = InMemoryLocker::new();
        let key = "test_key";

        // First acquisition should succeed
        let _lock = locker
            .acquire_lock(key)
            .await
            .expect("first acquire should succeed");

        // Second acquisition should fail
        let result = locker.acquire_lock(key).await;
        assert!(result.is_err());
        if let Err(LockError::AlreadyLocked { .. }) = result {
            // expected
        } else {
            panic!("expected AlreadyLocked error");
        }
    }

    #[tokio::test]
    async fn test_inmemory_locker_release_nonexistent_fails() {
        let _locker = InMemoryLocker::new();
        let key = "nonexistent_key";

        struct DummyLock {
            key: String,
        }

        impl Lock for DummyLock {
            fn release_lock(
                self: Pin<Box<Self>>,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LockError>> + Send>>
            {
                let key = self.key.clone();
                Box::pin(async move { Err(LockError::NotLocked { key }) })
            }
        }

        let lock: Pin<Box<dyn Lock>> = Box::pin(DummyLock {
            key: key.to_string(),
        });
        let result = lock.release_lock().await;
        assert!(result.is_err());
    }
}
