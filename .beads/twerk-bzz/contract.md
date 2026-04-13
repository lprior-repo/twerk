# Contract Specification: twerk-bzz

## ASL Core: State, StateKind Enum, and StateMachine

## Context

- **Feature**: Create the core discriminated union for ASL states and the top-level state machine container
- **Files**:
  - `crates/twerk-core/src/asl/state.rs` -- State wrapper struct + StateKind enum (the discriminated union replacing the Go 30-field Task god object)
  - `crates/twerk-core/src/asl/machine.rs` -- StateMachine struct (named state map with start_at) + StateMachineError enum
- **Domain terms**:
  - **ASL**: Amazon States Language -- JSON/YAML-based language for defining state machines
  - **StateKind**: Discriminated union enum -- each variant holds the specific state type data. Replaces the Go pattern of a single Task struct with 30 optional fields
  - **State**: Wrapper combining shared fields (comment, input_path, output_path, assign) with a StateKind. Serde flattens the kind so the `type` discriminant appears at the top level
  - **StateMachine**: A named map of states with a designated start state. Used both as the top-level definition and as sub-machines embedded in ParallelState and MapState
  - **Terminal state**: A state with no outgoing transition -- Succeed and Fail variants. Execution ends here
  - **IndexMap**: Ordered hash map preserving insertion order with O(1) lookup -- used for `states` to maintain definition order
  - **Validate-after-construction**: Unlike the NewTypes which validate at construction, StateMachine validates inter-state relationships (references between states) via an explicit `validate()` method
- **Dependency types** (from twerk-fq8, `asl/types.rs`):
  - `StateName` -- validated 1-256 char string (INV-1: non-empty, <= 256 chars)
  - `Expression` -- validated non-empty string (INV-2)
  - `JsonPath` -- validated non-empty string starting with `$` (INV-3)
  - `VariableName` -- validated identifier, 1-128 chars, `[a-zA-Z_][a-zA-Z0-9_]*` (INV-4)
- **Dependency types** (from twerk-9xv):
  - `Transition` -- enum: `Next(StateName)` | `End` with custom serde (INV-T1: exactly one variant)
- **Dependency types** (from twerk-snj):
  - `ChoiceRule` -- struct: `condition: Expression`, `next: StateName`, `assign: Option<HashMap<VariableName, Expression>>`
  - `ChoiceState` -- struct: `choices: Vec<ChoiceRule>` (min 1), `default: Option<StateName>`
  - `WaitDuration` -- enum: `Seconds(u64)` | `Timestamp(String)` | `SecondsPath(JsonPath)` | `TimestampPath(JsonPath)`
  - `WaitState` -- struct: `duration: WaitDuration`, `transition: Transition`
  - `PassState` -- struct: `result: Option<serde_json::Value>`, `transition: Transition`
  - `SucceedState` -- unit struct (no fields, no transition, terminal)
  - `FailState` -- struct: `error: Option<String>`, `cause: Option<String>` (terminal)
- **Dependency types** (from twerk-14f):
  - `TaskState` -- struct: `image: ImageRef`, `run: ShellScript`, `env: HashMap<String, Expression>`, `var: Option<VariableName>`, `timeout: Option<u64>`, `heartbeat: Option<u64>`, `retry: Vec<Retrier>`, `catch: Vec<Catcher>`, `transition: Transition`
  - `ParallelState` -- struct: `branches: Vec<StateMachine>`, `transition: Transition`, `fail_fast: Option<bool>` (circular ref -- uses this bead's StateMachine)
  - `MapState` -- struct: `items_path: Expression`, `item_processor: StateMachine`, `max_concurrency: Option<u32>`, `transition: Transition`, `retry: Vec<Retrier>`, `catch: Vec<Catcher>`, `tolerated_failure_percentage: Option<f64>` (circular ref -- uses this bead's StateMachine)
- **Cargo.toml dependency**: `indexmap` crate with `serde` feature must be added to twerk-core
- **Assumptions**:
  - All dependency types already exist and are validated at construction (parse-don't-validate)
  - StateKind uses serde tagged enum: `#[serde(tag = "type", rename_all = "lowercase")]`
  - State uses `#[serde(flatten)]` on the `kind` field so the `type` discriminant from StateKind appears at the top level of the serialized object alongside `comment`, `input_path`, etc.
  - StateMachine validation is a separate step from deserialization -- you can deserialize an invalid machine (e.g. dangling transition targets) and then call `validate()` to check all invariants
  - `validate()` collects ALL errors rather than short-circuiting on the first one
  - `start_state()` is infallible because it is only safe to call after `validate()` succeeds
  - StateMachine is recursive: ParallelState.branches and MapState.item_processor contain StateMachine instances
  - No `Eq` or `Hash` on State, StateKind, or StateMachine because some variants contain f64 transitively (PassState via serde_json::Value, MapState via tolerated_failure_percentage, TaskState via BackoffRate)
- **Open questions**: None

---

## Types

### File: `crates/twerk-core/src/asl/state.rs`

#### 1. StateKind

```
enum StateKind {
    Task(TaskState),
    Pass(PassState),
    Choice(ChoiceState),
    Wait(WaitState),
    Succeed(SucceedState),
    Fail(FailState),
    Parallel(ParallelState),
    Map(MapState),
}
```

| Attribute | Value |
|-----------|-------|
| Variants | `Task(TaskState)`, `Pass(PassState)`, `Choice(ChoiceState)`, `Wait(WaitState)`, `Succeed(SucceedState)`, `Fail(FailState)`, `Parallel(ParallelState)`, `Map(MapState)` |
| Serde | `#[serde(tag = "type", rename_all = "lowercase")]` -- discriminated by `type` field with values `task`, `pass`, `choice`, `wait`, `succeed`, `fail`, `parallel`, `map` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` |

**NOT** derived: `Eq`, `Hash` (TaskState contains BackoffRate wrapping f64; PassState contains serde_json::Value wrapping f64; MapState contains tolerated_failure_percentage f64).

**Methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `is_terminal` | `pub fn is_terminal(&self) -> bool` | Returns `true` for `Succeed` and `Fail` variants; `false` for all others |
| `transition` | `pub fn transition(&self) -> Option<&Transition>` | Returns `Some(&transition)` for Task, Pass, Wait, Parallel, Map; returns `None` for Choice (routing is via choices/default) and Succeed/Fail (terminal, no transition) |

#### 2. State

```
struct State {
    comment:     Option<String>,
    input_path:  Option<JsonPath>,
    output_path: Option<JsonPath>,
    assign:      Option<HashMap<VariableName, Expression>>,
    kind:        StateKind,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `comment: Option<String>`, `input_path: Option<JsonPath>`, `output_path: Option<JsonPath>`, `assign: Option<HashMap<VariableName, Expression>>`, `kind: StateKind` |
| Serde | `#[serde(rename_all = "camelCase")]`; `kind` field uses `#[serde(flatten)]` so StateKind's `type` discriminant and variant fields appear at top level |
| Derives | `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` |
| Optional field serde | All `Option` fields use `#[serde(skip_serializing_if = "Option::is_none")]` |

**NOT** derived: `Eq`, `Hash` (inherits from StateKind).

**Serde flatten behavior**: A serialized State with a TaskState kind looks like:
```yaml
type: task
comment: "Run the container"
inputPath: "$.data"
image: "alpine:3.18"
run: "echo hello"
next: "NextState"
```
The `type`, `image`, `run`, `next` come from the flattened StateKind/TaskState; `comment`, `inputPath` come from State's own fields.

### File: `crates/twerk-core/src/asl/machine.rs`

#### 3. StateMachine

```
struct StateMachine {
    comment:  Option<String>,
    start_at: StateName,
    states:   IndexMap<StateName, State>,
    timeout:  Option<u64>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `comment: Option<String>`, `start_at: StateName`, `states: IndexMap<StateName, State>`, `timeout: Option<u64>` |
| Serde | `#[serde(rename_all = "camelCase")]`; `comment` and `timeout` with `#[serde(skip_serializing_if = "Option::is_none")]` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` |
| IndexMap | `indexmap::IndexMap` with `serde` feature -- preserves insertion order, O(1) key lookup |

**NOT** derived: `Eq`, `Hash` (transitively contains f64 via State/StateKind).

**Methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `validate` | `pub fn validate(&self) -> Result<(), Vec<StateMachineError>>` | Checks ALL 6 invariants; collects all errors into Vec; returns `Ok(())` only if all pass |
| `get_state` | `pub fn get_state(&self, name: &StateName) -> Option<&State>` | Looks up a state by name in the IndexMap |
| `start_state` | `pub fn start_state(&self) -> &State` | Returns the state referenced by `start_at`. Infallible -- caller must validate first. Panics if `start_at` is not found (contract: only call after `validate()` succeeds) |

#### 4. StateMachineError

```
enum StateMachineError {
    EmptyStates,
    StartAtNotFound { start_at: StateName },
    TransitionTargetNotFound { from: StateName, target: StateName },
    ChoiceTargetNotFound { from: StateName, target: StateName },
    DefaultTargetNotFound { from: StateName, target: StateName },
    NoTerminalState,
}
```

| Attribute | Value |
|-----------|-------|
| Variants | See above |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq` |
| Display | Implements `std::fmt::Display` with human-readable messages |
| Error | Implements `std::error::Error` (via `thiserror`) |

---

## Invariants

These must be checked by `StateMachine::validate()`:

| ID | Invariant | Trigger |
|----|-----------|---------|
| SM-1 | `start_at` must reference a key in `states` | `StateMachineError::StartAtNotFound { start_at }` |
| SM-2 | `states` must not be empty | `StateMachineError::EmptyStates` |
| SM-3 | Every `Transition::Next(name)` in any state's `kind.transition()` must reference a key in `states` | `StateMachineError::TransitionTargetNotFound { from, target }` |
| SM-4 | Every `ChoiceRule.next` in any `ChoiceState.choices` must reference a key in `states` | `StateMachineError::ChoiceTargetNotFound { from, target }` |
| SM-5 | Every `ChoiceState.default` (if `Some`) must reference a key in `states` | `StateMachineError::DefaultTargetNotFound { from, target }` |
| SM-6 | At least one state must be terminal (`StateKind::Succeed`, `StateKind::Fail`, or any state whose transition is `Transition::End`) | `StateMachineError::NoTerminalState` |

**Invariant checking order**: SM-2 first (short-circuit if empty -- other checks are meaningless). Then SM-1, SM-3, SM-4, SM-5, SM-6 all checked, collecting all errors.

**Invariant scope**: `validate()` checks only direct state references. It does NOT recursively validate sub-StateMachines inside ParallelState.branches or MapState.item_processor -- those are validated independently when they are constructed/validated.

### StateKind invariants (enforced by type system, not by validate)

| ID | Type | Invariant |
|----|------|-----------|
| INV-SK1 | `StateKind` | Exactly one variant is active (enforced by Rust enum representation) |
| INV-SK2 | `StateKind::Task` | Inner `TaskState` satisfies INV-TS1 through INV-TS11 |
| INV-SK3 | `StateKind::Pass` | Inner `PassState` satisfies INV-PS1 |
| INV-SK4 | `StateKind::Choice` | Inner `ChoiceState` satisfies INV-CS1 through INV-CS3 |
| INV-SK5 | `StateKind::Wait` | Inner `WaitState` satisfies INV-WS1, INV-WS2 |
| INV-SK6 | `StateKind::Succeed` | Inner `SucceedState` satisfies INV-SS1 (trivially) |
| INV-SK7 | `StateKind::Fail` | Inner `FailState` satisfies INV-FS1 (trivially) |
| INV-SK8 | `StateKind::Parallel` | Inner `ParallelState` satisfies INV-PS1, INV-PS2 |
| INV-SK9 | `StateKind::Map` | Inner `MapState` satisfies INV-MS1 through INV-MS5 |

### State invariants

| ID | Type | Invariant |
|----|------|-----------|
| INV-ST1 | `State` | If `input_path` is `Some(p)`, then `p` satisfies JsonPath invariants (INV-3) |
| INV-ST2 | `State` | If `output_path` is `Some(p)`, then `p` satisfies JsonPath invariants (INV-3) |
| INV-ST3 | `State` | If `assign` is `Some(map)`, every key satisfies VariableName invariants (INV-4) and every value satisfies Expression invariants (INV-2) |
| INV-ST4 | `State` | `kind` satisfies INV-SK1 through INV-SK9 as applicable |

---

## StateMachineError Variant-to-Trigger Mapping

| Error Variant | Triggering Condition | Example |
|---------------|---------------------|---------|
| `EmptyStates` | `self.states.is_empty()` | StateMachine with no states defined |
| `StartAtNotFound { start_at }` | `!self.states.contains_key(&self.start_at)` | `start_at: "Init"` but no state named "Init" in `states` |
| `TransitionTargetNotFound { from, target }` | A state named `from` has `kind.transition() == Some(Transition::Next(target))` and `!self.states.contains_key(&target)` | State "Step1" has `next: "Step2"` but "Step2" does not exist |
| `ChoiceTargetNotFound { from, target }` | A ChoiceState named `from` has a rule with `rule.next == target` and `!self.states.contains_key(&target)` | Choice state "Router" has rule pointing to "Handler" but "Handler" does not exist |
| `DefaultTargetNotFound { from, target }` | A ChoiceState named `from` has `default == Some(target)` and `!self.states.contains_key(&target)` | Choice state "Router" has `default: "Fallback"` but "Fallback" does not exist |
| `NoTerminalState` | No state in `states` satisfies `kind.is_terminal()` and no state has `kind.transition() == Some(Transition::End)` | All states transition to other states; none end execution |

---

## Serde Behavior

### StateKind: Tagged enum discrimination

- **Tag field**: `"type"` at the top level of the serialized object
- **Tag values**: `"task"`, `"pass"`, `"choice"`, `"wait"`, `"succeed"`, `"fail"`, `"parallel"`, `"map"`
- **Rename**: `rename_all = "lowercase"` maps variant names to lowercase tag values
- **Inner fields**: Each variant's struct fields are flattened into the same object as the `type` tag

Example -- a TaskState variant serializes as:
```json
{
  "type": "task",
  "image": "alpine:3.18",
  "run": "echo hello",
  "next": "NextState"
}
```

### State: Flatten composition

- State's own fields (`comment`, `inputPath`, `outputPath`, `assign`) appear at the top level
- `#[serde(flatten)]` on `kind` merges StateKind's fields (including `type` tag) into the same level
- Combined result:
```json
{
  "type": "task",
  "comment": "Run container",
  "inputPath": "$.data",
  "image": "alpine:3.18",
  "run": "echo hello",
  "next": "NextState"
}
```

### StateMachine: camelCase with IndexMap

- `start_at` serializes as `"startAt"`
- `states` serializes as an ordered JSON object preserving IndexMap insertion order
- `timeout` serializes as `"timeout"` (already camelCase)
- IndexMap requires `indexmap` crate with `serde` feature flag

Example:
```json
{
  "comment": "My workflow",
  "startAt": "Init",
  "states": {
    "Init": { "type": "pass", "next": "Done" },
    "Done": { "type": "succeed" }
  },
  "timeout": 3600
}
```

### Deserialization notes

- StateKind deserialization uses serde's internally tagged enum: the `type` field is read first, then the remaining fields are deserialized into the appropriate variant struct
- State deserialization: serde first extracts `comment`, `input_path`, `output_path`, `assign`, then passes remaining fields to StateKind deserializer via flatten
- StateMachine deserialization: standard derive -- no custom deserializer needed since validation is a separate step via `validate()`

---

## Defaults

| Type | Field | Default |
|------|-------|---------|
| `State` | `comment` | `None` |
| `State` | `input_path` | `None` |
| `State` | `output_path` | `None` |
| `State` | `assign` | `None` |
| `StateMachine` | `comment` | `None` |
| `StateMachine` | `timeout` | `None` |

---

## Trait Requirements

### StateKind

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(tag = "type", rename_all = "lowercase")]` |
| `Deserialize` | Derive with same serde attributes |

**NOT** derived: `Eq`, `Hash` (variants contain f64 transitively).

### State

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]`; `kind` flattened |
| `Deserialize` | Derive with same serde attributes |

**NOT** derived: `Eq`, `Hash` (inherits from StateKind).

### StateMachine

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Derive with same serde attributes |

**NOT** derived: `Eq`, `Hash` (transitively contains f64 via State/StateKind).

### StateMachineError

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive (StateName impls Eq) |
| `Display` | Via `thiserror` `#[error("...")]` attributes |
| `Error` | Via `thiserror` derive |

---

## Preconditions

### StateKind::is_terminal
- **Pre**: None (infallible, total function on all variants)

### StateKind::transition
- **Pre**: None (infallible, total function on all variants)

### StateMachine::validate
- **Pre**: None (works on any StateMachine instance, even structurally invalid ones)

### StateMachine::get_state
- **Pre**: `name` is a valid StateName (enforced by type)

### StateMachine::start_state
- **Pre-SM-START**: `self.validate()` has returned `Ok(())` -- specifically, SM-1 must hold (start_at references a valid key). If this precondition is violated, the method panics.

---

## Postconditions

### StateKind::is_terminal
- **Post-TERM-1**: Returns `true` if and only if `self` matches `StateKind::Succeed(_)` or `StateKind::Fail(_)`
- **Post-TERM-2**: Returns `false` for `Task`, `Pass`, `Choice`, `Wait`, `Parallel`, `Map`

### StateKind::transition
- **Post-TRANS-1**: Returns `Some(&t)` where `t` is the `Transition` field for `Task`, `Pass`, `Wait`, `Parallel`, `Map` variants
- **Post-TRANS-2**: Returns `None` for `Choice` (routing is via choices/default, not a single transition)
- **Post-TRANS-3**: Returns `None` for `Succeed` and `Fail` (terminal states have no transition)

### StateMachine::validate
- **Post-VAL-1**: If `Ok(())` is returned, ALL of SM-1 through SM-6 hold
- **Post-VAL-2**: If `Err(errors)` is returned, `errors` is non-empty and contains every violated invariant (not just the first)
- **Post-VAL-3**: The `errors` Vec contains at most one `EmptyStates` and at most one `NoTerminalState`
- **Post-VAL-4**: The `errors` Vec may contain multiple `TransitionTargetNotFound`, `ChoiceTargetNotFound`, and `DefaultTargetNotFound` (one per dangling reference)
- **Post-VAL-5**: If `states` is empty, the only error is `EmptyStates` (other checks are skipped)

### StateMachine::get_state
- **Post-GET-1**: Returns `Some(&state)` if `name` is a key in `self.states`
- **Post-GET-2**: Returns `None` if `name` is not a key in `self.states`

### StateMachine::start_state
- **Post-START-1**: Returns a reference to the State whose key equals `self.start_at`
- **Post-START-2**: The returned reference is the same as `self.get_state(&self.start_at).unwrap()`

---

## Contract Signatures

```rust
// --- state.rs ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StateKind {
    Task(TaskState),
    Pass(PassState),
    Choice(ChoiceState),
    Wait(WaitState),
    Succeed(SucceedState),
    Fail(FailState),
    Parallel(ParallelState),
    Map(MapState),
}

impl StateKind {
    /// Returns true for Succeed and Fail; false otherwise.
    pub fn is_terminal(&self) -> bool;

    /// Returns the Transition for states that have one.
    /// None for Choice (uses choices/default) and terminals (Succeed, Fail).
    pub fn transition(&self) -> Option<&Transition>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_path: Option<JsonPath>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<JsonPath>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub assign: Option<HashMap<VariableName, Expression>>,

    #[serde(flatten)]
    pub kind: StateKind,
}

// --- machine.rs ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMachine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    pub start_at: StateName,

    pub states: IndexMap<StateName, State>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

impl StateMachine {
    /// Validates all 6 invariants. Returns all errors, not just the first.
    pub fn validate(&self) -> Result<(), Vec<StateMachineError>>;

    /// Looks up a state by name.
    pub fn get_state(&self, name: &StateName) -> Option<&State>;

    /// Returns the start state. Panics if start_at is not in states.
    /// PRECONDITION: validate() returned Ok(()).
    pub fn start_state(&self) -> &State;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StateMachineError {
    #[error("states map is empty")]
    EmptyStates,

    #[error("start_at '{start_at}' does not reference a state")]
    StartAtNotFound { start_at: StateName },

    #[error("state '{from}' transitions to '{target}' which does not exist")]
    TransitionTargetNotFound { from: StateName, target: StateName },

    #[error("choice state '{from}' has rule targeting '{target}' which does not exist")]
    ChoiceTargetNotFound { from: StateName, target: StateName },

    #[error("choice state '{from}' has default '{target}' which does not exist")]
    DefaultTargetNotFound { from: StateName, target: StateName },

    #[error("no terminal state found (need at least one Succeed, Fail, or end: true)")]
    NoTerminalState,
}
```

---

## Non-goals

- No execution logic -- StateMachine is a data structure only; interpretation/stepping is a separate concern
- No recursive validation of sub-StateMachines in ParallelState or MapState -- each sub-machine validates independently
- No builder pattern -- StateMachine is constructed directly (fields are public)
- No graph cycle detection -- ASL does not forbid cycles (loops are valid)
- No Catcher.next validation -- Catcher target validation is deferred (Catcher.next is a StateName referencing the containing machine's states, but Catcher is validated at its own level)
- No runtime semantics (execution order, input/output processing, variable resolution)

---

## Given-When-Then Scenarios

### Scenario 1: StateKind discrimination -- Task variant

**Given**: A `StateKind::Task(task_state)` where `task_state.transition` is `Transition::Next(StateName("Step2"))`
**When**: `is_terminal()` is called
**Then**:
- Returns `false`

**When**: `transition()` is called
**Then**:
- Returns `Some(&Transition::Next(StateName("Step2")))`

### Scenario 2: StateKind discrimination -- Succeed variant

**Given**: A `StateKind::Succeed(SucceedState)`
**When**: `is_terminal()` is called
**Then**:
- Returns `true`

**When**: `transition()` is called
**Then**:
- Returns `None`

### Scenario 3: StateKind discrimination -- Fail variant

**Given**: A `StateKind::Fail(FailState { error: Some("CustomError"), cause: Some("bad input") })`
**When**: `is_terminal()` is called
**Then**:
- Returns `true`

**When**: `transition()` is called
**Then**:
- Returns `None`

### Scenario 4: StateKind discrimination -- Choice variant

**Given**: A `StateKind::Choice(choice_state)` with `choices` containing rules targeting "BranchA" and "BranchB"
**When**: `is_terminal()` is called
**Then**:
- Returns `false`

**When**: `transition()` is called
**Then**:
- Returns `None` (Choice uses its own routing mechanism, not a single transition)

### Scenario 5: StateKind discrimination -- all non-terminal transitioning variants

**Given**: Each of `StateKind::Pass`, `StateKind::Wait`, `StateKind::Parallel`, `StateKind::Map` with a `Transition::End`
**When**: `is_terminal()` is called on each
**Then**:
- Returns `false` for all (they are not terminal state types; `Transition::End` means the machine ends after this state executes, but the state itself is not a terminal type)

**When**: `transition()` is called on each
**Then**:
- Returns `Some(&Transition::End)` for all

### Scenario 6: State construction with shared fields

**Given**: A YAML/JSON document:
```yaml
type: task
comment: "Execute the build"
inputPath: "$.build"
outputPath: "$.result"
assign:
  build_id: "$.context.id"
image: "golang:1.21"
run: "go build ./..."
next: "Verify"
```
**When**: Deserialized into a `State`
**Then**:
- `state.comment == Some("Execute the build")`
- `state.input_path == Some(JsonPath("$.build"))`
- `state.output_path == Some(JsonPath("$.result"))`
- `state.assign == Some({"build_id": Expression("$.context.id")})`
- `state.kind` matches `StateKind::Task(_)` with image "golang:1.21" and next "Verify"

### Scenario 7: State construction with minimal fields

**Given**: A YAML/JSON document:
```yaml
type: succeed
```
**When**: Deserialized into a `State`
**Then**:
- `state.comment == None`
- `state.input_path == None`
- `state.output_path == None`
- `state.assign == None`
- `state.kind` matches `StateKind::Succeed(SucceedState)`

### Scenario 8: StateMachine validation -- all invariants pass

**Given**: A StateMachine with:
- `start_at: "Init"`
- `states`: `{"Init": Pass(next: "Done"), "Done": Succeed}`
**When**: `validate()` is called
**Then**:
- Returns `Ok(())`

### Scenario 9: StateMachine validation -- empty states (SM-2)

**Given**: A StateMachine with:
- `start_at: "Init"`
- `states`: `{}` (empty)
**When**: `validate()` is called
**Then**:
- Returns `Err(vec![StateMachineError::EmptyStates])`
- Only `EmptyStates` is returned (other checks are skipped)

### Scenario 10: StateMachine validation -- start_at not found (SM-1)

**Given**: A StateMachine with:
- `start_at: "Missing"`
- `states`: `{"Init": Pass(next: "Done"), "Done": Succeed}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains `StateMachineError::StartAtNotFound { start_at: "Missing" }`

### Scenario 11: StateMachine validation -- transition target not found (SM-3)

**Given**: A StateMachine with:
- `start_at: "Init"`
- `states`: `{"Init": Pass(next: "Ghost"), "End": Succeed}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains `StateMachineError::TransitionTargetNotFound { from: "Init", target: "Ghost" }`

### Scenario 12: StateMachine validation -- choice target not found (SM-4)

**Given**: A StateMachine with:
- `start_at: "Router"`
- `states`: `{"Router": Choice(choices: [{condition: "$.x > 0", next: "Phantom"}], default: None), "End": Succeed}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains `StateMachineError::ChoiceTargetNotFound { from: "Router", target: "Phantom" }`

### Scenario 13: StateMachine validation -- default target not found (SM-5)

**Given**: A StateMachine with:
- `start_at: "Router"`
- `states`: `{"Router": Choice(choices: [{condition: "$.x > 0", next: "End"}], default: Some("Ghost")), "End": Succeed}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains `StateMachineError::DefaultTargetNotFound { from: "Router", target: "Ghost" }`

### Scenario 14: StateMachine validation -- no terminal state (SM-6)

**Given**: A StateMachine with:
- `start_at: "A"`
- `states`: `{"A": Pass(next: "B"), "B": Pass(next: "A")}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains `StateMachineError::NoTerminalState`

### Scenario 15: StateMachine validation -- multiple errors collected

**Given**: A StateMachine with:
- `start_at: "Missing"`
- `states`: `{"A": Pass(next: "Ghost1"), "B": Pass(next: "Ghost2")}`
**When**: `validate()` is called
**Then**:
- Returns `Err(errors)` where `errors` contains ALL of:
  - `StartAtNotFound { start_at: "Missing" }`
  - `TransitionTargetNotFound { from: "A", target: "Ghost1" }`
  - `TransitionTargetNotFound { from: "B", target: "Ghost2" }`
  - `NoTerminalState`
- `errors.len() == 4`

### Scenario 16: StateMachine validation -- Transition::End counts as terminal

**Given**: A StateMachine with:
- `start_at: "Init"`
- `states`: `{"Init": Pass(end: true)}`
**When**: `validate()` is called
**Then**:
- Returns `Ok(())` because `Transition::End` satisfies SM-6 (state is terminal in effect)

### Scenario 17: StateMachine get_state and start_state

**Given**: A validated StateMachine with `start_at: "Init"`, `states`: `{"Init": Pass(next: "Done"), "Done": Succeed}`
**When**: `get_state(&StateName("Init"))` is called
**Then**:
- Returns `Some(&state)` where `state.kind` matches `StateKind::Pass(_)`

**When**: `get_state(&StateName("Unknown"))` is called
**Then**:
- Returns `None`

**When**: `start_state()` is called
**Then**:
- Returns a reference to the "Init" state (same as `get_state(&StateName("Init")).unwrap()`)

### Scenario 18: StateMachine serde round-trip with IndexMap ordering

**Given**: A StateMachine serialized to JSON:
```json
{
  "startAt": "First",
  "states": {
    "First": { "type": "pass", "next": "Second" },
    "Second": { "type": "pass", "next": "Third" },
    "Third": { "type": "succeed" }
  }
}
```
**When**: Deserialized and then re-serialized
**Then**:
- The `states` keys appear in the same order: "First", "Second", "Third"
- IndexMap preserves insertion order through round-trip

### Scenario 19: StateMachine with recursive sub-machines

**Given**: A StateMachine with a ParallelState containing branches that are each StateMachines:
```json
{
  "startAt": "Fork",
  "states": {
    "Fork": {
      "type": "parallel",
      "branches": [
        { "startAt": "SubA", "states": { "SubA": { "type": "succeed" } } },
        { "startAt": "SubB", "states": { "SubB": { "type": "succeed" } } }
      ],
      "next": "Join"
    },
    "Join": { "type": "succeed" }
  }
}
```
**When**: The outer StateMachine is deserialized and `validate()` is called
**Then**:
- Returns `Ok(())` for the outer machine
- Sub-machine validation is NOT checked by the outer `validate()` -- each sub-machine must be validated independently

### Scenario 20: StateMachine serde -- unknown type tag rejected

**Given**: A JSON document with `"type": "unknown_state_type"`
**When**: Deserialized as a `State`
**Then**:
- Deserialization fails with a serde error indicating unknown variant
- Only `task`, `pass`, `choice`, `wait`, `succeed`, `fail`, `parallel`, `map` are valid

---

## Error Taxonomy

### Compile-time safety (enforced by Rust type system)

| Safety | Mechanism |
|--------|-----------|
| StateKind variant exhaustiveness | `match` on StateKind must handle all 8 variants |
| State field types | JsonPath, VariableName, Expression enforce format at construction |
| StateName validity | 1-256 char, non-empty -- enforced at construction |
| Transition mutual exclusivity | `Next` vs `End` enforced by enum |

### Deserialization errors (serde)

| Error | Cause |
|-------|-------|
| Unknown `type` tag | `type` field value not in `{task, pass, choice, wait, succeed, fail, parallel, map}` |
| Missing `type` tag | No `type` field in the serialized State |
| Missing required fields | Variant-specific required fields not present (e.g., TaskState without `image`) |
| Invalid NewType value | Inner type validation fails (e.g., empty StateName, JsonPath not starting with `$`) |
| Conflicting flatten fields | Ambiguous field names between State shared fields and StateKind variant fields |

### Validation errors (StateMachineError)

| Error | Severity | Cause |
|-------|----------|-------|
| `EmptyStates` | Fatal | No states defined -- machine is vacuous |
| `StartAtNotFound` | Fatal | Entry point references non-existent state |
| `TransitionTargetNotFound` | Error | A state's `next` references non-existent state |
| `ChoiceTargetNotFound` | Error | A choice rule's `next` references non-existent state |
| `DefaultTargetNotFound` | Error | A choice state's `default` references non-existent state |
| `NoTerminalState` | Warning/Error | No reachable end -- machine would run forever |

### Runtime errors (out of scope for this contract)

Runtime errors (timeout exceeded, container failure, expression evaluation failure) are NOT part of this contract. They belong to the execution engine.

---

## Cargo.toml Change

The following dependency must be added to `crates/twerk-core/Cargo.toml`:

```toml
indexmap = { version = "2", features = ["serde"] }
```

This enables `IndexMap<StateName, State>` to serialize/deserialize via serde while preserving insertion order.
