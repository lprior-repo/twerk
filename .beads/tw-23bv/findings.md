# Bead tw-23bv Findings

## Task
Test compaction reclaims disk space from deleted keys in twerk-store

## Approach
- Examined `crates/twerk-store/src/lib.rs` which wraps `fjall::SingleWriterTxDatabase`
- Found `SingleWriterTxDatabase::disk_space()` method to measure disk usage
- No explicit `compact()` method exists - fjall uses automatic background compaction
- Wrote test that inserts 1000 keys, deletes 500, reopens store, and verifies space reclamation

## Key Findings

### fjall API
- `SingleWriterTxDatabase::disk_space()` returns `Result<u64>` with disk usage
- Compaction is automatic (LSM tree background compaction) - no explicit trigger
- Deleted keys are stored as tombstones and cleaned up during compaction

### Test Implementation
- Insert 1000 keys with 1000-byte values
- Delete 500 keys (keys 0-499)
- Reopen store to trigger disk flush
- Verify:
  1. Remaining 500 keys still readable
  2. Deleted keys return None
  3. Disk space reduced by ~50%

### Files Modified
- `crates/twerk-store/src/lib.rs`: Added `test_compaction_reclaims_disk_space_from_deleted_keys` test

## Test Results
- All 5 tests pass (4 existing + 1 new)
- The test validates that compaction reclaims disk space from deleted keys
- Space reclamation verified through `disk_space()` measurements before and after store reopen

## Limitations
- fjall's compaction is automatic and runs in background - no explicit `compact()` API
- Test relies on reopen to ensure data is flushed to disk
- Actual compaction timing depends on internal fjall thresholds
