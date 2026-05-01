# Findings for Bead tw-34fc

## Task
Test journal writer batches commits for throughput in `crates/twerk-infrastructure/src/journal/writer.rs`.

## Requirements
1. Write 100 events without explicit commit
2. Call commit()
3. Measure time
4. Compare to writing+committing 100 individual events
5. Assert batch commit is >5x faster
6. Assert all 100 events are recoverable after batch commit

## Work Performed

### Code Analysis

**JournalWriter batching mechanism** (`writer.rs:64-102`):
- Events sent via mpsc channel and accumulated in `batch_keys` and `batch_values` vectors
- Auto-flush occurs when `batch_keys.len() >= batch_size`
- `commit()` method calls `self.db.persist(PersistMode::SyncAll)` which forces fsync

**Key finding**: The `flush_batch()` function at line 104-120 inserts entries one-by-one into the keyspace:
```rust
for (key, value) in batch_keys.drain(..).zip(batch_values.drain(..)) {
    keyspace.insert(key, value)?;
}
```

This is an in-memory flush to the LSM tree, but the actual fsync only happens when `commit()` is called.

### Test Implementation

Added two new tests to `crates/twerk-infrastructure/tests/journal_writer_commit_test.rs`:

1. **`journal_writer_batch_commit_throughput_vs_individual`**: Compares batch commit (100 events written then committed once) vs individual commit (100 events each with its own commit). Asserts >5x speedup.

2. **`journal_writer_batch_commit_all_events_recoverable`**: Verifies all 100 events are recoverable after batch commit with correct sequence numbers and data.

### Pre-existing Test Infrastructure Issue

**CRITICAL**: The existing journal tests are broken due to a deserialization bug:

```
failed to deserialize: This is a feature that PostCard will never implement
```

This occurs in `JournalReader::replay()` when calling `postcard::from_bytes::<JournalEntry>(&value)`.

**Root Cause**: `time::OffsetDateTime` in `JournalEntry` (events.rs:25) requires postcard's `serde` feature to properly serialize/deserialize, but the workspace Cargo.toml only enables `postcard = { version = "1", features = ["alloc"] }` without the `serde` feature.

**Affected tests** (all failing):
- `test_journal_reader_replays_in_chronological_order` (tests.rs:272)
- `test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order` (tests.rs:342)
- `test_journal_reader_no_native_seek_by_timestamp` (tests.rs:384)

### Code Changes Made

1. Added `futures_lite::stream::StreamExt` import (needed for `try_collect()`)
2. Added `tokio::time::sleep()` after `drop(writer)` to allow background task to release db lock
3. Changed `collect::<Result<Vec<_>, _>>()` to `try_collect().await.unwrap()` (idiomatic futures-lite)
4. Added two new throughput tests as described above

### Files Modified
- `crates/twerk-infrastructure/tests/journal_writer_commit_test.rs` (+212 lines)

### Recommendation

To fix the pre-existing deserialization issue, add `serde` feature to postcard in workspace Cargo.toml:
```toml
postcard = { version = "1", features = ["alloc", "serde"] }
```

This would enable proper serialization of `time::OffsetDateTime` via serde.

## Status
**Blocked by pre-existing infrastructure issue**. Tests cannot be verified until the postcard/time serialization issue is resolved.
