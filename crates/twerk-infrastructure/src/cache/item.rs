//! Cache item implementation.

use parking_lot::{Mutex, MutexGuard};
use tokio::time::Instant;

/// Expiration behavior for cache items.
///
/// - `Default` - Use the cache's default expiration
/// - `Never` - Item never expires
/// - `Absolute(Instant)` - Expires at a specific instant
#[derive(Debug, Clone, Copy, Default)]
pub enum Expiration {
    #[default]
    Default,
    Never,
    Absolute(Instant),
}

/// A cache item storing a value with an optional expiration time.
///
/// Uses interior mutability via `Mutex` for thread-safe access to the stored value.
#[derive(Debug)]
pub struct Item<V> {
    /// The stored value, protected by a mutex for interior mutability.
    object: Mutex<V>,
    /// When this item expires, or `None` if it never expires.
    expiration: Option<Instant>,
}

impl<V> Item<V> {
    /// Creates a new `Item` with the given value and optional expiration.
    #[must_use]
    pub fn new(object: V, expiration: Option<Instant>) -> Self {
        Self {
            object: Mutex::new(object),
            expiration,
        }
    }

    /// Creates a new `Item` with the given value and expiration policy.
    #[must_use]
    pub fn with_expiration(
        object: V,
        expiration: Expiration,
        default_exp: Option<Instant>,
    ) -> Self {
        let expiration = match expiration {
            Expiration::Default => default_exp,
            Expiration::Never => None,
            Expiration::Absolute(inst) => Some(inst),
        };
        Self {
            object: Mutex::new(object),
            expiration,
        }
    }

    /// Returns a clone of the inner object.
    #[must_use]
    pub fn object(&self) -> V
    where
        V: Clone,
    {
        self.object.lock().clone()
    }

    /// Returns `true` if this item has expired.
    ///
    /// An item with `expiration: None` is never considered expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expiration.is_some_and(|exp| Instant::now() > exp)
    }

    /// Returns a reference to the stored object, if the lock can be acquired.
    pub fn get(&self) -> Option<MutexGuard<'_, V>> {
        Some(self.object.lock())
    }

    /// Returns a reference to the stored object, blocking until the lock is acquired.
    pub fn get_blocking(&self) -> MutexGuard<'_, V> {
        self.object.lock()
    }

    /// Returns the expiration time, if any.
    #[must_use]
    pub fn expiration(&self) -> Option<Instant> {
        self.expiration
    }

    /// Sets a new expiration time, replacing any existing expiration.
    pub fn set_expiration(&mut self, expiration: Option<Instant>) {
        self.expiration = expiration;
    }

    /// Returns a mutable reference to the stored object, if the lock can be acquired.
    pub fn get_mut(&self) -> Option<MutexGuard<'_, V>> {
        Some(self.object.lock())
    }
}

// Implement Clone manually to clone the inner value, not the mutex itself.
impl<V: Clone> Clone for Item<V> {
    fn clone(&self) -> Self {
        let object = self.get_blocking().clone();
        Self {
            object: Mutex::new(object),
            expiration: self.expiration,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_item_never_expires() {
        let item = Item::new(42, None);
        assert!(!item.is_expired());
        let value = item.get().expect("should get lock");
        assert_eq!(*value, 42);
    }

    #[tokio::test]
    async fn test_item_expired() {
        let item = Item::new(42, Some(Instant::now() - Duration::from_secs(1)));
        assert!(item.is_expired());
    }

    #[tokio::test]
    async fn test_item_not_yet_expired() {
        let item = Item::new(42, Some(Instant::now() + Duration::from_secs(100)));
        assert!(!item.is_expired());
    }

    #[tokio::test]
    async fn test_clone() {
        let item = Item::new(vec![1, 2, 3], None);
        let cloned = item.clone();
        let orig = item.get().expect("should get lock");
        let clone = cloned.get().expect("should get lock");
        assert_eq!(*orig, *clone);
    }

    #[tokio::test]
    async fn test_get_returns_none_on_poison() {
        let item = Item::new(42, None);
        let value = item.get();
        assert!(value.is_some());
    }
}
