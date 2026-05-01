# Findings for Bead tw-5nmw

## Bead Description
Test journal writer batches commits for throughput. In `crates/twerk-infrastructure/src/journal/writer.rs`, JournalWriter batches writes before fsync. Write test:
1. write 100 events without explicit commit
2. call commit()
3. measure time
4. compare to writing+committing 100 individual events
5. assert batch commit is >5x faster
6. assert all 100 events recoverable after batch commit

## Investigation Summary

### Test Written
Added `journal_writer_batch_commit_throughput_vs_individual_commits` test to `crates/twerk-infrastructure/tests/journal_writer_commit_test.rs`. The test:
- Writes 100 events with batch_size=200 (so events stay in buffer, don't auto-flush)
- Calls commit() to trigger persist
- Measures batch commit time
- Compares to individual commits (write 1, commit 1, repeat 100 times)
- Asserts batch is >5x faster
- Asserts all 100 events are recoverable

### Critical Bug Found: Postcard Serialization

**ALL journal tests are failing** with deserialization error:
```
failed to deserialize: This is a feature that PostCard will never implement
```

This error occurs when `JournalReader::replay()` calls `postcard::from_bytes::<JournalEntry>(&value)`.

**Root Cause**: The `time::OffsetDateTime` type (used in `JournalEntry.ts`) requires full serde support, but `postcard` in this workspace only has `features = ["alloc"]`. Without the `serde` feature, postcard cannot deserialize types that rely on serde's deserialize trait.

**Workspace dependency** (`Cargo.toml:94`):
```toml
postcard = { version = "1", features = ["alloc"] }
```

### Secondary Issue: JournalWriter Commit() Does Not Flush Channel Buffer

Even if serialization worked, there is a design issue in `JournalWriter`:

The `commit()` method only calls `self.db.persist(PersistMode::SyncAll)`. This persists data **already in the keyspace** to disk. However, events sent via `workflow_started()` etc. are buffered in a tokio channel and processed asynchronously by a spawned task. They are NOT in the keyspace until:
1. The batch is full (`batch_keys.len() >= batch_size`)
2. The channel is closed (writer dropped)

So calling `commit()` immediately after sending events does NOT make the events durable - they are still in the channel buffer and lost on crash.

**Evidence from code** (`writer.rs:189-192`):
```rust
pub async fn commit(&self) -> Result<()> {
    self.db.persist(PersistMode::SyncAll)?;  // Only persists keyspace, not channel!
    Ok(())
}
```

The spawned write loop (`writer.rs:64-102`) only flushes to keyspace when:
- Batch reaches capacity (line 88-90)
- Channel closes (line 92-96)

### Test Infrastructure Issues

The existing tests in `tests/journal_writer_commit_test.rs` also fail with:
- `FjallError: Locked` - database lock not released before reader opens
- Deserialization errors

The lib tests in `src/journal/tests.rs` also fail with deserialization.

### Bead Task Analysis

The bead asks to verify that "batched fsync reduces syscall overhead". However:
1. The serialization bug prevents any tests from running
2. Even if fixed, `commit()` doesn't actually batch the way the test expects

**What the test SHOULD verify** (if infrastructure worked):
- Events are in the channel buffer after `send()` calls
- `commit()` only syncs what is already in the keyspace
- To truly batch, events must be flushed to keyspace BEFORE commit()

## Discovered Issues

**bd create "journal: postcard missing serde feature for time::OffsetDateTime deserialization" --type=bug --priority=1 --rig twerk --json**

## Test Code Added

```rust
#[tokio::test]
async fn journal_writer_batch_commit_throughput_vs_individual_commits() {
    let event_count = 100;
    let batch_size = event_count * 2;  // 200, so 100 events don't auto-flush

    // ... (full test in tests/journal_writer_commit_test.rs)
}
```

Test was written but cannot run due to infrastructure bugs.

## Recommendations

1. Add `serde` feature to postcard workspace dependency:
   ```toml
   postcard = { version = "1", features = ["alloc", "serde"] }
   ```

2. Consider adding a `flush()` method to `JournalWriter` that triggers channel buffer flush before commit

3. Write integration tests that verify the actual batching behavior, not just commit timing
