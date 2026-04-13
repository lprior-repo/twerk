# Architectural Drift Report — ASL Module

**Date**: 2025-07-17
**Scope**: `crates/twerk-core/src/asl/` (16 files), `eval/intrinsics.rs`, `eval/data_flow.rs`
**Verdict**: **STATUS: APPROVED**

---

## 1. File Size Convention (<300 lines)

| File | Lines | Status |
|------|------:|--------|
| `asl/types.rs` | 331 | ⚠️ ADVISORY |
| `eval/intrinsics.rs` | 298 | ✅ |
| `asl/wait.rs` | 262 | ✅ |
| `asl/task_state.rs` | 224 | ✅ |
| `eval/data_flow.rs` | 223 | ✅ |
| `asl/validation.rs` | 199 | ✅ |
| `asl/machine.rs` | 191 | ✅ |
| `asl/retrier.rs` | 185 | ✅ |
| `asl/map.rs` | 182 | ✅ |
| `asl/transition.rs` | 168 | ✅ |
| `asl/state.rs` | 149 | ✅ |
| `asl/choice.rs` | 126 | ✅ |
| `asl/catcher.rs` | 117 | ✅ |
| `asl/parallel.rs` | 108 | ✅ |
| `asl/terminal.rs` | 97 | ✅ |
| `asl/error_code.rs` | 86 | ✅ |
| `asl/pass.rs` | 37 | ✅ |
| `asl/mod.rs` | 34 | ✅ |

**`types.rs` (331 lines)**: 31 lines over the 300-line limit. However, this file
contains 8 cohesive newtype definitions sharing a `str_newtype_impls!` macro.
Splitting would fragment tightly-coupled code and break macro locality.
**Accepted as advisory** — monitor if more types are added.

## 2. Scott Wlaschin DDD Principles

### ✅ NewTypes over raw primitives
Excellent coverage. Domain boundaries use validated newtypes:
- `StateName`, `Expression`, `JsonPath`, `VariableName` (string domain types)
- `ImageRef`, `ShellScript` (resource references)
- `BackoffRate` (constrained numeric)

All enforce invariants at construction time. **"Parse, don't validate"** is
consistently applied — every `new()` returns `Result<Self, Error>`.

### ✅ Sum types over boolean flags
- `Transition` = `Next(StateName) | End` — not a `(Option<StateName>, bool)` pair
- `WaitDuration` = `Seconds | Timestamp | SecondsPath | TimestampPath` — mutually exclusive
- `StateKind` = 8-variant enum covering all ASL state types
- `ErrorCode` = proper enum with wildcard matching

The `is_seconds()`, `is_end()` etc. methods are query helpers on sum types, not
boolean-flag-driven design. Correct pattern.

### ✅ Make illegal states unrepresentable
- `Transition` cannot have both `Next` and `End` (impossible by construction)
- `WaitDuration` cannot have multiple fields set (enum, not struct)
- `TaskState` validates heartbeat < timeout at construction
- `Retrier` validates interval > 0, max_attempts > 0 at construction
- `Catcher` requires non-empty `error_equals` at construction

### ℹ️ Informational (not blocking)
- `HashMap<String, Expression>` in `TaskState.env` — raw `String` as key.
  Defensible since env var names are OS-level, not ASL domain concepts.
- `timeout: Option<u64>` / `heartbeat: Option<u64>` — raw seconds. Validation
  happens in constructor (INV-TS constraints). Low risk.

## 3. Module Boundaries

### ✅ Encapsulation
`mod.rs` provides a clean facade:
- 15 `pub mod` declarations (no leaking of internals)
- Selective `pub use` re-exports: types + errors only
- Consumers import `use crate::asl::{StateMachine, TaskState, ...}`

### ✅ No circular dependencies
- ASL files reference only siblings via `use super::`
- `eval/data_flow.rs` → `crate::asl::types::JsonPath` (correct direction)
- `eval/intrinsics.rs` → `super::context` (within eval module, no asl dep)
- No back-references from `asl/` into `eval/`

## 4. Dependency Direction

### ✅ ASL module (core domain)
Only depends on: `std`, `serde`, `indexmap`, `thiserror`, `md5` (for ErrorCode).
**Zero infrastructure dependencies.** Clean domain layer.

### ✅ Eval module (application layer)
- `intrinsics.rs`: `base64`, `evalexpr`, `sha2`, `md5`, `uuid`, `rand`, `serde_json`
- `data_flow.rs`: `serde_json`, `thiserror`, `crate::asl::types::JsonPath`

Dependency direction is correct: eval depends on asl, never the reverse.

## 5. Naming Conventions

### ✅ All clear
- Types: `CamelCase` — `StateMachine`, `TaskState`, `ChoiceRule`, etc.
- Functions: `snake_case` — `apply_input_path`, `math_add_fn`, etc.
- Modules: `snake_case` — `task_state`, `error_code`, `data_flow`, etc.
- Errors: `{Type}Error` pattern — `StateNameError`, `TransitionError`, etc.
- No violations detected.

## 6. Dead Code

### ✅ Zero warnings
`cargo check -p twerk-core` produced no warnings for unused imports,
dead code, or unreachable patterns.

---

## Summary

| Check | Result |
|-------|--------|
| File size (<300 lines) | ⚠️ 1 advisory (`types.rs` at 331) |
| DDD / NewTypes | ✅ Excellent |
| Sum types over booleans | ✅ Clean |
| Illegal states unrepresentable | ✅ Enforced |
| Module encapsulation | ✅ Clean facade |
| Circular dependencies | ✅ None |
| Dependency direction | ✅ Correct |
| Naming conventions | ✅ Consistent |
| Dead code | ✅ Zero warnings |

**STATUS: APPROVED**

No blocking issues. The `types.rs` 331-line advisory should be revisited if
the file grows further — at that point, extract `BackoffRate` (the only
non-string newtype) into its own file.
