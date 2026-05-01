# Findings: tw-otjr - Test broker subscription filter with regex patterns

## Task Summary
Write tests for `subscribe()` method in `crates/twerk-infrastructure/src/broker/inmemory/` that verify pattern filtering with regex patterns.

## Tests Written

Added 5 new tests to `crates/twerk-infrastructure/src/broker/inmemory/tests.rs`:

1. **`test_subscribe_returns_broadcast_receiver`** - Verifies `subscribe()` returns a working broadcast receiver
2. **`test_subscribe_pattern_matches_order_created`** - Subscribe with `order.*` pattern, publish to `order.created` -> should receive
3. **`test_subscribe_pattern_matches_order_updated`** - Subscribe with `order.*` pattern, publish to `order.updated` -> should receive
4. **`test_subscribe_pattern_does_not_match_payment_created`** - Subscribe with `order.*` pattern, publish to `payment.created` -> should NOT receive
5. **`test_subscribe_accepts_pattern_without_immediate_validation`** - Subscribe with `[invalid` pattern -> no error (patterns not validated at subscribe time)

## Key Findings

### Finding 1: Pattern Matching is Wildcard, Not Regex

The issue description states "subscribe() accepts regex patterns", but the actual implementation uses **wildcard matching** (glob-style with `*` as the wildcard character).

**Code location**: `crates/twerk-infrastructure/src/broker/inmemory/publish.rs:183`
```rust
if wildcard_match(pattern, &topic) {
```

The `wildcard_match` function in `crates/twerk-common/src/wildcard.rs` implements glob-style matching, not regex:
- `*` matches any sequence of characters
- No special regex syntax support (`.`, `\d`, `+`, `?`, etc.)

**Impact**: Tests use wildcard syntax (`order.*`) not regex syntax (`order\\..*`). The behavior is equivalent for simple patterns like `order.*` matching `order.created`, but regex-specific patterns like `order\\..*` (escaped dot) would not work as expected with wildcard matching.

### Finding 2: No Pattern Validation

The `subscribe()` method does NOT validate patterns at subscription time. The pattern is stored as-is and used later during event publication for matching.

**Code location**: `crates/twerk-infrastructure/src/broker/inmemory/subscription.rs:115-131`
```rust
pub(crate) fn typed_events(
    broker: &InMemoryBroker,
    pattern: &str,
) -> BoxedFuture<tokio::sync::broadcast::Receiver<JobEvent>> {
    let pattern = pattern.to_string();
    let channels = broker.typed_event_channels.clone();
    Box::pin(async move {
        let rx = channels
            .entry(pattern)
            .or_insert_with(|| {
                let (tx, _rx) = tokio::sync::broadcast::channel(256);
                tx
            })
            .subscribe();
        Ok(rx)
    })
}
```

**Impact**: Invalid regex patterns like `[invalid` do NOT return `Err::InvalidPattern` as the issue description suggests. They are stored and used as literal strings.

### Finding 3: Event Broadcast Requires JobEvent Conversion

For `subscribe()` to receive events via the broadcast channel, the published event must:
1. Deserialize as a `Job` from the JSON value
2. Have a state that produces a `JobEvent` via `job_event_from_state()`

**States that produce events**: `COMPLETED`, `FAILED`, `CANCELLED`
**States that return None**: `PENDING`, `SCHEDULED`, `RUNNING`, `RESTART`

**Code location**: `crates/twerk-core/src/job.rs:170-177`

### Finding 4: Pre-existing Build Issue - Missing `slot` Module

The codebase has a pre-existing build issue: `twerk-common/src/lib.rs` references a `slot` module that doesn't exist:
```
pub mod slot;
```

This prevents running `cargo test` on the entire workspace.

## Tests Status

Unable to execute tests due to pre-existing build issue. Tests were written following the existing test patterns in the file and should compile correctly when the `slot` module issue is resolved.

## Recommendations

1. **Clarify pattern type**: If regex support is required, the implementation needs to change from `wildcard_match` to regex matching
2. **Add pattern validation**: If `Err::InvalidPattern` is desired, add validation in `subscribe()` or `typed_events()`
3. **Fix `slot` module**: The missing module in `twerk-common` needs to be created or the reference removed
