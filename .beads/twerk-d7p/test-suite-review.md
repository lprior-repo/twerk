# Test Suite Review — twerk-d7p (FINAL)

**Date**: 2026-04-13
**Mode**: 2 — Suite Inquisition
**Scope**: twerk-core library tests (lib + integration)
**Pre-existing blockers**: `trigger/tests.rs` (120 compile errors), `red_queen_trigger_error.rs` (variant arity mismatches)

---

## VERDICT: **REJECTED**

Multiple LETHAL findings block this suite. The pre-existing errors in `trigger/tests.rs` prevent:
1. `cargo test --lib` (unit tests in lib)
2. Mutation analysis (Tier 3)
3. Full coverage measurement

---

## Tier 0 — Static Analysis

### [FAIL] Banned Assertions — 77 instances of weak assertions

**`assert!(result.is_ok())` / `assert!(result.is_err())` without concrete value checks:**

| File | Lines | Count |
|------|-------|-------|
| `tests/red_queen_trigger_error.rs` | 680 | 1 |
| `tests/red_queen_gen3.rs` | 333, 350 | 2 |
| `tests/red_queen_gen2.rs` | 70,76,82,84,86,88,285,291,320,346,428,433,438 | 13 |
| `tests/red_queen_adversarial.rs` | 19,25,31,50,345,356,367,374,386,458,464,531,538,911 | 14 |
| `tests/asl_adversarial_test.rs` | 262,719,808,828,840,853,859,865,871,877,883,889,895,1193,1201,1240,1325,1394,1402,1410,1417,1428,1437,1457,1464,1481,1496,1536,1578,1585,1601,1608,1615,1656,1663,1672,1697,1704,1716,1726,1733,1742,1776,1805 | 44 |
| `src/id.rs` | 971, 975 (prop_assert - acceptable) | 0 (proptest) |
| `tests/asl_types_test.rs` | (uses prop_assert - acceptable) | 0 (proptest) |

**LETHAL**: Every `assert!(x.is_ok())` or `assert!(x.is_err())` that doesn't verify the concrete inner value is a hollow test. If the implementation changes the error type or returns a different variant, the test continues passing while the bug ships.

### [FAIL] Silent Error Discard — 43 instances

**`let _ = result` or `.ok()` in test context:**

| File | Line | Context |
|------|------|---------|
| `src/trigger/tests.rs` | 1546 | `let _ = is_valid_transition(*from, *to, *variant);` |
| `tests/red_queen_trigger_error.rs` | 20,23,26,29,32,35,38,41,44,47,50,58-68,572,630,633,636,639,642,645,648,651,654,657,660 | Error construction for Debug tests |
| `tests/red_queen_gen3.rs` | 525,527,529,531 | `let _ = err.to_string()` |
| `tests/red_queen_adversarial.rs` | 880 | `let _ = err.to_string()` |
| `src/eval/intrinsics.rs` | 188 | `let _ = write!(acc, "{b:02x}")` — implementation, not test |
| `src/fns.rs` | 17,37 | Implementation file writes |

**LETHAL** (test context): `trigger/tests.rs:1546` is in a test helper and silently discards the result.

### [PASS] `#[ignore]` Tests — None found

### [PASS] Sleep in Tests — None in test files
The `tokio::time::sleep` references in `webhook.rs:218,219` are in implementation, not tests.

### [FAIL] Test Naming — 134 instances of `fn test_`

Standard naming convention, but many test names are generic (e.g., `test_validate_cron_valid`, `test_validate_duration_valid`). These are MINOR — the names describe what they test but don't follow Fowler-style naming that describes the behavior being proven.

### [FAIL] Holzmann Rule 2 — 55 Loops in Test Bodies

| File | Line | Loop | Severity |
|------|------|------|----------|
| `tests/red_queen_trigger_error.rs` | 112,444,445,503,597 | Nested state transition iteration | LETHAL |
| `tests/red_queen_gen3.rs` | 80,187,194,205,217,233,250,269,270,322,360,374,425,436,447,476 | Fuzzing loops (0..1000, 0..10000) | LETHAL |
| `tests/red_queen_gen2.rs` | 60,140,158,310,336,475,489,493,562,614 | Pattern/state iteration | LETHAL |
| `tests/red_queen_adversarial.rs` | 112,228,257,268,318,331,392,469,481,504,718,737,751,767,782 | Case iteration | LETHAL |
| `tests/asl_validation_test.rs` | 58 | Iteration in setup helper | MINOR |
| `tests/asl_types_test.rs` | 1125,1304,1374,1402 | Serde roundtrip loops | LETHAL |
| `tests/asl_transition_test.rs` | 233 | JitterStrategy iteration | LETHAL |
| `tests/asl_functions_test.rs` | 104 | `for _ in 0..20` fuzz | LETHAL |
| `tests/validation_test.rs` | 78,93 | Boundary iteration | LETHAL |

**LETHAL**: A test with a loop is a program with hidden logic. The `for i in 0..1000` loops in red_queen tests are particularly dangerous — they appear to be fuzzing but hide the iteration count in the test body rather than using proptest.

### [PASS] Mock Interrogation — Clean
No mockall usage found in twerk-core.

### [PASS] Integration Test Purity — Clean
No `use crate::` in `/tests/` directories.

### [FAIL] Error Variant Completeness — Many untested variants

**TriggerError (trigger/types.rs) variants with no exact-variant tests:**
- `TriggerError::PayloadTooLarge(usize)` — NOT TESTED
- `TriggerError::UnsupportedContentType(String)` — NOT TESTED
- `TriggerError::AuthenticationFailed(String)` — NOT TESTED
- `TriggerError::PollingHttpError(String)` — NOT TESTED
- `TriggerError::PollingExpressionError(String)` — NOT TESTED
- `TriggerError::MaxConsecutiveFailures(usize)` — NOT TESTED
- `TriggerError::JobCreationFailed(String)` — NOT TESTED
- `TriggerError::JobPublishFailed(String)` — NOT TESTED
- `TriggerError::ConcurrencyLimitReached` — NOT TESTED
- `TriggerError::JobIdGenerationFailed(String)` — NOT TESTED

**LETHAL**: Only `NotFound`, `AlreadyExists`, `InvalidStateTransition`, `DatastoreUnavailable`, `BrokerUnavailable`, `TriggerInErrorState`, `TriggerDisabled`, `InvalidConfiguration`, `InvalidTimezone` have tests (in `red_queen_trigger_error.rs`).

### [PASS] Density Audit
- **Public functions**: 222
- **Total tests** (#[test] + #[rstest]): 1280
- **Integration tests** (tests/ directory): 969
- **Ratio**: 1280 / 222 = **5.76×** ✅ (target ≥5×)

---

## Tier 1 — Compilation + Execution

### [PASS] Clippy — 0 warnings
```
cargo clippy -p twerk-core --lib
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.14s
```

### [FAIL] Tests — 444 pass, 27 fail

**Passing tests (runnable):**
| Test File | Passed | Status |
|-----------|--------|--------|
| `asl_types_test` | 107 | ✅ |
| `asl_transition_test` | 50 | ✅ |
| `asl_states_test` | 53 | ✅ |
| `asl_container_test` | 36 | ✅ |
| `asl_machine_test` | 35 | ✅ |
| `asl_validation_test` | 16 | ✅ |
| `asl_data_flow_test` | 25 | ✅ |
| `asl_functions_test` | 38 | ✅ |
| `asl_adversarial_test` | 183 | ✅ |
| `validation_test` | 59 | ✅ |
| `webhook_test` | 25 | ✅ |
| **Subtotal** | **627** | ✅ |

**Failing tests:**
| Test File | Passed | Failed |
|-----------|--------|--------|
| `eval_test` | 58 | 24 |
| `red_queen_adversarial` | 91 | 3 |
| `red_queen_gen2` | (blocked by trigger errors) | - |
| `red_queen_gen3` | (blocked by trigger errors) | - |
| `trigger_registry_test` | (blocked by trigger errors) | - |
| **Total runnable** | **444** | **27** |

**eval_test failures** — All 24 failures are `InvalidUuid` panics at `eval_test.rs:28`:
```
thread 'test_task_condition_non_boolean' (2703154) panicked at crates/twerk-core/tests/eval_test.rs:28:44:
called `Result::unwrap()` on an `Err` value: InvalidUuid
```
**Root cause**: Test setup creates a JobId/TaskId from a hardcoded UUID string that fails validation.

### [N/A] Ordering Probe
Cannot execute due to pre-existing compile errors in lib test compilation.

### [N/A] Insta Staleness
Insta not present in this crate.

---

## Tier 2 — Coverage

### [N/A] Line Coverage — Cannot measure
`cargo llvm-cov` not executed (requires separate installation). The pre-existing compilation errors in `trigger/tests.rs` would also block llvm-cov from measuring the lib.

### [MANUAL] Coverage Analysis (Code Inspection)

**ASL module** (covered by 627 tests):
- `StateName`, `Expression`, `JsonPath`, `VariableName`, `ImageRef`, `ShellScript`, `BackoffRate`, `ErrorCode`, `Transition`, `Retrier`, `Catcher`, `ChoiceState`, `WaitDuration`, `TaskState`, `ParallelState`, `MapState`, `StateMachine` — all have comprehensive tests with exact value assertions.

**Domain types** (minimal coverage):
- `domain_types.rs` — QueueName, CronExpression, GoDuration, Priority, RetryLimit — SERIALIZATION ONLY tested (round-trip), NOT validation/error paths.

**Critical gaps in domain_types:**
| Type | Missing Coverage |
|------|-----------------|
| `QueueName` | Invalid characters, empty string, too long |
| `CronExpression` | Invalid expressions, invalid timezones |
| `GoDuration` | Invalid format, negative values |
| `Priority` | Out-of-range values |
| `RetryLimit` | Out-of-range values |

---

## Tier 3 — Mutation

### [N/A] Kill Rate — Cannot measure
```
cargo mutants -p twerk-core
error: could not compile `twerk-core` (lib test) due to 120 previous errors
```
**Baseline build failure** prevents mutation analysis.

---

## LETHAL FINDINGS

1. **77 bare `is_ok()`/`is_err()` assertions** — Tests prove nothing about the actual value
   - Location: Multiple test files
   - Each proves only "some result exists" not "the correct result"

2. **55 loops in test bodies** (Holzmann Rule 2) — Hidden program logic
   - `for i in 0..1000` fuzz loops should use proptest
   - `for state in [...]` iteration should use `#[rstest]`

3. **10+ untested TriggerError variants** — Missing error path coverage
   - `PayloadTooLarge`, `UnsupportedContentType`, `AuthenticationFailed`, `PollingHttpError`, `PollingExpressionError`, `MaxConsecutiveFailures`, `JobCreationFailed`, `JobPublishFailed`, `ConcurrencyLimitReached`, `JobIdGenerationFailed`

4. **27 test failures** in `eval_test` and `red_queen_adversarial`
   - `eval_test.rs:28` — InvalidUuid panic in test setup

5. **Pre-existing trigger/tests.rs compilation errors** (120 errors)
   - Blocks: `cargo test --lib`, mutation analysis, full coverage measurement
   - Root cause: `TriggerId` constructor is private but tests try to construct it directly

---

## MAJOR FINDINGS (3)

1. **Domain type validation untested** — All 6 domain types (QueueName, CronExpression, GoDuration, Priority, RetryLimit) only have serialization round-trip tests. No error path validation.

2. **eval_test setup uses invalid UUIDs** — 24 tests fail because `JobId::new("task-001")` returns `Err(InvalidUuid)` instead of being handled gracefully.

3. **red_queen_adversarial 3 failures** — `rq_error_ts_unknown_message_mentions_input`, `rq_serde_ts_accepts_valid_json`, `rq_ts_case_insensitive_mixed` fail.

---

## MINOR FINDINGS (0/5 threshold)

1. **134 generic test names** — `fn test_validate_cron_valid` style names don't describe the behavior proven.

---

## MANDATE

The following MUST be fixed before resubmission:

1. **Fix pre-existing trigger/tests.rs compilation errors**
   - `TriggerId` inner field is private but tests try to construct `TriggerId("...")`
   - This blocks ALL unit tests (`cargo test --lib`) and mutation analysis

2. **Fix eval_test InvalidUuid failures** (24 tests)
   - `eval_test.rs:28` — change hardcoded UUID strings to valid ones

3. **Fix red_queen_adversarial failures** (3 tests)
   - `rq_error_ts_unknown_message_mentions_input`
   - `rq_serde_ts_accepts_valid_json`
   - `rq_ts_case_insensitive_mixed`

4. **Replace 77 bare `is_ok()`/`is_err()` assertions** with concrete value checks
   - Every assertion must verify the exact inner value or variant

5. **Eliminate 55 loops in test bodies**
   - Convert fuzz loops to proptest with `prop_assert!`
   - Convert iteration to `#[rstest]` parameterized tests

6. **Add tests for 10+ untested TriggerError variants**
   - `PayloadTooLarge`, `UnsupportedContentType`, `AuthenticationFailed`, `PollingHttpError`, `PollingExpressionError`, `MaxConsecutiveFailures`, `JobCreationFailed`, `JobPublishFailed`, `ConcurrencyLimitReached`, `JobIdGenerationFailed`

7. **Add domain type validation tests**
   - QueueName: invalid characters, empty, too long
   - CronExpression: invalid expressions, invalid timezones
   - GoDuration: invalid format, negative values
   - Priority: out-of-range values
   - RetryLimit: out-of-range values

8. **Re-run Tier 3 mutation analysis** after fixing build errors
   - Achieve ≥90% kill rate

---

**STATUS: REJECTED**

The suite has 5 LETHAL categories and 3 MAJOR findings. The pre-existing compilation errors in `trigger/tests.rs` are the root cause blocking full test execution, mutation analysis, and coverage measurement. These must be resolved before the suite can be approved.
