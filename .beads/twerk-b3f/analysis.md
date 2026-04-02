# Analysis: Reduce Clones in Task Creation Hot Path

## Executive Summary

**Claim Status: INVALID** — The optimization using `Cow`/`Arc-swap` would NOT significantly improve performance. The real bottleneck is the **per-item HashMap clone** in `each.rs` which cannot be solved with `Cow`/`Arc-swap` patterns without architectural changes.

## Hot Path Analysis

### Clone Count Per Task (parallel.rs)

For each subtask created in `schedule_parallel_task`:

| Line | Operation | Type | Allocation |
|------|-----------|------|------------|
| 57 | `evaluate_task(t, &job_ctx)` | Full Task clone | ~50+ fields cloned |
| 61 | `job_id.to_string().into()` | Redundant String alloc | 1 × String |
| 62 | `task_id.to_string().into()` | Redundant String alloc | 1 × String |
| 63 | `TASK_STATE_PENDING.to_string()` | Required String | 1 × String |
| 84 | `error_msg.clone()` (rollback only) | Per-failed-task | 1 × String |

**Total per subtask (parallel): ~54 allocations**

### Clone Count Per Task (each.rs - spawn_each_tasks)

| Line | Operation | Type | Allocation |
|------|-----------|------|------------|
| 109 | `job_ctx.clone()` | **MAJOR: Full HashMap clone** | M × (key + Value) |
| 120 | `evaluate_task(template, &cx)` | Full Task clone | ~50+ fields cloned |
| 125 | `job_id.to_string().into()` | Redundant String alloc | 1 × String |
| 126 | `task_id.to_string().into()` | Redundant String alloc | 1 × String |
| 127 | `TASK_STATE_PENDING.to_string()` | Required String | 1 × String |
| 148 | `error_msg.clone()` (rollback only) | Per-failed-task | 1 × String |

**Total per item (each): ~(M + 54) allocations**, where M = job_ctx.len()

## The Real Bottleneck

### `job_ctx.clone()` is the Critical Issue

```rust
// each.rs lines 107-118
let cx = {
    let mut m = job_ctx.clone();  // O(M) per item!
    m.insert(
        var_name.to_string(),
        serde_json::json!({
            "index": ix.to_string(),
            "value": item
        }),
    );
    m
};
```

For N list items with a context of M entries:
- **Current**: N × M string copies
- **Cannot be fixed with `Cow`** because we need to INSERT a new key per iteration
- **Cannot use `Arc-swap`** because the HashMap itself is mutated

### Why Cow/Arc-swap Won't Help Here

1. **Cow<'a, str>`**: Only helps when you BORROW more often than MUTATE. Here we MUST mutate (insert item/index).

2. **Arc-swap**: Would help for READ-heavy scenarios, but the issue is we're CREATING a new modified copy per item, not sharing a single mutable reference.

3. **The structural problem**: Each subtask needs a UNIQUE context with the `item` and `index` bound. This requires per-item allocation regardless of optimization strategy.

## Redundant Operations (Valid but Minor)

### `job_id.to_string().into()` vs `job_id.into()`

```rust
// Current (line 61 in parallel.rs, line 125 in each.rs)
job_id: Some(job_id.to_string().into()),

// Better
job_id: Some(job_id.into()),  // Uses From<&str> directly
```

This saves ONE intermediate String allocation but the newtype still stores a `String` internally. Marginal gain.

### `TASK_STATE_PENDING.to_string()` - Always Allocates

```rust
// Current
state: twerk_core::task::TASK_STATE_PENDING.to_string(),

// Alternative: Use a &'static str and parse at boundary
state: TaskState::from_static(twerk_core::task::TASK_STATE_PENDING),
```

This would save one heap allocation per task if TaskState could hold a `&'static str`.

## Benchmark Baseline

```
job_creation/1           time:   [~350 ns]
job_creation/5           time:   [~450 ns]
job_creation/10          time:   [~675 ns]
job_creation/50          time:   [~6.3 µs]

10x_stress_test/100      time:   [~28 ms]
10x_stress_test/1000     time:   [~24 ms]
10x_stress_test/10000    time:   [~53 ms]
```

The benchmark `job_creation` doesn't actually test the hot path I analyzed — it measures simple Job construction, NOT `schedule_parallel_task` or `schedule_each_task`. A proper benchmark for task creation would need to mock the datastore.

## Recommendation

**The optimization claim is INVALID.** The real issue is:

1. **Architectural**: The `spawn_each_tasks` function creates N independent HashMap clones for N items. This is a fundamental design issue, not a clone-count issue.

2. **If reduced allocations are needed**: Consider redesigning `evaluate_task` to use a borrowed context with a scope-based cleanup, or use a arena allocator for the per-item context.

3. **The `Cow`/`Arc-swap` patterns are inapplicable** to this hot path because:
   - `Cow` requires mostly-borrowed, rarely-mutated data
   - `Arc-swap` requires lock-free read-mostly scenarios
   - We have write-mostly (each item needs unique context)

## Files Analyzed

- `crates/twerk-app/src/engine/coordinator/scheduler/parallel.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/each.rs`
- `crates/twerk-core/src/eval/task.rs`
- `crates/twerk-core/src/id.rs`
- `crates/twerk-core/src/task.rs`
