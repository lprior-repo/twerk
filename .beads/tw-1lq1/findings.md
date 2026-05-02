# Findings: tw-1lq1 - Test journal reader replays events in chronological order

## Summary
Tests for `JournalReader` replay behavior were already present but had bugs preventing them from passing. Fixed 3 issues to make all tests pass.

## Changes Made

### 1. Fixed `load_entries()` sort key (reader.rs:57)

**Bug**: Entries were sorted by timestamp (`e.ts`) instead of sequence number (`e.seq`).

**Impact**: When events were written with out-of-order timestamps, the reader would return them sorted by timestamp, breaking event sourcing correctness (events must be replayed in write/seq order).

**Fix**: Changed `entries.sort_by_key(|e| e.ts)` to `entries.sort_by_key(|e| e.seq)`

**Tests affected**:
- `test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order` - NOW PASSES (was failing)
- `test_journal_reader_replays_in_chronological_order` - Still passes (seq and ts were in sync)

### 2. Fixed type error in `test_journal_reader_skips_corrupt_entries` (tests.rs:426-460)

**Bug**: Test passed `OffsetDateTime` where `Timestamp` was expected.

**Impact**: Compilation failure due to type mismatch in `write_entry_direct()`.

**Fix**: Created `base_timestamp = Timestamp::from_offsetdatetime(base_ts)` and used that.

### 3. Fixed Fjall database lock issue (tests.rs:462)

**Bug**: `keyspace` handle was not explicitly dropped before `drop(db)`.

**Impact**: Fjall's file lock was not released, causing `FjallError: Locked` when `JournalReader::open()` tried to acquire the lock.

**Fix**: Added `drop(keyspace)` before `drop(db)`.

## Test Results

All 4 journal reader tests pass:
- `test_journal_reader_replays_in_chronological_order` ✓
- `test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order` ✓
- `test_journal_reader_no_native_seek_by_timestamp` ✓
- `test_journal_reader_skips_corrupt_entries` ✓

## Bead Requirements Coverage

| Requirement | Test | Status |
|-------------|------|--------|
| Write 10 events with timestamps | `test_journal_reader_replays_in_chronological_order` | ✓ |
| Read all events, assert chronological order | Same test | ✓ |
| Write event with out-of-order timestamp | `test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order` | ✓ |
| Assert reader returns in write order | Same test | ✓ |
| Seek to timestamp T | `test_journal_reader_no_native_seek_by_timestamp` | ✓ |
| Assert only events after T returned | Same test | ✓ |
