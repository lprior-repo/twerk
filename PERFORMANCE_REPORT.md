# Performance Improvement Report

## 1. Observe / Baseline
Before applying the 6 performance improvements, the system metrics were observed as follows:
- **`standalone_e2e_test` execution time**: ~0.39s
- **`engine_new` bench**: ~850 ns
- **`job_creation/50` bench**: ~4.77 µs

## 2. Hypothesize
The 6 improvements found in the stash targeted specific bottlenecks to improve throughput and latency:
1. **Parallelization in Broker (`for_each_concurrent`)**: Replaces excessive `tokio::spawn` calls per handler, which should reduce context switching and memory overhead.
2. **Parallel Task Creation in Scheduler (`try_join_all`)**: Submits tasks concurrently rather than awaiting them sequentially, theoretically lowering latency for jobs with multiple tasks.
3. **Secondary Indexing in Datastore**: Adding a `DashMap` index by `job_id` replaces O(N) full collection scans with O(1) lookups, greatly improving read performance.
4. **Avoiding Busy-Wait Shutdown**: Replacing a 100ms `sleep` polling loop with a `tokio::sync::watch` channel should reduce idle CPU cycles and improve shutdown responsiveness.

Overall, we hypothesize that these changes will reduce latency in E2E scenarios and improve task scheduling efficiency. Note that maintaining secondary indices might add marginal overhead to the creation path.

## 3. Experiment
The 6 performance improvements were applied to the codebase via `git stash pop`.
We ran the following commands to gather new metrics:
- `time cargo test -p twerk-app --test standalone_e2e_test`
- `cargo bench -p twerk-app --bench engine -- --sample-size=100`

## 4. Analyze
The new metrics obtained from the experiment are:
- **`standalone_e2e_test` execution time**: 0.54s
- **`engine_new` bench**: ~873 ns
- **`job_creation/50` bench**: ~5.03 µs

**Conclusion**:
Contrary to our initial hypothesis, the implementation of these 6 improvements resulted in a slight regression across our targeted benchmarks:
- The E2E test execution time increased from ~0.39s to 0.54s.
- `engine_new` increased slightly from ~850 ns to ~873 ns.
- `job_creation/50` increased from ~4.77 µs to ~5.03 µs.

**Analysis of Results**:
- Maintaining a secondary index in the Datastore adds overhead to the task creation path (`job_creation/50`), as it requires an additional insert/update operation into a concurrent `DashMap`.
- Async parallelization primitives like `try_join_all` and `for_each_concurrent` can introduce runtime scheduling overhead compared to simple sequential loops or fire-and-forget `tokio::spawn` calls. For micro-benchmarks where tasks are extremely fast, this scheduling overhead outweighs the benefits of parallelization.
- The `standalone_e2e_test` regression likely reflects these accumulated overheads, indicating that for small scales, the naive implementations were faster, and the algorithmic improvements only pay off at much larger scales or higher levels of concurrency than what was tested.