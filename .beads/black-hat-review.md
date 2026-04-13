# BLACK HAT CODE REVIEW — ASL Module

**Reviewer**: Black Hat Reviewer (STATE 5.5)
**Date**: 2025-07-15
**Scope**: `crates/twerk-core/src/asl/` (16 files) + `crates/twerk-core/src/eval/{intrinsics,data_flow}.rs`
**Contracts**: twerk-fq8, twerk-9xv, twerk-snj, twerk-14f, twerk-bzz

---

## STATUS: REJECTED

**Reason**: 3 CRITICAL defects, 7 MAJOR defects. The ASL type system is strong but has real contract violations, panic vectors in production code, and DDD breaches that make illegal states representable.

---

## PHASE 1: Contract & Bead Parity

### CRITICAL-1: `StateName` does not enforce "non-empty after trimming" (types.rs:69-77)
**Contract** (twerk-fq8, line 40): `Constraint: 1-256 UTF-8 characters, non-empty after trimming`
**Code**: `StateName::new()` checks `s.is_empty()` but does NOT trim. A name of `"   "` (all whitespace) passes validation.
**Severity**: CRITICAL — contract precondition has zero code enforcement.
**Fix**: Add `let s = s.trim().to_owned();` before the empty check, or reject whitespace-only strings explicitly.

### CRITICAL-2: `expect()` in production code — `machine.rs:146`
```rust
pub fn start_state(&self) -> &State {
    self.states.get(&self.start_at)
        .expect("start_state called on invalid machine: start_at not in states")
```
**Contract** (twerk-bzz, line 367): Acknowledges this panics if precondition violated.
**Severity**: CRITICAL — **No production code may contain `expect()`**. This is the Big 6 rule #1. The contract *allowing* a panic does not excuse it. The method should return `Option<&State>` or the signature should use a validated `ValidatedStateMachine` newtype that enforces SM-1 at the type level.
**Fix**: Return `Option<&State>` and remove `expect()`. Callers use `?` or match.

### CRITICAL-3: Three `expect()` calls in production code — `data_flow.rs:147,150,154`
```rust
.expect("checked above")   // line 147
.expect("checked above")   // line 150
.expect("just inserted")   // line 154
```
**Severity**: CRITICAL — Three panics in `set_at_path`. Even with "checked above" comments, this is banned in production code. If the invariant ever breaks (e.g., concurrent modification, future refactor), the process crashes.
**Fix**: Replace all three with `.ok_or_else(|| DataFlowError::NotAnObject { .. })?` or use `let`/`else` patterns.

### MAJOR-1: `unwrap_or` on path parsing — `data_flow.rs:42,47`
```rust
let rest = path.strip_prefix('$').unwrap_or(path);   // line 42
let rest = rest.strip_prefix('.').unwrap_or(rest);    // line 47
```
**Severity**: MAJOR — While `unwrap_or` doesn't panic, `parse_segments` silently accepts paths without `$` prefix. `JsonPath::new()` validates the `$` prefix, but `parse_segments` is a standalone function — if called with a raw string (bypassing `JsonPath`), it silently succeeds. This is a defense-in-depth gap.
**Fix**: Either return `DataFlowError::InvalidPath` when `$` is missing, or change the function signature to accept `&JsonPath` instead of `&str` (parse-don't-validate at the boundary).

### MAJOR-2: `FailState.error` and `FailState.cause` use `String`, not `ErrorCode` — `terminal.rs:64-69`
**Contract** (twerk-snj): FailState error and cause are "optional free-form strings" (INV-FS1).
**Assessment**: Contract explicitly allows `String`. However, `FailState.error` represents an error identifier. Using raw `String` when `ErrorCode` exists is a DDD concern. Not a contract violation per se, but a design gap.
**Severity**: MAJOR (DDD/Phase 4) — Cross-referenced below.

---

## PHASE 2: Farley Engineering Rigor

### MAJOR-3: Functions over 25 lines

| Function | File | Lines | Count |
|----------|------|-------|-------|
| `StateMachine::validate` | machine.rs:61-130 | 70 | ❌ |
| `WaitState::deserialize` | wait.rs:212-262 | 51 | ❌ |
| `TaskState::new` | task_state.rs:59-106 | 48 | ❌ |
| `set_at_path` | data_flow.rs:122-166 | 45 | ❌ |
| `format_fn` | intrinsics.rs:40-76 | 37 | ❌ |
| `dfs_visit` | validation.rs:116-152 | 37 | ❌ |
| `parse_segments` | data_flow.rs:40-75 | 36 | ❌ |
| `resolve_path` | data_flow.rs:81-116 | 36 | ❌ |
| `Transition::visit_map` | transition.rs:114-147 | 34 | ❌ |

**Severity**: MAJOR (aggregate) — 9 functions exceed the 25-line Farley hard constraint. The worst offender is `validate()` at 70 lines.
**Fix**: Extract sub-functions. E.g., `validate()` should decompose into `check_start_at()`, `check_transitions()`, `check_choices()`, `check_terminal()`.

### MAJOR-4: Functions with > 5 parameters

| Function | File | Params |
|----------|------|--------|
| `TaskState::new` | task_state.rs:59-69 | 9 |
| `MapState::new` | map.rs:65-73 | 7 |
| `Retrier::new` | retrier.rs:69-76 | 6 |
| `apply_data_flow` | data_flow.rs:207-213 | 5 (borderline) |

**Severity**: MAJOR — `TaskState::new` has 9 parameters. The contract mandates this struct shape, but the constructor should use a builder pattern or a `TaskStateConfig` parameter struct.
**Note**: `#[allow(clippy::too_many_arguments)]` at task_state.rs:58 and map.rs:64 suppresses the lint rather than addressing the design issue.

---

## PHASE 3: Functional Rust — Big 6

### Panic Vector Summary

| Location | Call | Context |
|----------|------|---------|
| machine.rs:146 | `expect()` | Production — `start_state()` |
| data_flow.rs:147 | `expect("checked above")` | Production — `set_at_path` |
| data_flow.rs:150 | `expect("checked above")` | Production — `set_at_path` |
| data_flow.rs:154 | `expect("just inserted")` | Production — `set_at_path` |
| transition.rs:159 | `unwrap()` | Test code — OK |

**Production panic count**: 4 (CRITICAL — must be zero)

### Mutable State Analysis

24 `let mut` declarations found. Most are justified by algorithm requirements (BFS queues, DFS coloring, serializer maps, string builders). The following are notable:

- **intrinsics.rs:55-57**: `let mut result`, `let mut arg_idx`, `let mut chars` in `format_fn` — imperative loop building a string with index tracking. Could be refactored with `fold` but acceptable for clarity.
- **intrinsics.rs:263-264**: `let mut result`, `let mut current` in `array_range_fn` — imperative range generation. Could use `std::iter::successors`.
- **intrinsics.rs:291**: `let mut seen` in `array_unique_fn` — O(n²) dedup with linear scan. Should use `IndexSet` or maintain insertion order differently.
- **validation.rs** (6 muts): All justified for BFS/DFS graph algorithms.
- **data_flow.rs:132-133**: `let mut result`, `let mut cursor` — mutable tree walk. Justified for in-place JSON mutation.

**Severity**: MINOR (aggregate) — The muts are mostly justified. `array_unique_fn` is the worst; it's O(n²) and uses mutable state when `IndexSet` exists.

### For Loops That Could Be Iterators

| Location | Loop | Could be |
|----------|------|----------|
| intrinsics.rs:292 | `for item in &items` (array_unique_fn) | `IndexSet` insertion |
| data_flow.rs:53 | `for token in rest.split('.')` | `map`/`try_fold` pipeline |
| types.rs:189 | `for c in chars` (VariableName validation) | `chars.all()` with early return |

**Severity**: MINOR — Not blocking. The VariableName loop at types.rs:189 is fine as-is (needs early return on invalid char).

---

## PHASE 4: DDD / Simplicity (Scott Wlaschin)

### MAJOR-5: `State` and `StateMachine` have fully public fields — `state.rs:69-81`, `machine.rs:48-55`

```rust
pub struct State {
    pub comment: Option<String>,      // line 69
    pub input_path: Option<JsonPath>,  // line 72
    pub output_path: Option<JsonPath>, // line 75
    pub assign: Option<HashMap<VariableName, Expression>>, // line 78
    pub kind: StateKind,               // line 81
}

pub struct StateMachine {
    pub comment: Option<String>,              // line 48
    pub start_at: StateName,                  // line 50
    pub states: IndexMap<StateName, State>,    // line 52
    pub timeout: Option<u64>,                 // line 55
}
```

**Severity**: MAJOR — **Illegal states are representable**. Anyone can write:
```rust
machine.start_at = StateName::new("nonexistent").unwrap();
machine.states.clear();
```
…and the validated invariants SM-1 through SM-6 are silently broken. Every other type in this module correctly uses private fields + accessor methods. `State` and `StateMachine` break the pattern catastrophically.

**Fix**: Make all fields `pub(crate)` or private. Add accessor methods. For `StateMachine`, add a validated constructor. The `start_state()` method's `expect()` (CRITICAL-2) exists precisely because the type allows construction of invalid instances.

### MAJOR-6: `StateMachine.timeout` uses raw `u64` — `machine.rs:55`
**Severity**: MAJOR — `TaskState.timeout` validates `>= 1` via the constructor, but `StateMachine.timeout` has no validation at all. Zero is accepted. Negative is impossible (u64), but zero-second timeout is likely a bug. There is no `Timeout` newtype, and the contract (twerk-bzz) does not specify a constraint on `StateMachine.timeout`, but this is inconsistent with `TaskState`'s timeout validation.
**Fix**: Either validate in `StateMachine::validate()` or create a `TimeoutSeconds` newtype.

### MINOR-1: `FailState.error` is `Option<String>` not `Option<ErrorCode>` — `terminal.rs:64-69`
The ASL error namespace has well-known codes (`ErrorCode` enum). `FailState.error` uses raw `String`, allowing arbitrary values that don't match the error taxonomy. The contract explicitly allows this, but it's a domain modeling weakness.
**Severity**: MINOR — Contract-compliant but not tight DDD.

### MINOR-2: `WaitDuration::Timestamp` wraps raw `String` — `wait.rs:40`
Contract (twerk-snj, INV-WD2): "Inner string is a non-empty ISO 8601 timestamp."
Code: No ISO 8601 validation. Only checks non-empty (wait.rs:127-130).
**Severity**: MINOR — The contract acknowledges this is a partial validation. Full ISO 8601 parsing is out of scope, but the invariant is technically unmet.

---

## PHASE 5: Bitter Truth (Velocity & Legibility)

### File Size Compliance

All 18 files are under 300 lines. ✅

| File | Lines | Status |
|------|-------|--------|
| types.rs | 331 | ⚠️ **OVER 300** |
| intrinsics.rs | 298 | ✅ |
| wait.rs | 262 | ✅ |
| task_state.rs | 224 | ✅ |
| data_flow.rs | 217 | ✅ |
| validation.rs | 199 | ✅ |
| retrier.rs | 185 | ✅ |
| map.rs | 182 | ✅ |
| transition.rs | 168 | ✅ |
| machine.rs | 148 | ✅ |
| choice.rs | 126 | ✅ |
| catcher.rs | 117 | ✅ |
| parallel.rs | 108 | ✅ |
| terminal.rs | 97 | ✅ |
| error_code.rs | 86 | ✅ |
| state.rs | 82 | ✅ |
| pass.rs | 37 | ✅ |
| mod.rs | 34 | ✅ |

### MINOR-3: `types.rs` is 331 lines — over the 300-line ceiling
**Severity**: MINOR — 7 newtypes + a shared macro in one file. Split into `string_types.rs` (StateName, Expression, JsonPath, VariableName, ImageRef, ShellScript) and `numeric_types.rs` (BackoffRate), or extract the macro to a shared module.

### Dead Code / YAGNI Assessment

No dead code detected. No over-abstractions. No traits with single implementers. The `str_newtype_impls!` macro is used 6 times — justified. The `validation.rs` module provides genuine graph analysis (reachability, cycles, dead-ends) that goes beyond the basic SM-1 through SM-6 checks — this is useful, not YAGNI.

### Legibility Assessment

Code is clean, boring, and well-documented. Module-level doc comments explain purpose. `#[must_use]` on all accessors. Consistent patterns across all files (Raw helper for serde, TryFrom, private fields + accessors). The code reads like it was written by someone who knows what they're doing and isn't trying to impress anyone. **This is the one area where the module shines.**

---

## DEFECTS SUMMARY

| ID | Severity | File:Line | Description | Fix |
|----|----------|-----------|-------------|-----|
| CRITICAL-1 | CRITICAL | types.rs:69-77 | StateName accepts whitespace-only strings; contract requires "non-empty after trimming" | Add trim or reject whitespace-only |
| CRITICAL-2 | CRITICAL | machine.rs:146 | `expect()` in production `start_state()` | Return `Option<&State>` |
| CRITICAL-3 | CRITICAL | data_flow.rs:147,150,154 | Three `expect()` calls in `set_at_path` | Replace with `ok_or_else(...)? ` |
| MAJOR-1 | MAJOR | data_flow.rs:42,47 | `parse_segments` silently accepts paths without `$` prefix | Accept `&JsonPath` or validate prefix |
| MAJOR-2 | MAJOR | terminal.rs:64-69 | `FailState.error` is `String` not `ErrorCode` | Use `ErrorCode` or document explicitly |
| MAJOR-3 | MAJOR | 9 functions | 9 functions exceed 25-line limit (worst: validate at 70 lines) | Extract sub-functions |
| MAJOR-4 | MAJOR | task_state.rs:59, map.rs:65, retrier.rs:69 | Functions with 6-9 parameters | Use builder/config struct |
| MAJOR-5 | MAJOR | state.rs:69-81, machine.rs:48-55 | Public fields on State and StateMachine allow invariant bypass | Make fields private, add accessors |
| MAJOR-6 | MAJOR | machine.rs:55 | `StateMachine.timeout` is unvalidated raw `u64` | Add validation or newtype |
| MINOR-1 | MINOR | terminal.rs:64-69 | FailState.error as String not ErrorCode (DDD gap) | Consider tighter typing |
| MINOR-2 | MINOR | wait.rs:40 | WaitDuration::Timestamp not validated as ISO 8601 | Add format validation |
| MINOR-3 | MINOR | types.rs | 331 lines exceeds 300-line ceiling | Split file |

---

## VERDICT

**REJECTED** — 3 CRITICAL, 7 MAJOR, 3 MINOR defects.

The ASL module demonstrates excellent structural discipline: parse-don't-validate newtypes, sum types over god objects, custom serde with validation, private fields with accessors (mostly). The code is clean and boring — highest praise.

But the panic vectors (4 `expect()` calls in production code) are an immediate disqualifier. The public fields on `State` and `StateMachine` are a DDD breach that makes the entire validation system toothless — anyone can mutate the fields and bypass all SM-1 through SM-6 checks. The `StateName` trimming gap is a direct contract violation.

### Required Fixes Before Re-Review

1. **Kill all 4 `expect()` calls in production code** (CRITICAL-1, CRITICAL-2, CRITICAL-3)
2. **Add whitespace trimming/rejection to `StateName::new()`** (CRITICAL-1)
3. **Make `State` and `StateMachine` fields private** (MAJOR-5)
4. **Extract `StateMachine::validate()` into sub-functions** (MAJOR-3 — at minimum the 70-line validate)
5. **Accept `&JsonPath` in `parse_segments`** or add `$` prefix validation (MAJOR-1)
