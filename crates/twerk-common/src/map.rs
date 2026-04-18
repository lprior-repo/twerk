//! Thread-safe map implementation using DashMap.
//!
//! # Architecture
//!
//! - **Data**: `Map` struct wraps DashMap for concurrent access
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

/// A thread-safe map wrapper around DashMap.
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
    pub fn delete(&self, key: K) {
        self.inner.remove(&key);
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

        entries.into_iter().for_each(|(k, v)| f(k, v));
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
        m.delete("somekey".to_string());
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

        vals.sort();
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

        let v1 = m1.get(&"key1".to_string());
        let v2 = m2.get(&"key2".to_string());
        assert_eq!(Some(100), v1);
        assert_eq!(Some(200), v2);

        let v1_key2 = m1.get(&"key2".to_string());
        let v2_key1 = m2.get(&"key1".to_string());
        assert_eq!(Some(200), v1_key2);
        assert_eq!(Some(100), v2_key1);
    }

    #[test]
    fn test_empty_string_key() {
        let m = Map::<String, i32>::new();
        m.set("".to_string(), 1);
        m.set("normal".to_string(), 2);

        assert_eq!(Some(1), m.get(&"".to_string()));
        assert_eq!(Some(2), m.get(&"normal".to_string()));

        m.delete("".to_string());
        assert!(m.get(&"".to_string()).is_none());
        assert_eq!(Some(2), m.get(&"normal".to_string()));
    }

    #[test]
    fn test_special_characters_in_key() {
        let m = Map::<String, String>::new();
        m.set("key with spaces".to_string(), "value1".to_string());
        m.set("key\twith\ttabs".to_string(), "value2".to_string());
        m.set("key\nwith\nnewlines".to_string(), "value3".to_string());
        m.set("key\0with\0nulls".to_string(), "value4".to_string());
        m.set("".to_string(), "empty key".to_string());
        m.set("🚀".to_string(), "emoji value".to_string());

        assert_eq!(
            Some("value1".to_string()),
            m.get(&"key with spaces".to_string())
        );
        assert_eq!(
            Some("value2".to_string()),
            m.get(&"key\twith\ttabs".to_string())
        );
        assert_eq!(
            Some("value3".to_string()),
            m.get(&"key\nwith\nnewlines".to_string())
        );
        assert_eq!(
            Some("value4".to_string()),
            m.get(&"key\0with\0nulls".to_string())
        );
        assert_eq!(Some("empty key".to_string()), m.get(&"".to_string()));
        assert_eq!(Some("emoji value".to_string()), m.get(&"🚀".to_string()));
    }

    #[test]
    fn test_special_characters_in_value() {
        let m = Map::<String, String>::new();
        m.set("key1".to_string(), "value with spaces".to_string());
        m.set("key2".to_string(), "value\twith\ttabs".to_string());
        m.set("key3".to_string(), "value\nwith\nnewlines".to_string());
        m.set("key4".to_string(), "value\0with\nulls".to_string());
        m.set("key5".to_string(), "".to_string());
        m.set("key6".to_string(), "🚀 emoji".to_string());

        assert_eq!(
            Some("value with spaces".to_string()),
            m.get(&"key1".to_string())
        );
        assert_eq!(
            Some("value\twith\ttabs".to_string()),
            m.get(&"key2".to_string())
        );
        assert_eq!(
            Some("value\nwith\nnewlines".to_string()),
            m.get(&"key3".to_string())
        );
        assert_eq!(
            Some("value\0with\nulls".to_string()),
            m.get(&"key4".to_string())
        );
        assert_eq!(Some("".to_string()), m.get(&"key5".to_string()));
        assert_eq!(Some("🚀 emoji".to_string()), m.get(&"key6".to_string()));
    }

    #[test]
    fn test_very_long_key_and_value() {
        let m = Map::<String, String>::new();
        let long_key = "k".repeat(100_000);
        let long_value = "v".repeat(100_000);

        m.set(long_key.clone(), long_value.clone());

        assert_eq!(Some(long_value.clone()), m.get(&long_key));
    }

    #[test]
    fn test_many_small_entries() {
        let m = Map::<String, i32>::new();

        for i in 0..1000 {
            m.set(format!("key{}", i), i);
        }

        for i in 0..1000 {
            assert_eq!(Some(i), m.get(&format!("key{}", i)));
        }
    }

    #[test]
    fn test_delete_during_iterate() {
        let m = Map::<String, i32>::new();
        m.set("k1".to_string(), 1);
        m.set("k2".to_string(), 2);
        m.set("k3".to_string(), 3);

        let mut count = 0;
        m.iterate(|k, _v| {
            count += 1;
            if k == "k2" {
                m.delete(k);
            }
        });

        assert_eq!(3, count);
        assert!(m.get(&"k2".to_string()).is_none());
        assert_eq!(Some(1), m.get(&"k1".to_string()));
        assert_eq!(Some(3), m.get(&"k3".to_string()));
    }

    #[test]
    fn test_concurrent_delete_during_iterate() {
        use std::thread;

        let m = Arc::new(Map::<String, i32>::new());
        m.set("k1".to_string(), 1);
        m.set("k2".to_string(), 2);
        m.set("k3".to_string(), 3);
        m.set("k4".to_string(), 4);
        m.set("k5".to_string(), 5);

        let m_clone = Arc::clone(&m);
        let handle = thread::spawn(move || {
            m_clone.delete("k3".to_string());
            m_clone.delete("k5".to_string());
        });

        let mut observed_keys = Vec::new();
        m.iterate(|k, _v| {
            observed_keys.push(k);
        });

        let _ = handle.join();

        assert!(observed_keys.contains(&"k1".to_string()));
        assert!(observed_keys.contains(&"k2".to_string()));
        assert!(observed_keys.contains(&"k4".to_string()));
    }

    #[test]
    fn test_concurrent_access_during_iteration() {
        use std::thread;

        let m = Arc::new(Map::<String, i32>::new());
        for i in 0..100 {
            m.set(format!("key{}", i), i);
        }

        let m_clone = Arc::clone(&m);
        let writer_handle = thread::spawn(move || {
            for i in 100..200 {
                m_clone.set(format!("key{}", i), i);
            }
        });

        let mut sum = 0;
        m.iterate(|_k, v| {
            sum += v;
        });

        let _ = writer_handle.join();

        assert_eq!(Some(199), m.get(&"key199".to_string()));
    }

    #[test]
    fn test_iterate_empty_map() {
        let m = Map::<String, i32>::new();
        let mut count = 0;
        m.iterate(|_k, _v| {
            count += 1;
        });
        assert_eq!(0, count);
    }

    #[test]
    fn test_iterate_after_clear() {
        let m = Map::<String, i32>::new();
        m.set("k1".to_string(), 1);
        m.set("k2".to_string(), 2);

        m.delete("k1".to_string());
        m.delete("k2".to_string());

        let mut count = 0;
        m.iterate(|_k, _v| {
            count += 1;
        });
        assert_eq!(0, count);
    }
}
