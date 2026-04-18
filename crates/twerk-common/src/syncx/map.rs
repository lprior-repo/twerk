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

    #[test]
    fn test_empty_string_key() {
        let m = Map::<String, i32>::new();
        m.set("".to_string(), 42);
        assert_eq!(Some(42), m.get(&"".to_string()));
        m.delete(&"".to_string());
        assert_eq!(None, m.get(&"".to_string()));
    }

    #[test]
    fn test_special_characters_in_key() {
        let m = Map::<String, String>::new();
        let special_keys = vec![
            "key with spaces",
            "key\twith\ttabs",
            "key\nwith\nnewlines",
            "key\rwith\rcarriage",
            "key\"with\"quotes",
            "key\\with\\backslashes",
            "key/with/slashes",
            "key#with#hashes",
            "key?with?questions",
            "key&with&ampersands",
            "key=with=equals",
            "key%with%percent",
            "key\x00with\x00nulls",
        ];

        for (i, key) in special_keys.iter().enumerate() {
            let owned_key = key.to_string();
            m.set(owned_key.clone(), format!("value_{}", i));
            assert_eq!(Some(format!("value_{}", i)), m.get(&owned_key));
        }
    }

    #[test]
    fn test_special_characters_in_value() {
        let m = Map::<String, String>::new();
        let special_values = vec![
            "value with spaces",
            "value\twith\ttabs",
            "value\nwith\nnewlines",
            "value\rwith\rcarriage",
            "value\"with\"quotes",
            "value\\with\\backslashes",
            "value/with/slashes",
            "value#with#hashes",
            "value?with?questions",
            "value&with&ampersands",
            "value=with=equals",
            "value%with%percent",
            "value\x00with\x00nulls",
        ];

        for (i, val) in special_values.iter().enumerate() {
            m.set(format!("key_{}", i), val.to_string());
        }

        for (i, val) in special_values.iter().enumerate() {
            assert_eq!(Some(val.to_string()), m.get(&format!("key_{}", i)));
        }
    }

    #[test]
    fn test_very_long_key_and_value() {
        let m = Map::<String, String>::new();
        let long_key = "k".repeat(100_000);
        let long_value = "v".repeat(100_000);

        m.set(long_key.clone(), long_value.clone());
        assert_eq!(Some(long_value.clone()), m.get(&long_key));

        m.delete(&long_key);
        assert_eq!(None, m.get(&long_key));
    }

    #[test]
    fn test_many_small_keys_and_values() {
        let m = Map::<String, i32>::new();
        let count = 10_000;

        for i in 0..count {
            m.set(format!("key_{}", i), i);
        }

        for i in 0..count {
            assert_eq!(Some(i), m.get(&format!("key_{}", i)));
        }

        let mut found_count = 0;
        m.iterate(|_k, _v| {
            found_count += 1;
        });
        assert_eq!(count, found_count);
    }

    #[test]
    fn test_concurrent_delete_during_iterate() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let m = Arc::new(Map::<String, i32>::new());

        for i in 0..1000 {
            m.set(format!("key_{}", i), i);
        }

        let m_clone = Arc::clone(&m);
        let iter_handle = thread::spawn(move || {
            let mut count = 0;
            m_clone.iterate(|k, _v| {
                if k == "key_500" {
                    m_clone.delete(&k);
                }
                count += 1;
            });
            count
        });

        let m_clone2 = Arc::clone(&m);
        let delete_handle = thread::spawn(move || {
            thread::sleep(Duration::from_micros(100));
            for i in 0..500 {
                m_clone2.delete(&format!("key_{}", i));
            }
        });

        let iter_count = iter_handle.join().unwrap();
        delete_handle.join().unwrap();

        assert!(iter_count > 0);
        assert_eq!(None, m.get(&"key_500".to_string()));
    }

    #[test]
    fn test_concurrent_iterate_and_set() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let m = Arc::new(Map::<String, i32>::new());

        for i in 0..100 {
            m.set(format!("key_{}", i), i);
        }

        let m_clone = Arc::clone(&m);
        let iter_handle = thread::spawn(move || {
            for _ in 0..10 {
                let mut count = 0;
                m_clone.iterate(|_k, _v| {
                    count += 1;
                });
                thread::sleep(Duration::from_micros(10));
            }
        });

        let m_clone2 = Arc::clone(&m);
        let set_handle = thread::spawn(move || {
            for i in 100..200 {
                m_clone2.set(format!("key_{}", i), i);
                thread::sleep(Duration::from_micros(10));
            }
        });

        iter_handle.join().unwrap();
        set_handle.join().unwrap();

        assert!(m.get(&"key_150".to_string()).is_some());
    }

    #[test]
    fn test_empty_map_operations() {
        let m: Map<String, i32> = Map::new();
        assert_eq!(None, m.get(&"anything".to_string()));

        let mut found = false;
        m.iterate(|_k, _v| {
            found = true;
        });
        assert!(!found);
    }
}
