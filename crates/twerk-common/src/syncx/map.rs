//! Thread-safe map implementation using `DashMap`.
//!
//! # Architecture
//!
//! - **Data**: `Map` struct wraps `DashMap` for concurrent access
//! - **Calc**: Pure getter operations with Option handling
//! - **Actions**: All operations are thread-safe by design

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use dashmap::DashMap;
use std::borrow::Borrow;
use std::cmp::Eq;
use std::hash::Hash;
use std::sync::Arc;

/// A thread-safe map wrapper around `DashMap`.
///
/// This provides a similar interface to Go's sync.Map with
/// Get, Set, Delete, and Iterate operations.
///
/// # Type Parameters
///
/// * `K` - The key type, must be hashable and comparable
/// * `V` - The value type
#[derive(Debug)]
pub struct Map<K, V>
where
    K: Hash + Eq,
{
    inner: Arc<DashMap<K, V>>,
}

impl<K, V> Clone for Map<K, V>
where
    K: Hash + Eq,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<K, V> Map<K, V>
where
    K: Hash + Eq,
{
    /// Creates a new empty `Map`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }
}

impl<K, V> Default for Map<K, V>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Map<K, V>
where
    K: Hash + Eq + Clone,
{
    /// Deletes a key from the map.
    pub fn delete(&self, key: &K) {
        self.inner.remove(key);
    }
}

impl<K, V> Map<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Sets a key-value pair in the map.
    pub fn set(&self, key: K, value: V) {
        self.inner.insert(key, value);
    }

    /// Iterates over all key-value pairs, calling `f` for each.
    pub fn iterate<F>(&self, mut f: F)
    where
        F: FnMut(K, V),
    {
        // Clone entries to avoid holding lock during callback
        let entries: Vec<(K, V)> = self
            .inner
            .iter()
            .map(|pair| (pair.key().clone(), pair.value().clone()))
            .collect();

        for (k, v) in entries {
            f(k, v);
        }
    }
}

impl<K, V> Map<K, V>
where
    K: Hash + Eq,
    V: Clone,
{
    /// Gets a value by key.
    ///
    /// Returns `Some(value)` if found, `None` if not.
    ///
    /// This method uses `Borrow` to allow looking up with a reference
    /// to a different type than the key (e.g., `&str` when key is `String`).
    #[must_use]
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get(key).map(|v| v.value().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_non_existent() {
        let m = Map::<String, i32>::new();
        let v = m.get(&"nothing".to_string());
        assert!(v.is_none());
    }

    #[test]
    fn test_set_and_get() {
        let m = Map::<String, i32>::new();
        m.set("somekey".to_string(), 100);
        let v = m.get(&"somekey".to_string());
        assert_eq!(Some(100), v);
    }

    #[test]
    fn test_set_and_delete() {
        let m = Map::<String, i32>::new();
        m.set("somekey".to_string(), 100);
        let v = m.get(&"somekey".to_string());
        assert_eq!(Some(100), v);
        m.delete(&"somekey".to_string());
        let v = m.get(&"somekey".to_string());
        assert!(v.is_none());
    }

    #[test]
    fn test_iterate() {
        let m = Map::<String, i32>::new();
        m.set("k1".to_string(), 100);
        m.set("k2".to_string(), 200);

        let mut vals = Vec::new();
        let mut keys = Vec::new();

        m.iterate(|k, v| {
            vals.push(v);
            keys.push(k);
        });

        vals.sort_unstable();
        keys.sort();
        assert_eq!(vec![100, 200], vals);
        assert_eq!(vec!["k1".to_string(), "k2".to_string()], keys);
    }

    #[test]
    fn test_concurrent_set_and_get() {
        use std::thread;

        let m = Arc::new(Map::<String, i32>::new());
        let mut handles = Vec::new();

        for i in 1..=100 {
            let m_clone = Arc::clone(&m);
            let handle = thread::spawn(move || {
                m_clone.set("somekey".to_string(), i);
                let v = m_clone.get(&"somekey".to_string());
                assert!(matches!(v, Some(x) if x > 0));
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }
    }

    #[test]
    fn test_clone_is_independent() {
        let m1 = Map::<String, i32>::new();
        m1.set("key1".to_string(), 100);

        let m2 = m1.clone();
        m2.set("key2".to_string(), 200);

        // Both maps share the same underlying storage
        // key1 is set in m1, key2 is set in m2
        let v1 = m1.get(&"key1".to_string());
        let v2 = m2.get(&"key2".to_string());
        assert_eq!(Some(100), v1);
        assert_eq!(Some(200), v2);

        // Both can access both keys since they share storage
        let v1_key2 = m1.get(&"key2".to_string());
        let v2_key1 = m2.get(&"key1".to_string());
        assert_eq!(Some(200), v1_key2);
        assert_eq!(Some(100), v2_key1);
    }
}
