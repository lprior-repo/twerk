//! Integration tests for the cache module.

#![allow(clippy::unwrap_used)]

use futures_util::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use twerk_infrastructure::cache::Cache;

#[tokio::test]
async fn cache_get_returns_none_when_key_not_present() {
    let cache: Cache<&str, String> = Cache::new();
    let result = cache.get(&"missing");
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_get_returns_value_when_key_present() {
    let cache = Cache::new();
    cache.insert("key", "value", None);
    let result = cache.get(&"key");
    assert_eq!(result, Some("value"));
}

#[tokio::test(start_paused = true)]
async fn cache_get_returns_none_when_key_expired() {
    let cache = Cache::new();
    cache.insert("key", "value", Some(Duration::from_millis(1)));
    tokio::time::advance(Duration::from_millis(10)).await;
    let result = cache.get(&"key");
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_set_stores_value_without_expiration() {
    let cache = Cache::new();
    cache.insert("key", "value", None);
    assert!(cache.contains(&"key"));
    assert_eq!(cache.len(), 1);
}

#[tokio::test]
async fn cache_set_overwrites_existing_value() {
    let cache = Cache::new();
    cache.insert("key", "old", None);
    cache.insert("key", "new", None);
    assert_eq!(cache.get(&"key"), Some("new"));
    assert_eq!(cache.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn cache_set_with_expiration_expires_correctly() {
    let cache = Cache::new();
    cache.insert("short", "value", Some(Duration::from_millis(10)));
    cache.insert("long", "value", Some(Duration::from_secs(60)));

    assert!(cache.contains(&"short"));
    assert!(cache.contains(&"long"));

    tokio::time::advance(Duration::from_millis(20)).await;

    assert!(
        !cache.contains(&"short"),
        "short-lived item should have expired"
    );
    assert!(
        cache.contains(&"long"),
        "long-lived item should still exist"
    );
}

#[tokio::test]
async fn cache_delete_removes_existing_key() {
    let cache = Cache::new();
    cache.insert("key", "value", None);
    assert!(cache.contains(&"key"));

    let removed = cache.remove(&"key");

    assert!(removed.is_some());
    assert!(!cache.contains(&"key"));
    assert!(cache.is_empty());
}

#[tokio::test]
async fn cache_delete_returns_none_for_missing_key() {
    let cache: Cache<&str, String> = Cache::new();
    let removed = cache.remove(&"missing");
    assert!(removed.is_none());
}

#[tokio::test]
async fn cache_clear_removes_all_items() {
    let cache = Cache::new();
    cache.insert("a", 1, None);
    cache.insert("b", 2, None);
    cache.insert("c", 3, None);
    assert_eq!(cache.len(), 3);

    cache.clear();

    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[tokio::test(start_paused = true)]
async fn cache_delete_expired_removes_only_expired_items() {
    let cache = Cache::new();
    cache.insert("expired1", 1, Some(Duration::from_millis(1)));
    cache.insert("permanent", 2, None);
    cache.insert("expired2", 3, Some(Duration::from_millis(1)));

    tokio::time::advance(Duration::from_millis(10)).await;
    cache.delete_expired();

    assert_eq!(cache.len(), 1);
    assert!(cache.contains(&"permanent"));
    assert!(!cache.contains(&"expired1"));
    assert!(!cache.contains(&"expired2"));
}

#[tokio::test]
async fn cache_concurrent_gets_are_safe() {
    let cache: Arc<Cache<&str, &str>> = Arc::new(Cache::new());
    cache.insert("shared", "value", None);

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            tokio::spawn(async move { cache_clone.get(&"shared") })
        })
        .collect();

    let results = join_all(handles).await;
    results
        .into_iter()
        .for_each(|result| assert_eq!(result.expect("join should succeed"), Some("value")));
}

#[tokio::test]
async fn cache_concurrent_inserts_are_safe() {
    let cache: Arc<Cache<i32, i32>> = Arc::new(Cache::new());

    let handles: Vec<_> = (0..100)
        .map(|i| {
            let cache_clone = Arc::clone(&cache);
            tokio::spawn(async move {
                cache_clone.insert(i, i * 2, None);
            })
        })
        .collect();

    join_all(handles)
        .await
        .into_iter()
        .for_each(|handle_result| {
            handle_result.expect("insert task join should succeed");
        });

    assert_eq!(cache.len(), 100);
}

#[tokio::test]
async fn cache_concurrent_mixed_operations_are_safe() {
    let cache: Arc<Cache<&str, i32>> = Arc::new(Cache::new());
    cache.insert("counter", 0i32, None);

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            tokio::spawn(async move {
                (0..10).for_each(|_| {
                    cache_clone.insert("counter", 1i32, None);
                });
            })
        })
        .collect();

    join_all(handles)
        .await
        .into_iter()
        .for_each(|handle_result| {
            handle_result.expect("mixed operation join should succeed");
        });

    assert_eq!(cache.len(), 1);
}

#[tokio::test]
async fn cache_concurrent_get_insert_remove_are_safe() {
    let cache: Arc<Cache<String, &str>> = Arc::new(Cache::new());
    cache.insert("key".to_string(), "initial", None);

    let get_handles = (0..30).map(|_| {
        let cache_clone = Arc::clone(&cache);
        tokio::spawn(async move {
            (0..10).for_each(|_| {
                cache_clone.get(&"key".to_string());
            });
        })
    });

    let insert_handles = (0..20).map(|_| {
        let cache_clone = Arc::clone(&cache);
        tokio::spawn(async move {
            (0..10).for_each(|i| {
                cache_clone.insert(format!("key_{i}"), "value", None);
            });
        })
    });

    let remove_handles = (0..10).map(|_| {
        let cache_clone = Arc::clone(&cache);
        tokio::spawn(async move {
            (0..10).for_each(|i| {
                cache_clone.remove(&format!("key_{i}"));
            });
        })
    });

    let handles: Vec<_> = get_handles
        .chain(insert_handles)
        .chain(remove_handles)
        .collect();

    join_all(handles)
        .await
        .into_iter()
        .for_each(|handle_result| {
            handle_result.expect("cache concurrent operation join should succeed");
        });

    assert!(cache.len() <= 210);
}

#[tokio::test]
async fn cache_modify_updates_value_atomically() {
    let cache: Cache<&str, i32> = Cache::new();
    cache.insert("counter", 0i32, None);

    (0..10).for_each(|_| {
        let result: Option<Result<(), String>> = cache.modify(&"counter", |v: &mut i32| {
            *v += 1;
            Ok(())
        });
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    });

    assert_eq!(cache.get(&"counter"), Some(10));
}

#[tokio::test]
async fn cache_modify_returns_none_for_missing_key() {
    let cache: Cache<&str, i32> = Cache::new();
    let result: Option<Result<(), String>> = cache.modify(&"missing", |v: &mut i32| {
        *v += 1;
        Ok(())
    });
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_modify_aborts_on_error() {
    let cache: Cache<&str, i32> = Cache::new();
    cache.insert("counter", 0i32, None);

    let result: Option<Result<(), &'static str>> =
        cache.modify(&"counter", |_v| Err("intentional error"));
    assert!(result.is_some());
    assert!(result.unwrap().is_err());

    assert_eq!(
        cache.get(&"counter"),
        Some(0),
        "value should be unchanged after error"
    );
}

#[tokio::test]
async fn cache_iterate_sums_values() {
    let cache: Cache<&str, i32> = Cache::new();
    cache.insert("a", 1, None);
    cache.insert("b", 2, None);
    cache.insert("c", 3, None);

    let mut sum = 0;
    cache.iterate(|_k, v| {
        sum += v;
        true
    });

    assert_eq!(sum, 6);
}

#[tokio::test]
async fn cache_on_evicted_callback_is_invoked() {
    let evicted_key = Arc::new(parking_lot::Mutex::new(String::new()));
    let evicted_val = Arc::new(parking_lot::Mutex::new(0));

    let k_clone = Arc::clone(&evicted_key);
    let v_clone = Arc::clone(&evicted_val);

    let mut cache: Cache<String, i32> = Cache::new();
    cache.set_on_evicted(move |k: &String, v: &i32| {
        *k_clone.lock() = k.clone();
        *v_clone.lock() = *v;
    });

    cache.insert("key".to_string(), 42, None);
    cache.remove(&"key".to_string());

    assert_eq!(*evicted_key.lock(), "key");
    assert_eq!(*evicted_val.lock(), 42);
}

#[tokio::test(start_paused = true)]
async fn cache_concurrent_delete_expired_is_safe() {
    let cache: Arc<Cache<&str, i32>> = Arc::new(Cache::new());

    cache.insert("expired", 1, Some(Duration::from_millis(1)));
    tokio::time::advance(Duration::from_millis(10)).await;

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            tokio::spawn(async move {
                cache_clone.delete_expired();
            })
        })
        .collect();

    join_all(handles)
        .await
        .into_iter()
        .for_each(|handle_result| {
            handle_result.expect("delete_expired join should succeed");
        });

    assert!(cache.is_empty());
}
