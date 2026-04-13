---
bead_id: twerk-bp2
bead_title: Eval Engine: State-based evaluation dispatch
phase: state-1-contract
updated_at: 2026-04-13T01:10:00Z
---

# Contract Specification

## Context

- **Feature**: define the STATE 1 contract for ASL-native evaluation dispatch in `crates/twerk-core/src/eval/`.
- **ASL mainline anchor**:
  - `crate::asl::state::StateKind` is the real 8-variant dispatch union: `Task | Pass | Choice | Wait | Succeed | Fail | Parallel | Map`.
  - `crate::asl::state::State` is the shared wrapper carrying `comment`, `input_path`, `output_path`, `assign`, and a flattened `StateKind`.
  - `crate::asl::machine::StateMachine` is the recursive container used at the top level, inside `ParallelState.branches`, and inside `MapState.item_processor`.
  - Variant payloads come from the current ASL module, especially `TaskState`, `ChoiceState`, `WaitState`, `PassState`, `ParallelState`, `MapState`, `SucceedState`, and `FailState`.
- **Current eval module surface**:
  - `evaluate_template(&str, &HashMap<String, Value>) -> Result<String, EvalError>`
  - `evaluate_expr(&str, &HashMap<String, Value>) -> Result<serde_json::Value, EvalError>`
  - `evaluate_task(&crate::task::Task, &HashMap<String, Value>) -> Result<crate::task::Task, EvalError>`
  - `create_context(&HashMap<String, Value>) -> Result<HashMapContext, EvalError>`
- **Problem statement**: the existing recursive evaluator is still anchored on the legacy `crate::task::Task` tree. STATE 1 must move the contract boundary to the real ASL model instead of inventing new `Task`-shaped dispatcher inputs.
- **Key design consequence**: the ASL mainline stores validated newtypes (`Expression`, `JsonPath`, `StateName`, `ImageRef`, `ShellScript`, `Transition`) rather than the old raw-string task fields. Therefore STATE 1 is a **shape-preserving ASL dispatcher** over validated state types. It does not regress the contract back into the legacy task aggregate.

### Assumptions

- Input ASL values already passed constructor / serde validation before reaching evaluation dispatch.
- `StateMachine::validate()` is the graph-validity gate for top-level and nested machines.
- Existing eval helpers remain the only authoritative expression/template engine in this workspace.
- `PassState.result`, `FailState.error`, `FailState.cause`, `State.comment`, and `WaitDuration::Timestamp` are treated as literal state-definition data in STATE 1.
- `TaskState.run` remains raw `ShellScript`; STATE 1 must not introduce template expansion for it.
- Malformed `Expression` payloads remain deferred to later runtime phases; STATE 1 preserves them unchanged instead of syntax-checking them.

### Open Questions

- None. The current workspace exposes enough ASL and eval surface to write the STATE 1 contract.

## Preconditions

- Callers use ASL entrypoints with `State`, `StateKind`, and `StateMachine`; they do not supply `crate::task::Task` as the public dispatcher input.
- The evaluation context is representable as `HashMap<String, serde_json::Value>` and is acceptable to the existing eval helpers.
- Callers are expected to pass validated `StateMachine` graphs, but `evaluate_state_machine` defensively returns exact `StateMachineError` values if an invalid graph still reaches the public boundary.
- All variant-specific constructor invariants already hold, including:
  - `TaskState`: timeout / heartbeat / env-key rules
  - `ChoiceState`: non-empty `choices`
  - `ParallelState`: non-empty `branches`
  - `MapState`: finite `tolerated_failure_percentage` in `0.0..=100.0`

## Postconditions

- The contract boundary is ASL-native:
  - `evaluate_state` accepts and returns `State`
  - `evaluate_state_machine` accepts and returns `StateMachine`
  - any internal dispatcher matches on `StateKind`, not on `crate::task::Task`
- Success preserves the incoming ASL layer. No public API in STATE 1 accepts, returns, or exposes the legacy `Task` god object.
- Success is shape-preserving:
  - input `StateKind` variant == output `StateKind` variant
  - shared `State` fields stay attached to the same state
  - validated ASL newtypes remain validated ASL newtypes
- Variant-specific guarantees:
  - **Task**: preserves `image`, `run`, `env`, `var`, `timeout`, `heartbeat`, `retry`, `catch`, and `transition` as `TaskState` fields
  - **Pass**: preserves optional JSON `result` and `transition`
  - **Choice**: preserves rule order, each rule's `condition`, `next`, and optional `assign`, plus `default`; STATE 1 does not select a branch
  - **Wait**: preserves the exact `WaitDuration` variant and `transition`
  - **Succeed / Fail**: remain terminal and never gain a transition
  - **Parallel**: preserves branch count and branch order; each branch remains a `StateMachine`
  - **Map**: preserves `items_path`, `item_processor`, `max_concurrency`, `retry`, `catch`, `tolerated_failure_percentage`, and `transition`
- STATE 1 does **not** eagerly execute or downgrade typed ASL fields:
  - `State.assign` stays `HashMap<VariableName, Expression>`
  - `ChoiceRule.condition` and `ChoiceRule.assign` stay typed expressions
  - `TaskState.env` values stay `Expression`
  - `MapState.items_path` stays `Expression`
  - `TaskState.run` stays `ShellScript`
- STATE 1 preserves malformed `Expression` payloads unchanged; runtime expression errors belong to later evaluation phases, not this definition-time dispatcher.
- All failures are returned as `Err(...)`; dispatch must not panic.

## Invariants

| ID | Invariant |
|---|---|
| INV-S1-1 | Dispatch is exhaustive over all 8 `StateKind` variants. |
| INV-S1-2 | The public boundary is `State` / `StateMachine`, not `crate::task::Task`. |
| INV-S1-3 | Successful evaluation is variant-preserving: no `StateKind` coercion. |
| INV-S1-4 | Shared `State` fields (`comment`, `input_path`, `output_path`, `assign`) are preserved across dispatch. |
| INV-S1-5 | `Transition` semantics are preserved exactly; terminal states never acquire a transition. |
| INV-S1-6 | Recursive containers preserve topology: `StateMachine.start_at`, state keys, branch order, and `MapState.item_processor` structure are stable on success. |
| INV-S1-7 | Validated ASL newtypes are never downgraded to raw strings at the contract boundary. |
| INV-S1-8 | STATE 1 is definition-time dispatch only; it does not execute a state machine, choose branches, wait on timers, or run containers. |
| INV-S1-9 | STATE 1 does not repurpose the old task evaluator as the public shape of the API. |

## Error Taxonomy

```rust
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StateEvalError {
    #[error("task state invariant violated after ASL dispatch: {0}")]
    TaskState(#[from] TaskStateError),

    #[error("choice state invariant violated after ASL dispatch: {0}")]
    ChoiceState(#[from] ChoiceStateError),

    #[error("parallel state invariant violated after ASL dispatch: {0}")]
    ParallelState(#[from] ParallelStateError),

    #[error("map state invariant violated after ASL dispatch: {0}")]
    MapState(#[from] MapStateError),

    #[error("state machine invalid after recursive ASL dispatch: {0:?}")]
    StateMachine(Vec<StateMachineError>),
}
```

### Error Semantics

- STATE 1 does not delegate to `evaluate_expr`, `evaluate_template`, or `create_context`; malformed runtime expressions remain preserved for later phases instead of failing here.
- `TaskState`, `ChoiceState`, `ParallelState`, and `MapState` mean ASL dispatch attempted to produce data that would violate the validated constructors.
- `StateMachine` means the rebuilt machine fails `StateMachine::validate()`; because STATE 1 preserves topology before that check, the same exact error also surfaces when invalid input graphs reach the boundary.
- `UnsupportedStateKind` is forbidden: `StateKind` is a closed enum and dispatch must be exhaustive.
- No error variant may expose `crate::task::Task` as part of the API contract.

## Contract Signatures

```rust
use std::collections::HashMap;
use serde_json::Value;
use crate::asl::{State, StateKind, StateMachine};

pub fn evaluate_state(
    state: &State,
    context: &HashMap<String, Value>,
) -> Result<State, StateEvalError>;

pub fn evaluate_state_machine(
    machine: &StateMachine,
    context: &HashMap<String, Value>,
) -> Result<StateMachine, StateEvalError>;

fn evaluate_state_kind(
    kind: &StateKind,
    context: &HashMap<String, Value>,
) -> Result<StateKind, StateEvalError>;
```

### Signature Rules

- `evaluate_state` is the canonical public entrypoint for a single ASL state.
- `evaluate_state_machine` is the canonical public entrypoint for recursive ASL containers and top-level machine dispatch.
- `evaluate_state_kind` is the internal exhaustive dispatcher over the real ASL union.
- No STATE 1 signature may accept `crate::task::Task`, `TaskSummary`, or any invented task-shaped adapter as its primary input.

## Non-goals

- Implementation code.
- Test planning.
- Reconstructing the ASL model from the old task model.
- Executing containers, stepping transitions, or selecting a `Choice` branch.
- Applying runtime payload flow via `apply_input_path`, `apply_result_path`, or `apply_output_path`.
- Eagerly evaluating `Expression`-typed ASL fields into raw JSON or strings.
- Template-expanding `TaskState.run`.
