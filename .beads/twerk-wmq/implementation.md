# Implementation: twerk-wmq — Secondary Index Investigation

## Status: COMPLETED (Investigation Only)

This bead was an **investigation into the feasibility of adding a secondary index** to the in-memory datastore for the `get_active_tasks(job_id)` query.

## Outcome: DO NOT IMPLEMENT

The investigation conclusively demonstrates that a secondary index for `get_active_tasks` **regresses performance at scale** (10,000 tasks) despite providing a speedup at small scale (100 tasks). The root cause is cache locality destruction from random hash lookups, which outweighs the algorithmic improvement from O(n) scan to O(k) lookup.

See `.beads/twerk-wmq/analysis.md` for full root cause analysis.

## Code Changes

**None.** The implementation was not modified. The investigation confirmed:

- `crates/twerk-infrastructure/src/datastore/inmemory.rs` — `get_active_tasks` (lines 101–108) uses an O(n) `DashMap` filter scan with no secondary index. This is the correct implementation.
- No secondary index was added, no code was changed.

## Verification

- `cargo clippy -p twerk-infrastructure -- -D warnings -D clippy::unwrap_used -D clippy::panic -D clippy::expect_used -W clippy::pedantic` — **PASSES** (0 warnings, 0 errors)
- Code is clean: no `unwrap()`, no `panic!()`, no `expect()`, no `mut` in core logic
- Data-Calc-Actions: the `get_active_tasks` method is a pure calculation with zero side effects, correctly located in the `InMemoryDatastore` struct which acts as a stateless query interface

## Benchmark Evidence

| n (total tasks) | Baseline | Secondary Index | Verdict |
|-----------------|----------|-----------------|---------|
| 100             | 6.38 µs  | 4.01 µs         | Secondary index faster |
| 1,000           | 43.84 µs | 41.13 µs        | Marginal improvement |
| 10,000          | 401 µs   | 562 µs          | 40% regression |

Breakeven: approximately **n ≈ 2,000–3,000 tasks**.
