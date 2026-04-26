# Test Plan: Generated Workload Contracts Surface (`crates/twerk-web/tests/generated_workload_contracts.rs`)

## Summary
- Behaviors identified: 14
- Trophy allocation: 8 unit / 5 integration / 0 e2e / 1 benchmark-only suite
- Proptest invariants: 5
- Fuzz targets: 1 reuse/expand existing parser fuzz corpus
- Kani harnesses: 2
- Mutation target: ≥90% kill rate for deterministic tests only

## 1. Behavior Inventory

1. `WorkloadScenario` returns the human-readable description for each scenario.
2. `WorkloadScenario` returns the expected complexity factor for each scenario.
3. `generate_workload` returns deterministic YAML for the same `(scenario, seed, iteration)`.
4. `generate_workload` varies job identity/content when iteration changes.
5. `generate_parallel_yaml` emits a parallel job with exactly `count` tasks.
6. `generate_each_yaml` emits an `each.items` list with exactly `item_count` items.
7. `generate_mutated_yaml` emits a single-task job whose chosen image/command is deterministic from inputs.
8. Every generated scenario fixture parses into a `Job` successfully.
9. Every generated scenario fixture preserves its scenario contract after parse (task count, `parallel`, `each`, env/volumes presence, etc.).
10. `GeneratedWorkloadStats::new` computes exact descriptive statistics for a known sample set.
11. `GeneratedWorkloadStats::new` keeps percentile and min/max values ordered and in-range for any non-empty sample set.
12. `coefficient_of_variation` returns the expected percentage for a known result.
13. `run_scenario` returns a result tagged with the requested scenario and exactly `RUNS_PER_SCENARIO` samples.
14. Performance reporting can observe throughput/latency trends, but those claims are benchmark evidence, not product-correctness assertions.

## 2. Test Seams and Mixed Categories in the Current File

### Currently mixed together
- **Generator correctness**: YAML string construction for 15 scenarios.
- **Parser correctness**: whether `from_slice::<Job>` accepts the generated YAML.
- **Statistical math correctness**: mean, variance, CI, percentiles, CV.
- **Harness/orchestration correctness**: warm-up, run count, sample count, scenario tagging.
- **Benchmark/perf claims**: throughput floors, latency ceilings, correlation claims.
- **Environmental variance**: wall-clock timing, machine load, package-cache lock contention, CI noise.

### Fowler-style split required

| Category | What to test | Layer | Notes |
|---|---|---|---|
| Generator characterization | Exact YAML shape and deterministic variation | unit | Pure, cheap, stable |
| Parser contract for generated workloads | Generated YAML parses into expected `Job` shape | integration | Black-box through `from_slice::<Job>` |
| Statistical math | Exact computed statistics from fixed sample vectors | unit | No wall-clock timing |
| Simulation harness contract | Returned sample count/scenario tag with injected fixed measurements | unit/characterization | Requires seam extraction or deterministic helper |
| Throughput/latency/correlation research | Trend reporting only | benchmark/ignored | Must not gate `cargo nextest run` |

## 3. Trophy Allocation and Named Tests

### Deterministic unit / characterization tests

#### Behavior: scenario metadata is stable
- **Test**: `fn workload_scenario_returns_expected_description_when_variant_is_known()`
- **Given** each `WorkloadScenario`
- **When** `description()` is called
- **Then** it equals the exact label expected by reports.

- **Test**: `fn workload_scenario_returns_expected_complexity_factor_when_variant_is_known()`
- **Given** each `WorkloadScenario`
- **When** `complexity_factor()` is called
- **Then** it equals the exact factor in the scenario table.

#### Behavior: workload generators are deterministic and scenario-specific
- **Test**: `fn generate_workload_returns_identical_yaml_when_seed_and_iteration_match()`
- **Given** the same scenario, seed, and iteration
- **When** `generate_workload` is called twice
- **Then** both YAML strings are exactly equal.

- **Test**: `fn generate_workload_changes_job_identity_when_iteration_changes()`
- **Given** the same scenario and seed with two iterations
- **When** `generate_workload` is called
- **Then** the YAML strings differ and job/task identifiers differ in observable fields.

- **Test**: `fn generate_parallel_yaml_returns_parallel_job_with_exact_task_count_when_count_is_8()`
- **Then** parsed/inspected YAML contains `parallel: true` and exactly 8 task entries.

- **Test**: `fn generate_each_yaml_returns_exact_item_count_when_item_count_is_25()`
- **Then** the emitted YAML contains exactly 25 `each.items` values and the task command references `{{ item }}`.

- **Test**: `fn generate_mutated_yaml_selects_expected_image_and_command_when_inputs_are_known()`
- **Given** fixed `job_id` and `task_id`
- **When** YAML is generated
- **Then** the exact image and command branch selected by modulo logic is present.

#### Behavior: statistical calculations are correct
- **Test**: `fn generated_workload_stats_return_exact_statistics_when_samples_are_known()`
- **Given** a fixed sample vector such as `[10.0, 20.0, 30.0, 40.0]`
- **When** `GeneratedWorkloadStats::new` is called
- **Then** mean, variance, std_dev, min, max, percentiles, and CI bounds equal the exact expected values (within explicit float tolerance).

- **Test**: `fn generated_workload_stats_return_zero_variance_and_zero_ci_width_when_all_samples_match()`
- **Given** a constant sample vector
- **Then** variance = `0.0`, std_dev = `0.0`, `ci_lower == ci_upper == mean`, and all percentiles equal the sample value.

- **Test**: `fn coefficient_of_variation_returns_expected_percentage_when_mean_and_std_dev_are_known()`
- **Given** a known `GeneratedWorkloadStats`
- **Then** CV equals the exact expected percentage.

#### Behavior: run harness contract is separate from performance evidence
- **Test**: `fn run_scenario_returns_requested_scenario_and_run_count_when_measurement_source_is_deterministic()`
- **Given** a deterministic timing/parser seam returning fixed sample durations
- **When** the scenario is run
- **Then** `result.scenario == requested_scenario` and `result.samples.len() == RUNS_PER_SCENARIO` with exact expected sample values.

- **Test**: `fn run_scenario_excludes_warmup_samples_from_reported_measurements_when_measurement_source_is_deterministic()`
- **Given** a deterministic seam that records parse invocations
- **Then** warm-up invocations do not appear in `result.samples`.

> Note: these two tests require extracting a deterministic seam from the current wall-clock implementation. Without that seam, `run_scenario` remains a benchmark harness, not a reliable unit-test subject.

### Contract / acceptance tests (black-box parser contract for generated workloads)

#### Behavior: every generated scenario fixture is valid input to the public parser
- **Test**: `fn simple_echo_workload_parses_into_single_echo_task_when_generated()`
- **Given** YAML from `generate_workload(SimpleEcho, 42, 7)`
- **When** `from_slice::<Job>` parses it
- **Then** the parsed job has `name == Some("echo-job-7")`, one task named `echo`, and command `['echo', '<message>']`.

- **Test**: `fn simple_file_write_workload_parses_with_expected_tmp_path_when_generated()`
- **Then** parsed command contains the same `/tmp/file_<n>` path in both write and read positions.

- **Test**: `fn simple_loop_workload_parses_with_iteration_dependent_bound_when_generated()`
- **Then** parsed command contains the expected loop upper bound.

- **Test**: `fn multi_task_workload_parses_with_four_tasks_when_generated()`
- **Then** parsed job contains exactly four tasks with the expected names.

- **Test**: `fn env_vars_workload_parses_with_expected_env_entries_when_generated()`
- **Then** parsed job contains the expected env key/value list.
- **Current risk**: this should expose the current malformed generator (`value: "..."[`), which is a characterization failure and must be fixed before any throughput claim uses this scenario.

- **Test**: `fn volumes_workload_parses_with_expected_mount_count_when_generated()`
- **Then** parsed task contains the exact number of generated volume mounts.

- **Test**: `fn parallel_8_tasks_workload_parses_as_parallel_job_when_generated()`
- **Then** parsed job is `parallel == true` with exactly 8 tasks.

- **Test**: `fn each_25_items_workload_parses_with_exact_item_cardinality_when_generated()`
- **Then** parsed job contains exactly 25 items in the `each` input shape expected by the public API.

- **Test**: `fn sustained_burst_workload_parses_with_iteration_dependent_task_count_when_generated()`
- **Then** parsed job is `parallel == true` and task count equals `8 + (iteration % 16)`.

- **Test**: `fn mixed_job_types_workload_parses_for_each_branch_when_iteration_selects_branch()`
- **Given** iterations that hit each modulo branch
- **Then** each generated fixture parses successfully into the expected family of job shapes.

- **Test**: `fn high_variability_workload_parses_for_each_branch_when_iteration_selects_branch()`
- **Given** iterations that hit all seven branches
- **Then** every branch parses successfully and preserves its expected shape.

### Benchmark / soak / research tests (non-gating)

These do **not** belong in default `cargo test` / `nextest` runs. Mark `#[ignore]`, move under a `benchmarks` module, or migrate to Criterion/custom runner.

- **Test/bench**: `simple_echo_parser_throughput_benchmark_reports_distribution_under_controlled_load`
  - Records throughput distribution for SimpleEcho.
  - Outputs mean/stddev/CI as diagnostics only.
  - No hard assertion like `ci_lower > 20_000.0` in unit-test lane.

- **Test/bench**: `parallel_8_tasks_parser_throughput_benchmark_reports_distribution_under_controlled_load`

- **Test/bench**: `sustained_burst_parser_throughput_benchmark_reports_distribution_under_controlled_load`

- **Test/bench**: `full_monte_carlo_report_generates_summary_across_all_scenarios`
  - Success criterion: benchmark run completes and emits report artifacts.
  - Optional regression policy: compare against checked-in baseline with tolerance bands only in dedicated perf CI.

- **Test/bench**: `complexity_to_throughput_correlation_report_is_published_for_analysis`
  - Research/report only.
  - No brittle assertion like `correlation < -0.5` in correctness CI.

## 4. Current Tests to Rewrite / Quarantine / Rename / Reclassify

| Current test | Action | Why |
|---|---|---|
| `run_full_generated_workload_contracts` | **Quarantine / reclassify as ignored benchmark** | Mixes product correctness with machine-sensitive throughput, latency, p-value, and report printing |
| `generated_workload_simple_echo_contract` | **Rewrite + split** | Replace with deterministic parse contract test; move current throughput threshold to benchmark lane |
| `generated_workload_parallel_8_tasks_contract` | **Rewrite + split** | Current assertion is benchmark evidence, not behavior contract |
| `generated_workload_sustained_burst_contract` | **Rewrite + split** | Same problem: ambient throughput threshold in unit-test lane |
| `correlation_complexity_vs_throughput` | **Quarantine or delete from correctness suite** | Correlation is a research claim highly sensitive to environment and sample noise |

### Recommended renamed correctness tests
- `simple_echo_workload_parses_into_single_echo_task_when_generated`
- `parallel_8_tasks_workload_parses_as_parallel_job_when_generated`
- `sustained_burst_workload_parses_with_iteration_dependent_task_count_when_generated`
- `env_vars_workload_parses_with_expected_env_entries_when_generated`
- `run_scenario_returns_requested_scenario_and_run_count_when_measurement_source_is_deterministic`

## 5. BDD Scenarios

### Behavior: generated workload parses successfully
Given: a generated YAML fixture for a specific scenario and fixed seed/iteration  
When: `from_slice::<Job>` parses the bytes  
Then: the returned `Job` contains the exact expected name and structural fields for that scenario  

Error variant:  
Given: a generator branch known to emit malformed YAML  
When: `from_slice::<Job>` parses the bytes  
Then: `Err(ApiError::BadRequest("YAML parse error..."))`  

> This is a temporary characterization test only if the malformed output is intentionally preserved during refactor; final state should remove this error case by fixing the generator.

### Behavior: GeneratedWorkloadStats computes statistics correctly
Given: a fixed vector of samples  
When: `GeneratedWorkloadStats::new` constructs a result  
Then: mean, variance, std_dev, CI, min/max, and percentiles equal exact expected values within stated tolerance  

### Behavior: run_scenario orchestrates runs
Given: a deterministic parser/timer seam with known durations  
When: `run_scenario` executes  
Then: the result contains exactly `RUNS_PER_SCENARIO` samples, tagged to the requested scenario, with expected derived statistics  

## 6. Proptest Invariants

1. **`generate_workload` determinism**  
   - Invariant: same `(scenario, seed, iteration)` always yields identical YAML.  
   - Strategy: arbitrary `u64` seed/iteration and enum variant.  

2. **`generate_parallel_yaml` cardinality**  
   - Invariant: for `count in 1..=64`, YAML contains exactly `count` task entries after parse.  

3. **`generate_each_yaml` cardinality/order**  
   - Invariant: parsed item list length equals `item_count`, first item is `item-000`, last is `item-(count-1)` zero-padded.  

4. **`GeneratedWorkloadStats::new` ordering**  
   - Invariant: for any non-empty finite sample vector, `min <= p50 <= p95 <= p99 <= max` and `ci_lower <= mean <= ci_upper`.  

5. **`GeneratedWorkloadStats::new` translation invariance**  
   - Invariant: adding constant `k` to all samples increases mean/min/max/percentiles/CI by `k` and leaves variance/std_dev unchanged.  

## 7. Fuzz Targets

### Reuse existing parser fuzz/property surface with generated-scenario corpus
- **Target**: `from_slice::<Job>`
- **Input type**: YAML bytes produced by generated workload builders plus hand-mutated neighbors
- **Risk**: generator accidentally emits malformed YAML that benchmark code silently ignores; parser panic/parse drift on large synthetic workloads
- **Corpus seeds**:
  - one fixture from each of the 15 scenarios
  - branch-covering fixtures for `MixedJobTypes` and `HighVariability`
  - boundary fixtures for `Parallel16Tasks`, `Each50Items`, and max-task `SustainedBurst`

## 8. Kani Harnesses

1. **Percentile index safety**  
   - Property: for any non-empty bounded sample length, `p50_idx`, `p95_idx`, and `p99_idx` are within `sorted.len()`.  
   - Bound: sample vectors length `1..=64`.  
   - Rationale: proves percentile indexing cannot go out of bounds.

2. **CI encloses mean for finite samples**  
   - Property: for any non-empty bounded vector of finite non-NaN samples, `ci_lower <= mean <= ci_upper`.  
   - Bound: sample vectors length `1..=32`, values in a bounded finite range.  
   - Rationale: catches sign/ordering mistakes in CI construction.

## 9. Mutation Testing Checkpoints

Minimum threshold: **≥90% kill rate** on deterministic tests in this module.

- Flipping `parallel: true` removal in `generate_parallel_yaml` must be caught by `parallel_8_tasks_workload_parses_as_parallel_job_when_generated`.
- Changing task loop bound from `0..count` to `0..count-1` must be caught by parallel and sustained-burst cardinality tests.
- Changing `item_count` handling in `generate_each_yaml` must be caught by exact-cardinality tests.
- Changing modulo branch selection in `generate_mutated_yaml` must be caught by exact image/command characterization test.
- Replacing mean formula or variance formula must be caught by `monte_carlo_result_returns_exact_statistics_when_samples_are_known`.
- Swapping percentile indices or CI signs must be caught by statistics exact-value and ordering property tests.
- Returning wrong scenario or wrong sample count from `run_scenario` seam must be caught by harness-contract tests.

## 10. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| generator determinism | same seed/iteration | exact same YAML string | unit |
| generator variation | different iteration | different YAML identity fields | unit |
| parallel cardinality | count = 1, 8, 16 | exact parsed task count | unit/integration |
| each cardinality | item_count = 1, 10, 25, 50 | exact parsed item count | unit/integration |
| mixed branches | iteration % 3 = 0/1/2 | each branch parses into expected job family | integration |
| high-variability branches | iteration % 7 = 0..6 | each branch parses successfully | integration |
| env generator | fixed iteration | exact env entries parse successfully | integration |
| stats happy path | known samples | exact statistics | unit |
| stats zero variance | constant samples | zero variance / identical percentiles | unit |
| stats invariant | arbitrary finite non-empty samples | ordered percentiles, CI encloses mean | proptest |
| harness contract | deterministic measurement seam | exact sample count and scenario tag | characterization |
| perf report | real wall-clock machine | report emitted, no brittle threshold in correctness suite | benchmark |

## Open Questions

1. Should `run_scenario` be refactored behind a deterministic measurement seam, or should correctness coverage stop at generators + `GeneratedWorkloadStats` and leave `run_scenario` entirely in benchmark land?
2. Do we want a separate `cargo bench`/Criterion path, or simply `#[ignore]` perf tests with explicit manual invocation?
3. Is the malformed `EnvVars` generator a known bug, or was the stray `[` introduced accidentally? The acceptance test should answer this immediately.
