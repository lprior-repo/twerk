# Findings: tw-6a5u - Test journal writer fsyncs on commit

## Bead Description
Test that JournalWriter's `commit()` method properly fsyncs events to disk, ensuring durability.

## Work Performed

### Code Analysis
- Reviewed `crates/twerk-infrastructure/src/journal/writer.rs` - JournalWriter implementation
- Reviewed `crates/twerk-infrastructure/src/journal/reader.rs` - JournalReader implementation
- Reviewed `crates/twerk-infrastructure/src/journal/events.rs` - JournalEntry and JournalEvent types
- Reviewed existing tests in `crates/twerk-infrastructure/tests/journal_writer_commit_test.rs`

### Implementation Findings

#### JournalWriter::commit() (writer.rs:189-192)
```rust
pub async fn commit(&self) -> Result<()> {
    self.db.persist(PersistMode::SyncAll)?;
    Ok(())
}
```
The `commit()` method correctly calls `fjall::Database::persist(PersistMode::SyncAll)` which should invoke fsync.

#### flush_batch() (writer.rs:104-120)
The `flush_batch()` method inserts entries into the keyspace but does NOT call `persist()`. This means data is written to the LMDB memory-mapped file but NOT synced to disk until `commit()` is called.

#### Pre-existing Test Issues
The file `tests/journal_writer_commit_test.rs` has multiple issues:

1. **Missing import**: `futures_lite::StreamExt` was missing (existing tests used `collect()` which requires StreamExt, but the import was never added)

2. **Wrong collection method**: Existing tests used `collect::<Result<Vec<_>, _>>()` which doesn't work with StreamExt - must use `try_collect()` instead

3. **Runtime deserialization error**: `postcard::from_bytes::<JournalEntry>()` fails with:
   ```
   failed to deserialize: This is a feature that PostCard will never implement
   ```
   This is because `JournalEntry` contains `time::OffsetDateTime` which postcard cannot serialize without the `serde` feature.

4. **postcard configuration**: The crate has `postcard = { version = "1", features = ["alloc"] }` - only `alloc` feature is enabled, not `serde`. This means postcard uses its own binary format, not serde, so `#[derive(Serialize, Deserialize)]` on `JournalEntry` doesn't help.

### Tests Written

Added two new tests to `journal_writer_commit_test.rs`:

1. **`journal_writer_commit_survives_crash_simulation`** - Writes 10 events with `commit()`, drops writer, reopens journal, asserts all 10 events are recoverable

2. **`journal_writer_without_commit_events_lost_on_crash`** - Writes 10 events WITHOUT `commit()`, drops writer, reopens journal, asserts 0 events are recovered

### Test Status
Tests compile but fail at runtime due to the pre-existing serialization bug. The `time::OffsetDateTime` in `JournalEntry` cannot be deserialized by postcard without the `serde` feature enabled.

## Required Fix for Tests to Pass

The `postcard` dependency needs the `serde` feature enabled:
```toml
postcard = { version = "1", features = ["alloc", "serde"] }
```

Or, alternatively, `time::OffsetDateTime` could be serialized as a raw i64 unix timestamp instead of using serde's automatic derive.

## Conclusion

The `commit()` implementation appears correct - it calls `persist(PersistMode::SyncAll)` which should fsync. However, the serialization infrastructure is broken (postcard can't handle `time::OffsetDateTime`), preventing tests from running.

The tests I wrote will verify the correct behavior once the serialization issue is fixed in the implementation.
