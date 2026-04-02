//! Janitor thread logic and automatic expiration cleanup.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::time::{interval, MissedTickBehavior};

use super::{Item, OnEvictedCallback};

/// The janitor loop that periodically cleans up expired items.
pub async fn janitor_loop<K, V>(
    items: Arc<DashMap<K, Item<V>>>,
    cleanup_interval: Duration,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    on_evicted: OnEvictedCallback<K, V>,
) where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    tracing::debug!(
        "Janitor thread started with interval {:?}",
        cleanup_interval
    );

    let mut ticker = interval(cleanup_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::debug!("Janitor thread shutting down");
                    break;
                }
                let callback = on_evicted.lock().clone();
                delete_expired_from_map(&items, callback);
            }
            () = tokio::task::yield_now() => {
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::debug!("Janitor thread shutting down");
                    break;
                }
            }
        }
    }
}

/// Deletes all expired items from the given map and invokes callbacks.
#[allow(clippy::type_complexity)]
pub fn delete_expired_from_map<K, V>(
    items: &Arc<DashMap<K, Item<V>>>,
    on_evicted: Option<Arc<dyn Fn(&K, &V) + Send + Sync>>,
) where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    let now = tokio::time::Instant::now();

    // Collect expired items and their keys for callback
    let keys_to_remove: Vec<K> = items
        .iter()
        .filter(|entry| entry.is_expired() && entry.expiration().is_some_and(|exp| now > exp))
        .map(|entry| entry.key().clone())
        .collect();

    let evicted_count = keys_to_remove.len();
    if evicted_count > 0 {
        tracing::debug!("Janitor deleting {} expired items", evicted_count);

        // Remove expired items and collect values for callback
        let evicted: Vec<(K, V)> = keys_to_remove
            .iter()
            .filter_map(|k| items.remove(k).map(|(key, item)| (key, item.object())))
            .collect();

        // Invoke callbacks after removal
        if let Some(callback) = on_evicted {
            for (key, value) in &evicted {
                callback(key, value);
            }
        }
    }
}
