//! Thread-safe cache with automatic expiration cleanup.
//!
//! The janitor background thread runs periodically to remove expired items.

use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use parking_lot::Mutex;
use tracing::instrument;

pub mod error;
pub mod item;
pub mod janitor;
pub mod operations;

pub use error::CacheError;
pub use item::{Expiration, Item};

/// A filter function type for use with [`Cache::list`].
/// Matches items where the function returns `true`.
pub type ListFilter<'a, V> = Box<dyn Fn(&V) -> bool + 'a>;

/// Callback type for eviction notifications, wrapped in Mutex for interior mutability.
type OnEvictedCallback<K, V> = Arc<Mutex<Option<Arc<dyn Fn(&K, &V) + Send + Sync>>>>;

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
    /// Default expiration duration for items.
    default_expiration: Option<Duration>,
    /// Callback invoked when items are evicted.
    on_evicted: OnEvictedCallback<K, V>,
}

impl<K, V> Default for Cache<K, V>
where
    K: Eq + Hash + Send + Sync + Clone + 'static,
    V: Send + Sync + Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Send + Sync + Clone + 'static,
    V: Send + Sync + Clone + 'static,
{
    /// Creates a new empty cache without a janitor thread.
    ///
    /// This is equivalent to `newCache` in the Go implementation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Arc::new(DashMap::new()),
            cleanup_interval: None,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            default_expiration: None,
            on_evicted: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates a new cache with automatic cleanup.
    ///
    /// If `cleanup_interval` is `Some(duration)`, a janitor thread is spawned
    /// that runs every `duration` to delete expired items. If `None`,
    /// no automatic cleanup is performed.
    ///
    /// If `cleanup_interval` is `Some(Duration::ZERO)`, returns a cache without
    /// a janitor (same as `new()`).
    #[must_use]
    pub fn with_cleanup(cleanup_interval: Option<Duration>) -> Self {
        if let Some(interval) = cleanup_interval {
            if interval.is_zero() {
                return Self::new();
            }

            let items = Arc::new(DashMap::<K, Item<V>>::new());
            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let on_evicted = Arc::new(Mutex::new(None));

            let items_clone = Arc::clone(&items);
            let shutdown_flag_clone = Arc::clone(&shutdown_flag);
            let on_evicted_clone = Arc::clone(&on_evicted);

            tokio::spawn(async move {
                janitor::janitor_loop(items_clone, interval, shutdown_flag_clone, on_evicted_clone)
                    .await;
            });

            Self {
                items,
                cleanup_interval,
                shutdown_flag,
                default_expiration: None,
                on_evicted,
            }
        } else {
            Self::new()
        }
    }

    /// Creates a new cache with the specified default expiration and cleanup interval.
    ///
    /// This is equivalent to `New` in the Go implementation.
    ///
    /// - `default_expiration`: Duration after which items expire. Use `None` for no default
    ///   expiration (items never expire by default). Use `Some(Duration::ZERO)` for infinite
    ///   default (items must be deleted manually).
    /// - `cleanup_interval`: Interval between automatic cleanup runs. If `None`, no
    ///   automatic cleanup is performed.
    #[must_use]
    pub fn with_expiration_and_cleanup(
        default_expiration: Option<Duration>,
        cleanup_interval: Option<Duration>,
    ) -> Self {
        if let Some(interval) = cleanup_interval {
            if interval.is_zero() {
                let mut cache = Self::new();
                cache.default_expiration = default_expiration;
                return cache;
            }

            let items = Arc::new(DashMap::<K, Item<V>>::new());
            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let on_evicted = Arc::new(Mutex::new(None));

            let items_clone = Arc::clone(&items);
            let shutdown_flag_clone = Arc::clone(&shutdown_flag);
            let on_evicted_clone = Arc::clone(&on_evicted);

            tokio::spawn(async move {
                janitor::janitor_loop(items_clone, interval, shutdown_flag_clone, on_evicted_clone)
                    .await;
            });

            Self {
                items,
                cleanup_interval,
                shutdown_flag,
                default_expiration,
                on_evicted,
            }
        } else {
            let mut cache = Self::new();
            cache.default_expiration = default_expiration;
            cache
        }
    }

    /// Sets the callback function to be called when items are evicted.
    ///
    /// This is equivalent to `OnEvicted` in the Go implementation.
    pub fn set_on_evicted<F>(&mut self, callback: F)
    where
        F: Fn(&K, &V) + Send + Sync + 'static,
    {
        let mut guard = self.on_evicted.lock();
        *guard = Some(Arc::new(callback));
    }

    /// Returns the default expiration duration.
    #[must_use]
    pub fn default_expiration(&self) -> Option<Duration> {
        self.default_expiration
    }

    /// Stops the background cleanup janitor.
    ///
    /// This is equivalent to `stopJanitor` in the Go implementation.
    pub fn stop_janitor(&self) {
        if self.cleanup_interval().is_some() {
            self.shutdown_flag.store(true, Ordering::Relaxed);
        }
    }

    /// Returns a reference to the items map.
    #[must_use]
    pub fn items(&self) -> &Arc<DashMap<K, Item<V>>> {
        &self.items
    }

    /// Returns the number of items in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the cache contains no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns `true` if the cache has a janitor thread running.
    #[must_use]
    pub fn has_janitor(&self) -> bool {
        self.cleanup_interval().is_some()
    }

    /// Deletes all expired items from the cache.
    ///
    /// This can be called manually or by the janitor thread.
    #[instrument(skip(self))]
    pub fn delete_expired(&self)
    where
        V: Clone,
    {
        let callback = self.on_evicted.lock().clone();
        janitor::delete_expired_from_map(&self.items, callback);
    }

    /// Returns the shutdown flag reference for internal use.
    fn shutdown_flag(&self) -> &Arc<AtomicBool> {
        &self.shutdown_flag
    }

    /// Returns the `on_evicted` callback reference for internal use.
    fn on_evicted(&self) -> &OnEvictedCallback<K, V> {
        &self.on_evicted
    }

    /// Returns the cleanup interval for internal use.
    pub(crate) fn cleanup_interval(&self) -> Option<Duration> {
        self.cleanup_interval
    }
}

impl<K, V> Drop for Cache<K, V> {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
