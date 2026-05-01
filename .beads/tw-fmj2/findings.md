# Findings: tw-fmj2 - PriorityQueue::push/pop Tests

## Summary
Bead requested writing tests for PriorityQueue in `crates/twerk-scheduler/src/queue.rs`. Upon inspection, **all required tests were already implemented**.

## Tests Found (all passing)

### 1. `test_push_pop_order_by_priority` (line 57-66)
- Push tasks with priority 3, 1, 2
- Verifies pop returns 1 first, then 2, then 3
- **Covers**: Bead points 1, 2, 3

### 2. `test_pop_on_empty_returns_none` (line 69-72)
- Creates empty PriorityQueue, verifies pop returns None
- **Covers**: Bead point 4

### 3. `test_fifo_within_same_priority` (line 75-84)
- Pushes 3 tasks with same priority (1)
- Verifies FIFO ordering within same priority: first, second, third
- **Covers**: Bead points 5 and 6

## Verification
```
cargo test -p twerk-scheduler -- queue --nocapture
```
Result: **3 passed** ✓

## Conclusion
No code changes required. All requested test cases already exist and pass.
