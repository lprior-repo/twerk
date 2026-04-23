//! Core cache operations: insert, get, remove, modify, list, iterate, etc.

use std::time::Duration;

use super::item::Expiration;
use super::{Cache, Item, ListFilter};

impl<K, V> Cache<K, V>
where
    K: Eq + std::hash::Hash + Send + Sync + Clone + 'static,
    V: Send + Sync + Clone + 'static,
{
    /// Inserts an item into the cache.
    ///
    /// If the key already existed, the old item is returned.
    /// Use `insert_with_expiration` for explicit expiration control.
    pub fn insert(&self, key: K, value: V, expiration: Option<Duration>) -> Option<Item<V>> {
        let expiration = expiration.map(|d| tokio::time::Instant::now() + d);
        self.items.insert(key, Item::new(value, expiration))
    }

    /// Inserts an item with explicit expiration behavior.
    ///
    /// - `Expiration::Default` uses the cache's default expiration
    /// - `Expiration::Never` means the item never expires
    /// - `Expiration::Absolute` sets a specific expiration time
    ///
    /// If the key already existed, the old item is returned.
    pub fn insert_expiring(&self, key: K, value: V, expiration: Expiration) -> Option<Item<V>> {
        let default_exp = self
            .default_expiration
            .map(|d| tokio::time::Instant::now() + d);
        let item = Item::with_expiration(value, expiration, default_exp);
        self.items.insert(key, item)
    }

    /// Returns a cloned value if it exists and is not expired.
    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let entry = self.items.get(key)?;
        if entry.is_expired() {
            None
        } else {
            entry.get().map(|guard| guard.clone())
        }
    }

    /// Returns `true` if the cache contains the given key and it is not expired.
    pub fn contains(&self, key: &K) -> bool {
        self.items.get(key).is_some_and(|entry| !entry.is_expired())
    }

    /// Removes the item with the given key, returning it if it existed.
    ///
    /// If an item was removed and an `on_evicted` callback is set, it will be invoked.
    pub fn remove(&self, key: &K) -> Option<Item<V>> {
        self.items.remove(key).map(|(k, item)| {
            if let Some(callback) = self.on_evicted().lock().as_ref() {
                callback(&k, &item.object());
            }
            item
        })
    }

    /// Clears all items from the cache.
    pub fn clear(&self) {
        self.items.clear();
    }

    /// Shuts down the cache's janitor thread, if any.
    ///
    /// This signals the janitor to stop. For caches without a janitor,
    /// this is a no-op.
    pub fn close(&self) {
        if self.cleanup_interval().is_some() {
            self.shutdown_flag()
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Sets a new expiration on an existing key.
    ///
    /// Returns `true` if the key existed and was not expired, `false` otherwise.
    pub fn set_expiration(&self, key: &K, duration: Duration) -> bool {
        let expiration = Some(tokio::time::Instant::now() + duration);
        self.items
            .get_mut(key)
            .map(|mut entry| {
                entry.set_expiration(expiration);
            })
            .is_some()
    }

    /// Atomically modifies the value for a key using the given function.
    ///
    /// The modifier `f` is called with mutable access to the value, and can
    /// return an error to abort the modification. If the key does not exist
    /// or is expired, returns `None`.
    ///
    /// Returns `Some(Ok(()))` if the modification succeeded.
    /// Returns `Some(Err(e))` if the modifier returned an error.
    pub fn modify<F, E>(&self, key: &K, f: F) -> Option<Result<(), E>>
    where
        F: FnOnce(&mut V) -> Result<(), E>,
    {
        let entry = self.items.get(key).filter(|e| !e.is_expired())?;
        let mut guard = entry.get_mut()?;
        Some(f(&mut guard))
    }

    /// Returns all non-expired items matching the given filters.
    ///
    /// If no filters are provided, returns all non-expired items.
    /// Items are returned as clones and order is not guaranteed.
    #[must_use]
    #[allow(clippy::type_complexity)]
    #[allow(dead_code)]
    pub(crate) fn list<'a>(&'a self, filters: &'a [ListFilter<'a, V>]) -> Vec<V>
    where
        V: Clone,
    {
        self.items
            .iter()
            .filter(|entry| !entry.is_expired())
            .filter_map(|entry| {
                let guard = entry.get()?;
                if filters.iter().all(|f| f(&guard)) {
                    Some(guard.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Iterates over all non-expired items in the cache.
    ///
    /// The iterator function `f` is called for each key-value pair.
    /// Iteration stops early if `f` returns `false`.
    ///
    /// Returns the number of items iterated.
    pub fn iterate<F>(&self, mut f: F) -> usize
    where
        F: FnMut(&K, &V) -> bool,
    {
        self.items
            .iter()
            .filter(|entry| !entry.is_expired())
            .filter_map(|entry| {
                let guard = entry.get()?;
                let key = entry.key();
                if f(key, &guard) {
                    Some(1)
                } else {
                    None
                }
            })
            .count()
    }
}
