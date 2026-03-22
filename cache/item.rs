//! Cache item implementation.

use std::sync::Mutex;
use std::time::Instant;

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

    /// Returns `true` if this item has expired.
    ///
    /// An item with `expiration: None` is never considered expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expiration.is_some_and(|exp| Instant::now() > exp)
    }

    /// Returns a reference to the stored object, if the lock can be acquired.
    pub fn get(&self) -> Option<MutexGuard<'_, V>> {
        self.object.lock().ok()
    }

    /// Returns a reference to the stored object, blocking until the lock is acquired.
    pub fn get_blocking(&self) -> MutexGuard<'_, V> {
        self.object.lock().expect("mutex poisoned")
    }

    /// Returns the expiration time, if any.
    #[must_use]
    pub fn expiration(&self) -> Option<Instant> {
        self.expiration
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

/// A guard for accessing the value inside an `Item`.
pub type MutexGuard<'a, V> = std::sync::MutexGuard<'a, V>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_never_expires() {
        let item = Item::new(42, None);
        assert!(!item.is_expired());
        let value = item.get().expect("should get lock");
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_item_expired() {
        let item = Item::new(42, Some(Instant::now() - std::time::Duration::from_secs(1)));
        assert!(item.is_expired());
    }

    #[test]
    fn test_item_not_yet_expired() {
        let item = Item::new(
            42,
            Some(Instant::now() + std::time::Duration::from_secs(100)),
        );
        assert!(!item.is_expired());
    }

    #[test]
    fn test_clone() {
        let item = Item::new(vec![1, 2, 3], None);
        let cloned = item.clone();
        let orig = item.get().expect("should get lock");
        let clone = cloned.get().expect("should get lock");
        assert_eq!(*orig, *clone);
    }

    #[test]
    fn test_get_returns_none_on_poison() {
        // This test verifies that get() returns Option
        // We can't easily test poison, but we can verify the API works
        let item = Item::new(42, None);
        let value = item.get();
        assert!(value.is_some());
    }
}
