# Findings: tw-pifd - Test journal reader replays events in chronological order

## Summary

Test code written at `crates/twerk-infrastructure/src/journal/tests.rs` reveals critical findings about `JournalReader` behavior.

## Finding 1: `JournalReader::replay()` returns events in sequence number order, NOT timestamp order

**Location:** `crates/twerk-infrastructure/src/journal/reader.rs:25-53`

**Behavior:**
- Keys are `SequenceNumber.0.to_le_bytes()` - iteration is by sequence number
- `ts` (timestamp) field is stored in the value, NOT the key
- `replay()` uses `keyspace.range::<Vec<u8>, _>(..)` which iterates in key order

**Evidence:**
```rust
// reader.rs:38 - keys are sequence numbers
for guard in keyspace.range::<Vec<u8>, _>(..) {
    match guard.into_inner() {
        Ok((_key, value)) => match postcard::from_bytes::<JournalEntry>(&value) {
```

**Implication:** If timestamps are out-of-order with respect to sequence numbers (e.g., due to clock skew or concurrent writers with different system clocks), `replay()` will NOT return events in chronological timestamp order.

## Finding 2: No native seek-by-timestamp functionality exists

**Evidence:** `JournalReader` only has:
- `replay()` - returns all entries in seq order
- `replay_workflow()` - returns filtered entries for a specific workflow
- `latest_seq()` - returns highest sequence number

There is NO `seek_to()` or similar method to filter by timestamp.

## Finding 3: Pre-existing broken integration test

**Location:** `crates/twerk-infrastructure/tests/journal_reader_seek_test.rs`

This file references non-existent methods:
- `reader.seek_to(timestamp)` (line 59, 64, 69)
- `reader.next()` (line 60, 65)

These methods don't exist on `JournalReader`. This prevents the entire test suite from compiling:
```
error[E0599]: no method named `seek_to` found for struct `JournalReader`
error[E0599]: no method named `next` found for struct `JournalReader`
```

**Impact:** All `cargo test` invocations fail due to this broken file.

## Finding 4: Writer assigns timestamps via `OffsetDateTime::now_utc()`

**Location:** `crates/twerk-infrastructure/src/journal/writer.rs:80`

```rust
let entry = JournalEntry {
    seq: current_seq,
    ts: OffsetDateTime::now_utc(),  // <-- auto-generated
    event,
};
```

Sequence numbers and timestamps are monotonically coupled (seq increases with each write, timestamp is `now_utc()`). Under single-writer conditions, seq order = timestamp order. This breaks under concurrent writers with clock skew.

## Test Code Written

Added to `crates/twerk-infrastructure/src/journal/tests.rs`:

1. `test_journal_reader_replays_in_chronological_order` - verifies replay returns entries in seq order
2. `test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order` - proves out-of-order timestamps don't affect return order
3. `test_journal_reader_no_native_seek_by_timestamp` - documents that seek-by-timestamp requires manual in-memory filtering

## Recommendations

1. **Fix `journal_reader_seek_test.rs`**: Either implement `seek_to()` and `next()` methods, or remove the broken test file
2. **Clarify spec**: If chronological order means timestamp order, `JournalReader` needs redesign (use timestamp as key, or maintain a secondary index)
3. **Add seek functionality**: If timestamp-based seeking is required, implement a `replay_from_timestamp(timestamp)` method that filters entries

## Spec vs Implementation Gap

| Spec Requirement | Current Implementation | Gap |
|-----------------|----------------------|-----|
| Events in chronological order | Events in seq number order | TIMESTAMP IGNORED IN ITERATION |
| Seek to timestamp T | No seek method exists | MISSING FUNCTIONALITY |
| Out-of-order timestamps handled | Returns in seq order | BEHAVIOR DIFFERS FROM SPEC |
