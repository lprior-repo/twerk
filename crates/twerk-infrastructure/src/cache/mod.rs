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

/// A filter function type for use with [`Cache::list`].
/// Matches items where the function returns `true`.
type ListFilter<'a, V> = Box<dyn Fn(&V) -> bool + 'a>;

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
        let expired_count = items
            .iter()
            .filter(|entry| entry.value().is_expired())
            .count();

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

    /// Sets a new expiration on an existing key.
    ///
    /// Returns `true` if the key existed and was not expired, `false` otherwise.
    pub fn set_expiration(&self, key: &K, duration: Duration) -> bool {
        let expiration = Some(std::time::Instant::now() + duration);
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
    #[allow(clippy::type_complexity)]
    pub fn list<'a>(&'a self, filters: &'a [ListFilter<'a, V>]) -> Vec<V>
    where
        V: Clone,
    {
        let mut result = Vec::new();
        for entry in self.items.iter() {
            if entry.is_expired() {
                continue;
            }
            if let Some(guard) = entry.get() {
                if filters.iter().all(|f| f(&guard)) {
                    result.push((*guard).clone());
                }
            }
        }
        result
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
        let mut count = 0;

        for entry in self.items.iter().filter(|e| !e.is_expired()) {
            let guard = match entry.get() {
                Some(g) => g,
                None => continue,
            };
            if !f(entry.key(), &guard) {
                break;
            }
            count += 1;
        }

        count
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
    async fn cache_is_empty_when_created_without_janitor() {
        let cache = Cache::<i32, String>::new();
        assert!(!cache.has_janitor());
        assert!(cache.is_empty());

        cache.insert(1, "one".to_string(), None);
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cache_cleans_expired_items_when_janitor_is_enabled() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(50)));
        assert!(cache.has_janitor());

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(200)));
        cache.insert(2, "two".to_string(), Some(Duration::from_millis(10)));

        // Advance time for item 2 to expire and janitor to run
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        // Cache should have only item 1 (item 2 expired and was cleaned by janitor)
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&1));
    }

    #[tokio::test(start_paused = true)]
    async fn cache_removes_expired_items_when_delete_expired_is_called() {
        let cache = Cache::new();
        cache.insert(1, "one".to_string(), Some(Duration::from_millis(1)));
        cache.insert(2, "two".to_string(), Some(Duration::from_millis(1000)));

        // Wait for item 1 to expire
        tokio::time::advance(Duration::from_millis(10)).await;
        cache.delete_expired();

        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&2));
        assert!(!cache.contains(&1));
    }

    #[tokio::test(start_paused = true)]
    async fn cache_contains_returns_false_when_item_expires() {
        let cache = Cache::new();
        cache.insert(1, "value".to_string(), Some(Duration::from_millis(10)));

        assert!(cache.contains(&1));

        // Wait for expiration
        tokio::time::advance(Duration::from_millis(20)).await;

        assert!(!cache.contains(&1));
    }

    #[test]
    fn cache_defaults_to_no_janitor_and_empty() {
        let cache: Cache<i32, i32> = Cache::default();
        assert!(!cache.has_janitor());
        assert!(cache.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn cache_retains_items_when_closed() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(10)));
        assert!(cache.has_janitor());

        cache.insert(1, "one".to_string(), None);

        cache.close();

        // Give time for shutdown signal to be processed
        tokio::time::advance(Duration::from_millis(20)).await;
        tokio::task::yield_now().await;

        // Item should still be there
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn janitor_cleans_expired_items_when_ticker_ticks() {
        let cache = Cache::with_cleanup(Some(Duration::from_millis(20)));

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(5)));

        // Wait for expiration and janitor run
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        assert_eq!(cache.len(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn cache_expires_item_when_shorter_expiration_is_set() {
        let cache = Cache::new();

        // Set initial item with long expiration
        cache.insert(1, "one".to_string(), Some(Duration::from_secs(10)));
        assert!(cache.contains(&1));

        // Modify expiration to very short
        let result = cache.set_expiration(&1, Duration::from_millis(1));
        assert!(result);

        // Item should still be there
        assert!(cache.contains(&1));

        // Wait for new expiration
        tokio::time::advance(Duration::from_millis(10)).await;

        // Item should be expired now
        assert!(!cache.contains(&1));
    }

    #[tokio::test]
    async fn cache_set_expiration_returns_false_when_key_is_missing() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), None);

        // Try to set expiration on non-existent key
        let result = cache.set_expiration(&999, Duration::from_secs(10));
        assert!(!result);
    }

    #[tokio::test]
    async fn cache_modify_updates_value_when_key_exists() {
        let cache = Cache::new();

        cache.insert(1, 10i32, None);

        // Modify value
        let result = cache.modify(&1, |v| {
            *v *= 2;
            Ok::<(), ()>(())
        });
        
        // Assert concrete outcome
        match result {
            Some(Ok(())) => {},
            _ => panic!("Expected Some(Ok(())), got {:?}", result),
        }

        // Verify modification
        assert_eq!(cache.get(&1), Some(20));
    }

    #[tokio::test]
    async fn cache_modify_returns_error_when_modifier_fails() {
        let cache = Cache::new();

        cache.insert(1, 10i32, None);

        // Modify that returns error
        let result = cache.modify(&1, |_v| Err::<(), &str>("something went wrong"));
        
        match result {
            Some(Err(e)) => assert_eq!(e, "something went wrong"),
            _ => panic!("Expected Some(Err(\"something went wrong\")), got {:?}", result),
        }

        // Value should be unchanged
        assert_eq!(cache.get(&1), Some(10));
    }

    #[tokio::test]
    async fn cache_modify_returns_none_when_key_is_missing() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), None);

        // Try to modify non-existent key
        let result = cache.modify::<_, ()>(&999, |_v| Ok(()));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn cache_list_returns_all_items_when_no_filters_applied() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), None);
        cache.insert(2, "two".to_string(), None);
        cache.insert(3, "three".to_string(), None);

        let items = cache.list(&[]);
        assert_eq!(items.len(), 3);
        assert!(items.contains(&"one".to_string()));
        assert!(items.contains(&"two".to_string()));
        assert!(items.contains(&"three".to_string()));
    }

    #[tokio::test]
    async fn cache_list_returns_matching_items_when_filters_applied() {
        let cache = Cache::new();

        cache.insert(1, 10i32, None);
        cache.insert(2, 20i32, None);
        cache.insert(3, 30i32, None);
        cache.insert(4, 40i32, None);

        // Filter for values >= 20
        let is_large = Box::new(|v: &i32| *v >= 20);
        let items = cache.list(&[is_large]);
        assert_eq!(items.len(), 3);
        assert!(!items.contains(&10));
        assert!(items.contains(&20));
        assert!(items.contains(&30));
        assert!(items.contains(&40));
    }

    #[tokio::test]
    async fn cache_list_returns_matching_items_when_multiple_filters_applied() {
        let cache = Cache::new();

        cache.insert(1, 10i32, None);
        cache.insert(2, 20i32, None);
        cache.insert(3, 30i32, None);
        cache.insert(4, 40i32, None);

        // Filter for values >= 20 AND < 40
        let is_large = Box::new(|v: &i32| *v >= 20);
        let is_small = Box::new(|v: &i32| *v < 40);
        let items = cache.list(&[is_large, is_small]);
        assert_eq!(items.len(), 2);
        assert!(items.contains(&20));
        assert!(items.contains(&30));
    }

    #[tokio::test(start_paused = true)]
    async fn cache_list_excludes_items_when_expired() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(1)));
        cache.insert(2, "two".to_string(), None);

        // Wait for item 1 to expire
        tokio::time::advance(Duration::from_millis(10)).await;

        let items = cache.list(&[]);
        assert_eq!(items.len(), 1);
        assert!(items.contains(&"two".to_string()));
    }

    #[tokio::test]
    async fn cache_iterate_visits_all_items_when_returning_true() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), None);
        cache.insert(2, "two".to_string(), None);
        cache.insert(3, "three".to_string(), None);

        let mut sum = 0;
        let count = cache.iterate(|_k, v: &String| {
            sum += v.len();
            true // continue
        });
        assert_eq!(count, 3);
        assert_eq!(sum, 11); // "one" + "two" + "three" = 3 + 3 + 5
    }

    #[tokio::test]
    async fn cache_iterate_stops_early_when_returning_false() {
        let cache = Cache::new();

        // Insert 2 items
        cache.insert(1, "one".to_string(), None);
        cache.insert(2, "two".to_string(), None);

        // Use cell to track call count
        use std::cell::Cell;
        let call_count = Cell::new(0);
        let iterated = cache.iterate(|_k, _v: &String| {
            let c = call_count.get();
            call_count.set(c + 1);
            c < 1 // Return true only on first call, false after
        });

        // callback called twice (once for each entry before break)
        assert_eq!(call_count.get(), 2);
        // but only 1 item was successfully iterated (first f returned true)
        assert_eq!(iterated, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cache_iterate_excludes_items_when_expired() {
        let cache = Cache::new();

        cache.insert(1, "one".to_string(), Some(Duration::from_millis(1)));
        cache.insert(2, "two".to_string(), None);

        // Wait for item 1 to expire
        tokio::time::advance(Duration::from_millis(10)).await;

        let mut count = 0;
        cache.iterate(|_k, _v: &String| {
            count += 1;
            true
        });
        assert_eq!(count, 1);
    }
}

