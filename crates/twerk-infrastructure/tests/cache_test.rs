//! Integration tests for the cache module.

use std::time::Duration;
use tokio::time::timeout;
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

#[tokio::test]
async fn cache_get_returns_none_when_key_expired() {
    let cache = Cache::new();
    cache.insert("key", "value", Some(Duration::from_millis(1)));
    tokio::time::sleep(Duration::from_millis(10)).await;
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

#[tokio::test]
async fn cache_set_with_expiration_expires_correctly() {
    let cache = Cache::new();
    cache.insert("short", "value", Some(Duration::from_millis(10)));
    cache.insert("long", "value", Some(Duration::from_secs(60)));

    assert!(cache.contains(&"short"));
    assert!(cache.contains(&"long"));

    tokio::time::sleep(Duration::from_millis(20)).await;

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

#[tokio::test]
async fn cache_expiration_removes_item_after_ttl() {
    let cache = Cache::new();
    cache.insert("key", "value", Some(Duration::from_millis(50)));

    assert!(cache.contains(&"key"));

    tokio::time::sleep(Duration::from_millis(60)).await;

    assert!(
        !cache.contains(&"key"),
        "item should have expired after TTL"
    );
}

#[tokio::test]
async fn cache_expiration_respects_individual_ttls() {
    let cache = Cache::new();
    cache.insert("fast", "fast_value", Some(Duration::from_millis(20)));
    cache.insert("slow", "slow_value", Some(Duration::from_millis(100)));

    tokio::time::sleep(Duration::from_millis(30)).await;

    assert!(!cache.contains(&"fast"), "fast item should be expired");
    assert!(cache.contains(&"slow"), "slow item should still exist");
}

#[tokio::test]
async fn cache_expiration_none_means_no_expiration() {
    let cache = Cache::new();
    cache.insert("permanent", "value", None);

    tokio::time::sleep(Duration::from_secs(10)).await;

    assert!(
        cache.contains(&"permanent"),
        "item with no expiration should persist"
    );
}

#[tokio::test]
async fn cache_exists_returns_true_for_present_key() {
    let cache = Cache::new();
    cache.insert("key", "value", None);
    assert!(cache.contains(&"key"));
}

#[tokio::test]
async fn cache_exists_returns_false_for_missing_key() {
    let cache: Cache<&str, String> = Cache::new();
    assert!(!cache.contains(&"missing"));
}

#[tokio::test]
async fn cache_exists_returns_false_for_expired_key() {
    let cache = Cache::new();
    cache.insert("key", "value", Some(Duration::from_millis(1)));
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!cache.contains(&"key"));
}

#[tokio::test]
async fn cache_keys_iterates_over_all_keys() {
    let cache = Cache::new();
    cache.insert("a", 1, None);
    cache.insert("b", 2, None);
    cache.insert("c", 3, None);

    let mut keys = Vec::new();
    cache.iterate(|k, _v| {
        keys.push(k.clone());
        true
    });

    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"a"));
    assert!(keys.contains(&"b"));
    assert!(keys.contains(&"c"));
}

#[tokio::test]
async fn cache_values_iterates_over_all_values() {
    let cache = Cache::new();
    cache.insert("a", "first", None);
    cache.insert("b", "second", None);
    cache.insert("c", "third", None);

    let values: Vec<&str> = cache.list(&[]);

    assert_eq!(values.len(), 3);
    assert!(values.contains(&"first"));
    assert!(values.contains(&"second"));
    assert!(values.contains(&"third"));
}

#[tokio::test]
async fn cache_items_returns_key_value_pairs() {
    let cache = Cache::new();
    cache.insert("key1", "value1", None);
    cache.insert("key2", "value2", None);

    let mut items = Vec::new();
    cache.iterate(|k, v| {
        items.push((k.clone(), v.clone()));
        true
    });

    assert_eq!(items.len(), 2);
    assert!(items.contains(&("key1", "value1")));
    assert!(items.contains(&("key2", "value2")));
}

#[tokio::test(start_paused = true)]
async fn cache_janitor_removes_expired_items_automatically() {
    let cache = Cache::with_cleanup(Some(Duration::from_millis(20)));
    cache.insert("expiring", "value", Some(Duration::from_millis(5)));

    assert!(cache.contains(&"expiring"));

    tokio::time::advance(Duration::from_millis(30)).await;
    tokio::task::yield_now().await;

    assert!(
        !cache.contains(&"expiring"),
        "janitor should have removed expired item"
    );
}

#[tokio::test]
async fn cache_concurrent_gets_are_safe() {
    let cache = Cache::new();
    cache.insert("shared", "value", None);

    let mut handles = Vec::new();
    for _ in 0..100 {
        let cache = &cache;
        let handle = tokio::spawn(async move { cache.get(&"shared") });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert_eq!(result, Some("value"));
    }
}

#[tokio::test]
async fn cache_concurrent_inserts_are_safe() {
    let cache = Cache::new();

    let mut handles = Vec::new();
    for i in 0..100 {
        let cache = &cache;
        let handle = tokio::spawn(async move {
            cache.insert(i, i * 2, None);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(cache.len(), 100);
}

#[tokio::test]
async fn cache_concurrent_mixed_operations_are_safe() {
    let cache = Cache::new();
    cache.insert("counter", 0i32, None);

    let mut handles = Vec::new();
    for _ in 0..50 {
        let cache = &cache;
        let handle = tokio::spawn(async move {
            for _ in 0..10 {
                cache.insert("counter", 1i32, None);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(cache.len(), 1);
}

#[tokio::test]
async fn cache_concurrent_get_insert_remove_are_safe() {
    let cache = Cache::new();
    cache.insert("key", "initial", None);

    let mut handles = Vec::new();

    for _ in 0..30 {
        let cache = &cache;
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                let _ = cache.get(&"key");
            }
        }));
    }

    for _ in 0..20 {
        let cache = &cache;
        handles.push(tokio::spawn(async move {
            for i in 0..10 {
                cache.insert(format!("key_{}", i), "value", None);
            }
        }));
    }

    for _ in 0..10 {
        let cache = &cache;
        handles.push(tokio::spawn(async move {
            for i in 0..10 {
                let _ = cache.remove(&format!("key_{}", i));
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    assert!(cache.len() <= 210);
}

#[tokio::test]
async fn cache_stats_reflects_correct_counts() {
    let cache = Cache::new();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());

    cache.insert("a", 1, None);
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());

    cache.insert("b", 2, None);
    cache.insert("c", 3, None);
    assert_eq!(cache.len(), 3);

    cache.remove(&"b");
    assert_eq!(cache.len(), 2);

    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
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
async fn cache_modify_updates_value_atomically() {
    let cache = Cache::new();
    cache.insert("counter", 0i32, None);

    for _ in 0..10 {
        let result = cache.modify(&"counter", |v| {
            *v += 1;
            Ok(())
        });
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
    }

    assert_eq!(cache.get(&"counter"), Some(10));
}

#[tokio::test]
async fn cache_modify_returns_none_for_missing_key() {
    let cache = Cache::new();
    let result = cache.modify(&"missing", |v| {
        *v += 1;
        Ok(())
    });
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_modify_aborts_on_error() {
    let cache = Cache::new();
    cache.insert("counter", 0i32, None);

    let result = cache.modify(&"counter", |_v| -> Result<(), &'static str> { Err("intentional error") });
    assert!(result.is_some());
    assert!(result.unwrap().is_err());

    assert_eq!(
        cache.get(&"counter"),
        Some(0),
        "value should be unchanged after error"
    );
}

#[tokio::test(start_paused = true)]
async fn cache_modify_with_existing_expired_key_returns_none() {
    let cache = Cache::new();
    cache.insert("key", 10i32, Some(Duration::from_millis(1)));

    tokio::time::advance(Duration::from_millis(10)).await;

    let result = cache.modify(&"key", |v| {
        *v += 1;
        Ok(())
    });

    assert!(
        result.is_none(),
        "modify should return none for expired key"
    );
}

#[tokio::test]
async fn cache_list_returns_all_non_expired_items() {
    let cache = Cache::new();
    cache.insert("a", "first", None);
    cache.insert("b", "second", None);
    cache.insert("c", "third", None);

    let items = cache.list(&[]);
    assert_eq!(items.len(), 3);
}

#[tokio::test(start_paused = true)]
async fn cache_list_excludes_expired_items() {
    let cache = Cache::new();
    cache.insert("valid", "value", None);
    cache.insert("expired", "value", Some(Duration::from_millis(1)));

    tokio::time::advance(Duration::from_millis(10)).await;

    let items = cache.list(&[]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], "value");
}

#[tokio::test]
async fn cache_list_with_filter_returns_matching_items() {
    let cache = Cache::new();
    cache.insert("a", 10i32, None);
    cache.insert("b", 20i32, None);
    cache.insert("c", 30i32, None);
    cache.insert("d", 40i32, None);

    let is_large = Box::new(|v: &i32| *v >= 20);
    let items = cache.list(&[is_large]);

    assert_eq!(items.len(), 3);
    assert!(!items.contains(&10));
    assert!(items.contains(&20));
    assert!(items.contains(&30));
    assert!(items.contains(&40));
}

#[tokio::test]
async fn cache_list_with_multiple_filters_returns_intersection() {
    let cache = Cache::new();
    cache.insert("a", 10i32, None);
    cache.insert("b", 20i32, None);
    cache.insert("c", 30i32, None);
    cache.insert("d", 40i32, None);

    let ge20 = Box::new(|v: &i32| *v >= 20);
    let lt40 = Box::new(|v: &i32| *v < 40);
    let items = cache.list(&[ge20, lt40]);

    assert_eq!(items.len(), 2);
    assert!(items.contains(&20));
    assert!(items.contains(&30));
}

#[tokio::test]
async fn cache_iterate_visits_all_items() {
    let cache = Cache::new();
    cache.insert("a", 1, None);
    cache.insert("b", 2, None);
    cache.insert("c", 3, None);

    let mut count = 0;
    let sum = cache.iterate(|_k, v| {
        count += 1;
        true
    });

    assert_eq!(count, 3);
    assert_eq!(sum, 3);
}

#[tokio::test]
async fn cache_iterate_stops_early_when_callback_returns_false() {
    let cache = Cache::new();
    cache.insert("a", 1, None);
    cache.insert("b", 2, None);
    cache.insert("c", 3, None);

    let mut call_count = 0;
    let iterated = cache.iterate(|_k, _v| {
        call_count += 1;
        call_count < 2
    });

    assert_eq!(call_count, 2, "callback should be called twice");
    assert_eq!(iterated, 1, "only 1 item should be fully iterated");
}

#[tokio::test(start_paused = true)]
async fn cache_iterate_skips_expired_items() {
    let cache = Cache::new();
    cache.insert("valid", "value", None);
    cache.insert("expired", "value", Some(Duration::from_millis(1)));

    tokio::time::advance(Duration::from_millis(10)).await;

    let mut count = 0;
    cache.iterate(|_k, _v| {
        count += 1;
        true
    });

    assert_eq!(count, 1);
}

#[tokio::test]
async fn cache_set_expiration_updates_existing_key() {
    let cache = Cache::new();
    cache.insert("key", "value", Some(Duration::from_secs(60)));

    assert!(cache.contains(&"key"));

    let result = cache.set_expiration(&"key", Duration::from_millis(1));
    assert!(result);

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        !cache.contains(&"key"),
        "item should have expired with new shorter TTL"
    );
}

#[tokio::test]
async fn cache_set_expiration_returns_false_for_missing_key() {
    let cache = Cache::new();
    let result = cache.set_expiration(&"missing", Duration::from_secs(10));
    assert!(!result);
}

#[tokio::test]
async fn cache_close_stops_janitor_but_keeps_items() {
    let cache = Cache::with_cleanup(Some(Duration::from_millis(10)));
    cache.insert("key", "value", None);

    cache.close();

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(cache.len(), 1, "items should be retained after close");
}

#[tokio::test]
async fn cache_with_zero_cleanup_interval_panics() {
    let result = std::panic::catch_unwind(|| {
        let _cache: Cache<i32, i32> = Cache::with_cleanup(Some(Duration::ZERO));
    });
    assert!(
        result.is_err(),
        "creating cache with zero cleanup interval should panic"
    );
}

#[tokio::test]
async fn cache_default_is_empty_without_janitor() {
    let cache: Cache<i32, i32> = Cache::default();
    assert!(!cache.has_janitor());
    assert!(cache.is_empty());
}

#[tokio::test(start_paused = true)]
async fn cache_janitor_respects_cleanup_interval() {
    let cache = Cache::with_cleanup(Some(Duration::from_millis(50)));
    cache.insert("key", "value", Some(Duration::from_millis(10)));

    tokio::time::advance(Duration::from_millis(30)).await;
    tokio::task::yield_now().await;
    assert!(
        cache.contains(&"key"),
        "item should not expire before janitor runs"
    );

    tokio::time::advance(Duration::from_millis(30)).await;
    tokio::task::yield_now().await;
    assert!(!cache.contains(&"key"), "item should be cleaned by janitor");
}

#[tokio::test]
async fn cache_replace_returns_old_value() {
    let cache = Cache::new();
    cache.insert("key", "old", None);

    let old = cache.insert("key", "new", None);

    assert!(old.is_some());
    assert_eq!(cache.get(&"key"), Some("new"));
}

#[tokio::test]
async fn cache_insert_none_expiration_never_expires() {
    let cache = Cache::new();
    cache.insert("key", "value", None);

    tokio::time::sleep(Duration::from_secs(100)).await;

    assert_eq!(cache.get(&"key"), Some("value"));
}

#[tokio::test]
async fn cache_concurrent_delete_expired_is_safe() {
    let cache = Cache::new();

    cache.insert("expired", 1, Some(Duration::from_millis(1)));
    tokio::time::sleep(Duration::from_millis(10)).await;

    let mut handles = Vec::new();
    for _ in 0..100 {
        let cache = &cache;
        handles.push(tokio::spawn(async move {
            cache.delete_expired();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert!(cache.is_empty());
}

#[tokio::test]
async fn cache_multiple_operations_maintain_consistency() {
    let cache = Cache::new();

    for i in 0..1000 {
        cache.insert(i, i, None);
    }
    assert_eq!(cache.len(), 1000);

    for i in 0..500 {
        cache.remove(&i);
    }
    assert_eq!(cache.len(), 500);

    for i in 500..1000 {
        assert!(cache.contains(&i));
    }

    cache.clear();
    assert!(cache.is_empty());
}
