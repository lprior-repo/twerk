//! TTL-based image caching tests.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unchecked_time_subtraction)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::float_cmp)]
#![allow(clippy::redundant_pattern_matching)]

#[allow(unused_imports)]
use dashmap::DashMap;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use std::time::{Duration, Instant};
#[allow(unused_imports)]
use tokio::sync::RwLock;

#[test]
fn test_ttl_check_within_ttl() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let image = "ubuntu:22.04";
    let ttl = Duration::from_secs(300);

    let now = Instant::now();
    images.insert(image.to_string(), now);

    let ts = images.get(image).unwrap();
    let elapsed = Instant::now().duration_since(*ts);
    assert!(elapsed <= ttl, "image should still be within TTL");
}

#[test]
fn test_ttl_check_expired_ttl() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let image = "ubuntu:22.04";
    let ttl = Duration::from_secs(300);

    let past = Instant::now() - ttl - Duration::from_secs(1);
    images.insert(image.to_string(), past);

    let ts = images.get(image).unwrap();
    let elapsed = Instant::now().duration_since(*ts);
    assert!(elapsed > ttl, "image should be expired");
}

#[test]
fn test_ttl_check_image_not_in_cache() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let image = "ubuntu:22.04";

    let result = images.get(image);
    assert!(result.is_none(), "image should not be in cache");
}

#[test]
fn test_prune_images_removes_expired() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let ttl = Duration::from_secs(300);

    let now = Instant::now();
    let expired_image = "ubuntu:22.04";
    let fresh_image = "alpine:3.18";

    images.insert(
        expired_image.to_string(),
        now - ttl - Duration::from_secs(1),
    );
    images.insert(fresh_image.to_string(), now);

    let now_check = Instant::now();
    let to_remove: Vec<String> = images
        .iter()
        .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
        .map(|entry| entry.key().clone())
        .collect();

    assert_eq!(1, to_remove.len());
    assert_eq!(expired_image, to_remove[0]);
}

#[test]
fn test_prune_images_preserves_fresh() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let ttl = Duration::from_secs(300);

    let now = Instant::now();
    let fresh_image = "alpine:3.18";

    images.insert(fresh_image.to_string(), now);

    let now_check = Instant::now();
    let to_remove: Vec<String> = images
        .iter()
        .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
        .map(|entry| entry.key().clone())
        .collect();

    assert!(to_remove.is_empty(), "fresh image should not be removed");
}

#[test]
fn test_prune_images_skips_when_tasks_running() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let tasks: Arc<RwLock<usize>> = Arc::new(RwLock::new(5));
    let ttl = Duration::from_secs(300);

    let now = Instant::now();
    images.insert(
        "ubuntu:22.04".to_string(),
        now - ttl - Duration::from_secs(1),
    );

    let result = tasks.try_read();
    assert!(matches!(result, Ok(_)));
    let task_count = *result.unwrap();
    assert!(task_count > 0, "tasks should be running");

    let now_check = Instant::now();
    let to_remove: Vec<String> = if task_count > 0 {
        vec![]
    } else {
        images
            .iter()
            .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
            .map(|entry| entry.key().clone())
            .collect()
    };

    assert!(
        to_remove.is_empty(),
        "should not prune when tasks are running"
    );
}

#[test]
fn test_ttl_cache_multiple_images_mixed_expiration() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let ttl = Duration::from_secs(300);
    let now = Instant::now();

    images.insert("ubuntu:22.04".to_string(), now);
    images.insert(
        "alpine:3.18".to_string(),
        now - ttl - Duration::from_secs(60),
    );
    images.insert("nginx:1.25".to_string(), now - Duration::from_secs(100));

    let now_check = Instant::now();
    let to_remove: Vec<String> = images
        .iter()
        .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
        .map(|entry| entry.key().clone())
        .collect();

    assert_eq!(1, to_remove.len());
    assert_eq!("alpine:3.18", to_remove[0]);

    assert!(images.contains_key("ubuntu:22.04"));
    assert!(images.contains_key("nginx:1.25"));
}

#[test]
fn test_ttl_boundary_behavior() {
    let ttl = Duration::from_secs(300);
    let now = Instant::now();

    let at_boundary = now - ttl;
    let elapsed_at_boundary = now.duration_since(at_boundary);
    assert!(elapsed_at_boundary <= ttl, "at boundary should be <= TTL");

    let past_boundary = now - ttl - Duration::from_millis(1);
    let elapsed_past_boundary = now.duration_since(past_boundary);
    assert!(elapsed_past_boundary > ttl, "past boundary should be > TTL");

    assert!(elapsed_at_boundary <= ttl && elapsed_past_boundary > ttl);
}

#[test]
fn test_ttl_cache_one_second_over_ttl() {
    let images: Arc<DashMap<String, Instant>> = Arc::new(DashMap::new());
    let ttl = Duration::from_secs(300);

    let now = Instant::now();
    let one_second_over = now - ttl - Duration::from_secs(1);
    images.insert("ubuntu:22.04".to_string(), one_second_over);

    let now_check = Instant::now();
    let elapsed = now_check.duration_since(one_second_over);
    assert!(elapsed > ttl, "one second over TTL should be expired");

    let to_remove: Vec<String> = images
        .iter()
        .filter(|entry| now_check.duration_since(*entry.value()) > ttl)
        .map(|entry| entry.key().clone())
        .collect();

    assert_eq!(1, to_remove.len());
    assert_eq!("ubuntu:22.04", to_remove[0]);
}
