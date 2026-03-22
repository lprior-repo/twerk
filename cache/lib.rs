//! Thread-safe cache with automatic expiration cleanup.
//!
//! The janitor background thread runs periodically to remove expired items.

use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::time::{interval, MissedTickBehavior};

pub mod item;
pub use item::Item;

use tracing::{debug, instrument};

/// A thread-safe cache with optional automatic expiration cleanup.
///
/// The janitor thread runs every `cleanup_interval` duration to remove
/// expired items. If `cleanup_interval` is `None`, no janitor is spawned.
pub struct Cache<K, V> {
    /// The underlying storage map, wrapped in Arc for janitor access.
    items: Arc<DashMap<K, Item<V>>>,
    /// Interval between cleanup runs, or `None` if janitor is disabled.
    cleanup_interval: Option<Duration>,
    /// Shutdown flag for the janitor thread.
    shutdown_flag: Arc<AtomicBool>,
}

impl<K, V> Default for Cache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    /// Creates a new empty cache without a janitor thread.
    pub fn new() -> Self {
        Self {
            items: Arc::new(DashMap::new()),
            cleanup_interval: None,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates a new cache with automatic cleanup.
    ///
    /// If `cleanup_interval` is `Some(duration)`, a janitor thread is spawned
    /// that runs every `duration` to delete expired items. If `None`,
    /// no automatic cleanup is performed.
    ///
    /// # Panics
    ///
    /// Panics if `cleanup_interval` is `Some(Duration::ZERO)`.
    pub fn with_cleanup(cleanup_interval: Option<Duration>) -> Self {
        if let Some(interval) = cleanup_interval {
            if interval.is_zero() {
                panic!("cleanup_interval must be non-zero if provided");
            }

            let items = Arc::new(DashMap::<K, Item<V>>::new());
            let shutdown_flag = Arc::new(AtomicBool::new(false));

            let items_clone = Arc::clone(&items);
            let shutdown_flag_clone = Arc::clone(&shutdown_flag);

            tokio::spawn(async move {
                Self::janitor_loop(items_clone, interval, shutdown_flag_clone).await;
            });

            Self {
                items,
                cleanup_interval,
                shutdown_flag,
            }
        } else {
            Self::new()
        }
    }

    /// The janitor loop that periodically cleans up expired items.
    async fn janitor_loop(
        items: Arc<DashMap<K, Item<V>>>,
        cleanup_interval: Duration,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        debug!(
            "Janitor thread started with interval {:?}",
            cleanup_interval
        );

        let mut ticker = interval(cleanup_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if shutdown_flag.load(Ordering::Relaxed) {
                        debug!("Janitor thread shutting down");
                        break;
                    }
                    Self::delete_expired_from_map(&items);
                }
                _ = tokio::task::yield_now() => {
                    // Check shutdown flag on every yield
                    if shutdown_flag.load(Ordering::Relaxed) {
                        debug!("Janitor thread shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Deletes all expired items from the given map.
    fn delete_expired_from_map(items: &Arc<DashMap<K, Item<V>>>) {
        // Count expired items before removing
        let expired_count = items.iter().filter(|entry| entry.value().is_expired()).count();
        
        if expired_count > 0 {
            debug!("Janitor deleting {} expired items", expired_count);
        }

        // Retain only non-expired items
        items.retain(|_k, v| !v.is_expired());
    }

    /// Returns the number of items in the cache.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the cache contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns `true` if the cache has a janitor thread running.
    pub fn has_janitor(&self) -> bool {
        self.cleanup_interval.is_some()
    }

    /// Deletes all expired items from the cache.
    ///
    /// This can be called manually or by the janitor thread.
    #[instrument(skip(self))]
    pub fn delete_expired(&self) {
        Self::delete_expired_from_map(&self.items);
    }

    /// Inserts an item into the cache.
    ///
    /// If the key already existed, the old item is returned.
    pub fn insert(&self, key: K, value: V, expiration: Option<Duration>) -> Option<Item<V>> {
        let expiration = expiration.map(|d| std::time::Instant::now() + d);
        self.items.insert(key, Item::new(value, expiration))
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
        let entry = self.items.get(key);
        entry.map(|e| !e.is_expired()).unwrap_or(false)
    }

    /// Removes the item with the given key, returning it if it existed.
    pub fn remove(&self, key: &K) -> Option<Item<V>> {
        self.items.remove(key).map(|(_, item)| item)
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
        if self.cleanup_interval.is_some() {
            self.shutdown_flag.store(true, Ordering::Relaxed);
        }
    }
}

impl<K, V> Drop for Cache<K, V> {
    fn drop(&mut self) {
        // Signal the janitor to shut down
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cache_without_janitor() {
        let cache = Cache::<i32, String>::new();
        assert!(!cache.has_janitor());
        assert!(cache.is_empty());

        cache.insert(1, "one".to_string(), None);
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_cache_with_janitor() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(50)));
        assert!(cache.has_janitor());

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(200)));
        cache.insert(2, "two".to_string(), Some(Duration::from_millis(10)));

        // Item 2 should expire quickly
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Give janitor time to clean up
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cache should have only item 1 (item 2 expired and was cleaned by janitor)
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_expired() {
        let cache = Cache::new();
        cache.insert(1, "one".to_string(), Some(Duration::from_millis(1)));
        cache.insert(2, "two".to_string(), Some(Duration::from_millis(1000)));

        // Wait for item 1 to expire
        tokio::time::sleep(Duration::from_millis(10)).await;
        cache.delete_expired();

        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&2));
    }

    #[tokio::test]
    async fn test_insert_with_expiration() {
        let cache = Cache::new();
        cache.insert(1, "value".to_string(), Some(Duration::from_millis(10)));

        assert!(cache.contains(&1));

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        assert!(!cache.contains(&1));
    }

    #[test]
    fn test_cache_default() {
        let cache: Cache<i32, i32> = Cache::default();
        assert!(!cache.has_janitor());
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_close() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(10)));
        assert!(cache.has_janitor());

        cache.insert(1, "one".to_string(), None);

        cache.close();

        // Give time for shutdown to take effect
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Item should still be there
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_janitor_cleans_expired_items() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(20)));

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(5)));

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Janitor should have cleaned it by now
        tokio::time::sleep(Duration::from_millis(30)).await;

        assert_eq!(cache.len(), 0);
    }
}
