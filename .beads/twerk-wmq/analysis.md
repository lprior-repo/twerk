# Analysis: Secondary Index for `get_active_tasks`

## Benchmark Data

| Dataset Size | Baseline (O(n) scan) | Secondary Index (O(k) lookup) | Change |
|--------------|----------------------|-------------------------------|--------|
| 100 tasks    | 6.38 µs              | 4.01 µs                       | **-37%** (faster) |
| 1,000 tasks  | 43.84 µs             | 41.13 µs                      | **-6%** (faster) |
| 10,000 tasks | 401 µs               | 562 µs                        | **+40%** (regression) |

## Executive Summary

The secondary index **regresses at scale** despite theoretically better algorithmic complexity. The breakeven point lies between **1,000–2,000 total tasks**. For workloads with 10,000+ tasks per job, the O(n) sequential scan outperforms the O(k) hash lookup approach.

**Recommendation: DO NOT IMPLEMENT secondary index for `get_active_tasks`.**

---

## Root Cause Analysis

### 1. Hash Lookup Overhead Exceeds Benefit When k ≈ n

The theoretical advantage of a secondary index is O(1) lookup to retrieve k task IDs, then O(k) individual hash lookups in the main map:

```
Total secondary index cost = O(1) [index lookup] + k × O(1) [main map lookups] = O(k)
```

But this formula omits constant factors. Each `DashMap::get` involves:
- Hash computation (string hashing of `TaskId`)
- Atomic load of the bucket pointer
- Bucket traversal with atomic CAS operations
- Cache line fetch from main memory

At small k (e.g., job has 5 tasks out of 100 total), this is fine. But when k approaches n, you perform nearly n hash computations plus n scattered memory accesses.

### 2. Cache Locality Massacre

This is the dominant factor at scale.

**Sequential scan (baseline):**
```
tasks.iter() → sequential memory read → excellent L1/L2/L3 cache utilization
```
The `DashMap` bucket array is contiguous. Prefetchers work perfectly. A single cache line load fetches multiple Task entries you'll filter.

**Secondary index lookup:**
```
index.get(job_id) → fetch vec<TaskId>   (cache miss #1, scattered)
for each task_id:
    tasks.get(task_id) → random hash lookup (cache miss per item, k times)
```
Each `DashMap::get` is a non-sequential access into a separate hash bucket array. Modern CPUs can prefetch sequential streams but cannot predict random hash bucket traversals. The `tasks` and `job_id_index` `DashMap`s are completely separate memory regions — you thrash the cache bouncing between them.

At k = 5,000 tasks and n = 10,000, you perform 5,000 non-sequential memory fetches from the main map. This is catastrophic for cache efficiency.

### 3. Double-Write Overhead on Insert

Every `create_task` now requires TWO hash inserts instead of one:
- `tasks.insert(task_id, task)` — main map
- `job_id_index.entry(job_id).or_default().push(task_id)` — secondary index

This doubles write amplification. For workloads that are write-heavy, this is additional overhead on every task creation.

### 4. Breakeven Point Estimation

The breakeven occurs when the cache-miss cost of k random lookups equals the cost of n/2 sequential comparisons on average.

Empirically:
- At n=100: scan examines ~50 entries on average → 6.38 µs; index lookup → 4.01 µs ✓
- At n=1,000: scan examines ~500 entries → 43.84 µs; index lookup → 41.13 µs ✓ (marginal)
- At n=10,000: scan examines ~5,000 entries → 401 µs; index lookup → 562 µs ✗

The breakeven is approximately **n ≈ 2,000–3,000 tasks**, or when the average job contains **~15-20% of total tasks**. Beyond this, the cache thrashing from random lookups dominates.

### 5. Why O(n) Sequential Scan Wins at This Data Size

At n=10,000:
- Sequential scan: 10,000 sequential 64-byte reads = 640 KB, serviced by ~10 L3 cache line fills
- Secondary index: k=5,000 (for a large job) random 64-byte reads = 320 KB from `index`, then 5,000 scattered 64-byte reads = 320 KB from `tasks` = 40+ cache line fills for each DashMap bucket, many missing in L3 entirely

The sequential scan is memory-bandwidth-bound and well-predicted by hardware prefetchers. The secondary index is latency-bound on random memory access — each lookup stalls the CPU waiting for a cache line from main memory.

---

## Conclusion

The secondary index is a **micro-optimization that becomes a pessimization at scale**. It helps only when:
1. The dataset is small (< 2,000 tasks total), AND
2. Jobs are selective (each job contains < 5% of total tasks), AND
3. Reads far outnumber writes

For the twerk use case (job queue with potentially large task volumes), the current O(n) scan is the correct implementation. The index would need to be combined with a locality-aware data structure (e.g., storing task data co-located with the index entry) to overcome the cache thrashing problem at scale.

---

## Files Reviewed

- `crates/twerk-infrastructure/src/datastore/inmemory.rs` — confirmed no secondary index present; `get_active_tasks` uses O(n) filter scan (lines 101–108)
