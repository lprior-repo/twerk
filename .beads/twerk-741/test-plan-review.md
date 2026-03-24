bead_id: twerk-741
bead_title: Fix middleware gaps: context types, expr evaluation, webhook events
phase: 1.7
updated_at: 2026-03-24T12:00:00Z

# Test Plan Review: REJECTED

## VERDICT: REJECTED

---

## Executive Summary

The test plan has **11 LETHAL findings** across contract parity, trophy allocation, and assertion completeness. The plan identifies 24 behaviors but fails to cover 9 of the 11 public functions in the contract. Density ratio is 2.18x (target ≥5x). `evaluate_task_condition` has zero scenarios. Error variants `ContextCancelled` and `ContextDeadlineExceeded` are never asserted as exact types.

**This plan cannot be approved. Rewrite required.**

---

## Axis 1 — Contract Parity: FAIL

### LETHAL: 11 missing public function scenarios

| Function | Contract Signature | Status |
|----------|-------------------|--------|
| `Context::cancelled()` | `-> Arc<Context>` | **NO SCENARIO** |
| `Context::deadline_exceeded()` | `-> Arc<Context>` | **NO SCENARIO** |
| `is_cancelled(&self)` | `-> bool` | **NO SCENARIO** |
| `is_deadline_exceeded(&self)` | `-> bool` | **NO SCENARIO** |
| `evaluate_condition(...)` | `-> Result<bool, String>` | **NO STANDALONE SCENARIO** |
| `evaluate_task_condition(...)` | `-> Result<bool, String>` | **NO SCENARIO AT ALL** |
| `get_job(ctx, job_id, ds, cache)` | `-> Result<Job, TaskMiddlewareError>` | **NO SCENARIO** |
| `Datastore::get_job_by_id(...)` | trait method | **NO SCENARIO** |
| `Error::ContextCancelled` | exact variant | **NO SCENARIO ASSERTING EXACT VARIANT** |
| `Error::ContextDeadlineExceeded` | exact variant | **NO SCENARIO ASSERTING EXACT VARIANT** |

### LETHAL: `Context::new()` used in scenario but not in contract

- **Scenario 89 ("Context.get returns None when key does not exist"):** Uses `Context::new()` which does not exist in the contract
- **Contract specifies:** `Context::cancelled()`, `Context::deadline_exceeded()`, `Context::with_value()` constructors
- **Finding:** Test plan assumes a constructor that is not part of the contract

### Scenario count mismatch

- **Contract public functions:** 11
- **BDD scenarios in plan:** 26
- **Gap:** The plan has scenarios for internal functions (`evaluate_expr`, `sanitize_expr`, `transform_operators`, `valid_expr`) that are NOT in the contract's public API, while missing scenarios for 9 of 11 actual public functions

---

## Axis 2 — Assertion Sharpness: PASS (for scenarios that exist)

All existing "Then:" clauses use concrete values (`Some("value")`, `None`, `true`, `false`). No `is_ok()` or `is_err()` found. No `> 0` or `Some(_)` without inner value.

**However**, the missing scenarios above are LETHAL regardless of assertion quality.

---

## Axis 3 — Trophy Allocation: FAIL

### LETHAL: Density ratio 2.18x (target ≥5x)

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Public functions | 11 | — | — |
| Planned tests | 24 | — | — |
| **Ratio** | **2.18x** | **≥5x** | **FAIL** |

### LETHAL: `evaluate_condition` — no proptest invariant

The primary public entry point for expression evaluation in job webhooks has zero proptest invariants. Only internal functions (`evaluate_template`, `evaluate_expr`, `sanitize_expr`) have invariants.

### LETHAL: `evaluate_task_condition` — no proptest invariant, no fuzz target

This function is defined in the contract but has no scenarios, no invariants, and no fuzz targets.

### Planned allocation breakdown

- Unit: 8
- Integration: 14
- E2E: 2
- **Total: 24** (but need 55+ for 5x coverage of 11 functions)

---

## Axis 4 — Boundary Completeness: MAJOR

### Functions with explicit boundary gaps

**`Context::with_value`:**
- Empty key (should fail or handle) — not named
- Empty value (should fail or handle) — not named
- Key with special characters — not named
- One-above-maximum key length — not named

**`get_job(ctx, job_id, ds, cache)`:**
- Empty `job_id` — not named
- Non-existent `job_id` → `Err(JobNotFound(...))` — not named as exact error variant
- `ctx` is `Context::Cancelled` before call — not named
- `ctx` is `Context::DeadlineExceeded` before call — not named
- Cache miss scenario — not named
- Cache hit scenario — not named

**`evaluate_condition(expr, summary)`:**
- Empty expression → error — not named
- Invalid expression syntax → `Err(InvalidSyntax(...))` — not named
- Expression evaluates to non-boolean → `Err(NotBoolean(...))` — not named
- Missing `job_state` key in context — not named

**`should_fire_webhook`:** Truth table fully covered, no gaps found.

---

## Axis 5 — Mutation Survivability: MAJOR

### Survivors (would not be caught by plan)

| Mutation | Behavior | Required Test |
|----------|----------|---------------|
| `Context::get` uses `k == key` instead of `k.as_str() == key` | String comparison bug | `context_get_string_borrowing` — referenced in plan but has no scenario |
| `apply_middleware` uses wrong fold direction | Reversed execution order | No test independently verifies fold direction |
| `evaluate_condition` returns `Ok(true)` always | Broken condition evaluation | No standalone test of `evaluate_condition` with false expression |
| `evaluate_task_condition` — any mutation | Completely untested | `evaluate_task_condition_handles_false_expression` — doesn't exist |
| `evaluate_task_condition` — any mutation | Completely untested | `evaluate_task_condition_handles_context_cancelled` — doesn't exist |
| `get_job` never checks ctx cancellation | Context check silently dropped | `get_job_propagates_context_cancellation` — doesn't exist |

---

## Axis 6 — Holzmann Plan Audit: MINOR

- Rule 5 (State Your Assumptions): BDD Given/When/Then structure is present and explicit
- Rule 8 (Surface Your Side Effects): Middleware scenarios describe side effects in Given block
- **Finding:** `Context::new()` assumption mismatch between plan and contract

---

## LETHAL FINDINGS (must fix before resubmission)

1. **contract.md:44-48** — `Context::cancelled()` has no BDD scenario
2. **contract.md:44-48** — `Context::deadline_exceeded()` has no BDD scenario
3. **contract.md:61** — `is_cancelled(&self)` has no BDD scenario
4. **contract.md:62** — `is_deadline_exceeded(&self)` has no BDD scenario
5. **contract.md:288** — `evaluate_condition(expr, summary)` has no standalone scenario; only used as predicate in webhook tests
6. **contract.md:289** — `evaluate_task_condition(...)` has zero scenarios, zero invariants, zero fuzz coverage
7. **contract.md:269-274** — `get_job(ctx, job_id, ds, cache)` has zero scenarios
8. **contract.md:265-266** — `Datastore::get_job_by_id` trait method has zero scenarios
9. **contract.md:113-114** — `Error::ContextCancelled` variant never asserted as exact type in any scenario
10. **contract.md:115-116** — `Error::ContextDeadlineExceeded` variant never asserted as exact type in any scenario
11. **test-plan.md:90** — Scenario uses `Context::new()` which does not exist in contract

### Additional LETHAL (trophy density)

12. **test-plan.md** — Ratio 24 tests / 11 functions = 2.18x (target ≥5x)

---

## MAJOR FINDINGS (3)

1. **`evaluate_condition` proptest invariant missing** — Primary public API for job webhook condition evaluation has no property-based test coverage
2. **`evaluate_task_condition` completely untested** — Function defined in contract contract but absent from all test categories (unit/integration/proptest/fuzz/kani)
3. **Boundary gaps for `Context::with_value`** — Empty key/value, special characters, max length not explicitly named per function

---

## MINOR FINDINGS (2/5 threshold)

1. **`transform_operators` missing fuzz corpus for edge cases** — Single-quote strings, unicode operators not named
2. **`evaluate_headers` scenario missing** — Behavior 25 mentions it but no standalone BDD scenario exists

---

## MANDATE

Before resubmission, the following **must** exist:

### Required BDD Scenarios (9)

1. `context_cancelled_constructor_returns_arc_context` — Given: nothing, When: calling Context::cancelled(), Then: returns Arc<Context> where is_cancelled() == true
2. `context_deadline_exceeded_constructor_returns_arc_context` — Given: nothing, When: calling Context::deadline_exceeded(), Then: returns Arc<Context> where is_deadline_exceeded() == true
3. `context_is_cancelled_returns_true_when_cancelled` — Given: Context::cancelled(), When: calling is_cancelled(), Then: returns true
4. `context_is_cancelled_returns_false_when_not_cancelled` — Given: Context::with_value("key", "value"), When: calling is_cancelled(), Then: returns false
5. `context_is_deadline_exceeded_returns_true_when_deadline_exceeded` — Given: Context::deadline_exceeded(), When: calling is_deadline_exceeded(), Then: returns true
6. `evaluate_condition_false_expression_returns_ok_false` — Given: expression "false", When: calling evaluate_condition, Then: Returns Ok(false)
7. `evaluate_task_condition_handles_true_expression` — Given: expression "true" and task/job summaries, When: calling evaluate_task_condition, Then: Returns Ok(true)
8. `get_job_propagates_context_cancellation` — Given: Context::Cancelled() and valid job_id, When: calling get_job, Then: Returns Err(TaskMiddlewareError::ContextCancelled)
9. `datastore_get_job_by_id_returns_job_or_error` — Given: ctx with values and existing job_id, When: calling get_job_by_id, Then: Returns Ok(Job) or appropriate error variant

### Required Exact Error Variant Assertions

For each of `TaskMiddlewareError::ContextCancelled`, `TaskMiddlewareError::ContextDeadlineExceeded`, `TaskMiddlewareError::JobNotFound(String)`, `TaskMiddlewareError::Middleware(String)`, `TaskMiddlewareError::Datastore(String)` — a scenario must assert `Err(TaskMiddlewareError::ExactVariant { ... })`.

### Density Target

Minimum 55 tests (11 functions × 5x) to be considered for APPROVED status.

### Contract Correction

Remove all references to `Context::new()` from test plan. Replace with appropriate constructor calls that exist in the contract.

---

## Summary

| Axis | Status | Lethal Count |
|------|--------|-------------|
| Contract Parity | FAIL | 11 |
| Assertion Sharpness | PASS | 0 |
| Trophy Allocation | FAIL | 1 (density) + 2 (missing invariants) |
| Boundary Completeness | MAJOR | 0 (below threshold) |
| Mutation Survivability | MAJOR | 0 (below threshold) |
| Holzmann Rules | MINOR | 0 |

**Total LETHAL: 14**
**Total MAJOR: 5**
**Total MINOR: 2**

**STATUS: REJECTED**

The test plan is fundamentally incomplete. 9 of 11 public functions have no scenarios. The density ratio is 2.18x against a 5x target. `evaluate_task_condition` is defined in the contract but entirely absent from the test plan. Resubmit only after addressing all LETHAL findings above.
