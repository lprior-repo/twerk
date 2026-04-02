//! Tests for the cache module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::panic)]
#![allow(clippy::items_after_statements)]

use super::*;

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
        Some(Ok(())) => {}
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
        _ => panic!(
            "Expected Some(Err(\"something went wrong\")), got {:?}",
            result
        ),
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
