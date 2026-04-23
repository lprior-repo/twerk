//! Kani proof harnesses for `broker::utils`.
//!
//! Covers:
//! - `prefixed_queue`: empty engine_id returns queue unchanged
//! - `extract_engine_id` / `prefixed_queue` round-trip property

use twerk_infrastructure::broker::{extract_engine_id, prefixed_queue};

#[kani::proof]
fn prefixed_queue_empty_engine_returns_original() {
    let queue = "x-jobs";
    let result = prefixed_queue(queue, "");
    assert_eq!(result, queue);
}

#[kani::proof]
fn prefixed_queue_whitespace_engine_returns_original() {
    let queue = "x-jobs";
    let result = prefixed_queue(queue, "   ");
    assert_eq!(result, queue);
}

#[kani::proof]
fn prefixed_queue_nonempty_appends_dot_engine() {
    let queue = "x-jobs";
    let result = prefixed_queue(queue, "engine-abc");
    assert_eq!(result, "x-jobs.engine-abc");
}

#[kani::proof]
fn extract_engine_id_returns_none_for_unprefixed() {
    assert!(extract_engine_id("x-jobs").is_none());
    assert!(extract_engine_id("default").is_none());
    assert!(extract_engine_id("my-queue").is_none());
}

#[kani::proof]
fn extract_engine_id_roundtrip() {
    let queue = "x-jobs";
    let engine_id = "test-abc";
    let prefixed = prefixed_queue(queue, engine_id);
    let extracted = extract_engine_id(&prefixed);
    assert_eq!(extracted, Some(engine_id.to_string()));
}

#[kani::proof]
fn extract_engine_id_roundtrip_all_coordinator_queues() {
    let coordinator_queues = [
        "x-completed",
        "x-failed",
        "x-started",
        "x-heartbeat",
        "x-jobs",
        "x-progress",
        "x-task_log_part",
        "x-redeliveries",
    ];
    let engine_id = "engine-xyz";

    for queue in coordinator_queues {
        let prefixed = prefixed_queue(queue, engine_id);
        let extracted = extract_engine_id(&prefixed);
        assert_eq!(
            extracted,
            Some(engine_id.to_string()),
            "round-trip failed for queue '{queue}'"
        );
        // Re-prefixing should be idempotent
        let round_tripped = prefixed_queue(queue, &extracted.unwrap());
        assert_eq!(prefixed, round_tripped);
    }
}
