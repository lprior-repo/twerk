# Contract Specification: twerk-14f

## ASL Core: TaskState, ParallelState, MapState Container Types

## Context

- **Feature**: Three container/execution state types for the ASL state machine
- **Files**:
  - `crates/twerk-core/src/asl/task_state.rs` -- TaskState: the execution state that runs a container
  - `crates/twerk-core/src/asl/parallel.rs` -- ParallelState: concurrent branch execution
  - `crates/twerk-core/src/asl/map.rs` -- MapState: iterate array with sub-state-machine
- **Domain terms**:
  - **ASL**: Amazon States Language -- JSON/YAML-based language for defining state machines
  - **TaskState**: The primary workhorse state; executes a container image with a shell command, captures output, supports retry/catch
  - **ParallelState**: Runs multiple independent branches (each a full StateMachine) concurrently; optionally fails fast on first branch error
  - **MapState**: Iterates over an array (resolved from an Expression), running a sub-StateMachine per element with optional concurrency limits
  - **StateMachine**: A full sub-state-machine (forward dependency from twerk-bzz); not yet implemented
  - **Transition**: How a state declares what happens after completion -- Next(StateName) or End (from twerk-9xv)
  - **Retrier**: Retry policy matching errors with exponential backoff (from twerk-9xv)
  - **Catcher**: Error catch-and-route policy (from twerk-9xv)
- **Dependency types** (from twerk-fq8, `asl/types.rs` and `asl/error_code.rs`):
  - `ImageRef` -- validated non-empty string, no ASCII whitespace (INV-5)
  - `ShellScript` -- validated non-empty string (INV-6)
  - `Expression` -- validated non-empty string (INV-2)
  - `VariableName` -- validated identifier, 1-128 chars, `[a-zA-Z_][a-zA-Z0-9_]*` (INV-4)
- **Dependency types** (from twerk-9xv):
  - `Transition` -- enum: `Next(StateName)` | `End` (INV-T1, INV-T2)
  - `Retrier` -- retry policy with validated invariants (INV-R1 through INV-R6)
  - `Catcher` -- error catch-and-route with validated invariants (INV-C1 through INV-C4)
- **Forward dependency** (from twerk-bzz):
  - `StateMachine` -- a complete sub-state-machine; type signature TBD. Referenced opaquely in this contract.
- **Assumptions**:
  - All dependency types already exist and are validated at construction (parse-don't-validate)
  - `StateMachine` will be a struct from `crates/twerk-core/src/asl/machine.rs`; this contract references it as an opaque type
  - TaskState, ParallelState, and MapState are constructed via validated constructors returning `Result<Self, Error>`
  - All types are immutable after construction (no setters)
  - Serde deserialization must re-validate all invariants
  - `HashMap<String, Expression>` is used for TaskState.env because env var keys are plain strings (not VariableNames)
  - `env` HashMap keys are arbitrary non-empty strings (container environment variable names)
  - These are data types only -- no execution logic (execution is a separate concern)
- **Open questions**: None

---

## Types

### File: `crates/twerk-core/src/asl/task_state.rs`

#### 1. TaskState

```
struct TaskState {
    image:      ImageRef,
    run:        ShellScript,
    env:        HashMap<String, Expression>,
    var:        Option<VariableName>,
    timeout:    Option<u64>,
    heartbeat:  Option<u64>,
    retry:      Vec<Retrier>,
    catch:      Vec<Catcher>,
    transition: Transition,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | See above |
| Serde | `#[serde(rename_all = "camelCase")]` with `#[serde(flatten)]` on transition |
| Derives | `Debug`, `Clone`, `PartialEq` |

**No `Eq`/`Hash`**: Retrier contains `BackoffRate` (wraps f64).

### File: `crates/twerk-core/src/asl/parallel.rs`

#### 2. ParallelState

```
struct ParallelState {
    branches:   Vec<StateMachine>,
    transition: Transition,
    fail_fast:  Option<bool>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | See above |
| Serde | `#[serde(rename_all = "camelCase")]` with `#[serde(flatten)]` on transition |
| Derives | `Debug`, `Clone`, `PartialEq` |

**No `Eq`/`Hash`**: StateMachine trait bounds are TBD; conservative approach.

### File: `crates/twerk-core/src/asl/map.rs`

#### 3. MapState

```
struct MapState {
    items_path:                   Expression,
    item_processor:               StateMachine,
    max_concurrency:              Option<u32>,
    transition:                   Transition,
    retry:                        Vec<Retrier>,
    catch:                        Vec<Catcher>,
    tolerated_failure_percentage: Option<f64>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | See above |
| Serde | `#[serde(rename_all = "camelCase")]` with `#[serde(flatten)]` on transition |
| Derives | `Debug`, `Clone`, `PartialEq` |

**No `Eq`/`Hash`**: Contains `f64` (tolerated_failure_percentage) and Retrier (BackoffRate wraps f64).

---

## Invariants

These must ALWAYS hold for any instance of the type that exists in memory:

| ID | Type | Invariant |
|----|------|-----------|
| INV-TS1 | `TaskState` | `self.image` satisfies ImageRef invariants (INV-5: non-empty, no whitespace) |
| INV-TS2 | `TaskState` | `self.run` satisfies ShellScript invariants (INV-6: non-empty) |
| INV-TS3 | `TaskState` | Every value in `self.env` satisfies Expression invariants (INV-2: non-empty) |
| INV-TS4 | `TaskState` | Every key in `self.env` is a non-empty string |
| INV-TS5 | `TaskState` | If `self.var` is `Some(v)`, then `v` satisfies VariableName invariants (INV-4) |
| INV-TS6 | `TaskState` | If `self.timeout` is `Some(t)`, then `t >= 1` (at least 1 second) |
| INV-TS7 | `TaskState` | If `self.heartbeat` is `Some(h)`, then `h >= 1` (at least 1 second) |
| INV-TS8 | `TaskState` | If both `self.timeout` and `self.heartbeat` are `Some`, then `heartbeat < timeout` |
| INV-TS9 | `TaskState` | Every Retrier in `self.retry` satisfies INV-R1 through INV-R6 |
| INV-TS10 | `TaskState` | Every Catcher in `self.catch` satisfies INV-C1 through INV-C4 |
| INV-TS11 | `TaskState` | `self.transition` satisfies INV-T1 (valid Transition) |
| INV-PS1 | `ParallelState` | `!self.branches.is_empty()` -- at least one branch required |
| INV-PS2 | `ParallelState` | `self.transition` satisfies INV-T1 (valid Transition) |
| INV-MS1 | `MapState` | `self.items_path` satisfies Expression invariants (INV-2: non-empty) |
| INV-MS2 | `MapState` | `self.transition` satisfies INV-T1 (valid Transition) |
| INV-MS3 | `MapState` | Every Retrier in `self.retry` satisfies INV-R1 through INV-R6 |
| INV-MS4 | `MapState` | Every Catcher in `self.catch` satisfies INV-C1 through INV-C4 |
| INV-MS5 | `MapState` | If `self.tolerated_failure_percentage` is `Some(p)`, then `0.0 <= p && p <= 100.0 && p.is_finite()` |

---

## Defaults

| Type | Field | Default |
|------|-------|---------|
| `TaskState` | `env` | Empty `HashMap` (`HashMap::new()`) |
| `TaskState` | `var` | `None` |
| `TaskState` | `timeout` | `None` |
| `TaskState` | `heartbeat` | `None` |
| `TaskState` | `retry` | Empty `Vec` (`vec![]`) |
| `TaskState` | `catch` | Empty `Vec` (`vec![]`) |
| `ParallelState` | `fail_fast` | `None` (interpreted as `true` by runtime; type layer stores raw value) |
| `MapState` | `max_concurrency` | `None` (interpreted as unlimited by runtime; type layer stores raw value) |
| `MapState` | `retry` | Empty `Vec` (`vec![]`) |
| `MapState` | `catch` | Empty `Vec` (`vec![]`) |
| `MapState` | `tolerated_failure_percentage` | `None` |

---

## Trait Requirements

### TaskState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "RawTaskState")]` to enforce invariants on deserialize |

**NOT** derived: `Eq`, `Hash` (Retrier contains BackoffRate which wraps f64).

### ParallelState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "RawParallelState")]` to enforce invariants on deserialize |

**NOT** derived: `Eq`, `Hash` (conservative; StateMachine bounds TBD).

### MapState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "RawMapState")]` to enforce invariants on deserialize |

**NOT** derived: `Eq`, `Hash` (contains f64 via tolerated_failure_percentage and BackoffRate).

---

## Error Taxonomy

### `TaskStateError`

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TaskStateError {
    #[error("task state timeout must be >= 1 second, got {0}")]
    TimeoutTooSmall(u64),
    #[error("task state heartbeat must be >= 1 second, got {0}")]
    HeartbeatTooSmall(u64),
    #[error("task state heartbeat ({heartbeat}s) must be less than timeout ({timeout}s)")]
    HeartbeatExceedsTimeout { heartbeat: u64, timeout: u64 },
    #[error("task state env key cannot be empty")]
    EmptyEnvKey,
}
```

| Variant | Trigger |
|---------|---------|
| `TimeoutTooSmall(v)` | `timeout` is `Some(0)` |
| `HeartbeatTooSmall(v)` | `heartbeat` is `Some(0)` |
| `HeartbeatExceedsTimeout { .. }` | Both present and `heartbeat >= timeout` |
| `EmptyEnvKey` | Any key in `env` HashMap is empty string |

Note: `TaskStateError` derives `PartialEq` only (no `Eq`) for consistency with types containing f64-derived fields. Errors for invalid `image`, `run`, `var`, `retry`, `catch`, and `transition` are prevented by the type system -- those fields use already-validated NewTypes.

### `ParallelStateError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParallelStateError {
    #[error("parallel state must have at least one branch")]
    EmptyBranches,
}
```

| Variant | Trigger |
|---------|---------|
| `EmptyBranches` | `branches` Vec is empty |

Note: No further variants needed; `transition` and `fail_fast` require no additional validation beyond their types.

### `MapStateError`

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MapStateError {
    #[error("tolerated failure percentage must be 0.0..=100.0, got {0}")]
    InvalidToleratedFailurePercentage(f64),
    #[error("tolerated failure percentage must be finite")]
    NonFiniteToleratedFailurePercentage,
}
```

| Variant | Trigger |
|---------|---------|
| `InvalidToleratedFailurePercentage(v)` | `tolerated_failure_percentage` is `Some(p)` where `p < 0.0` or `p > 100.0` |
| `NonFiniteToleratedFailurePercentage` | `tolerated_failure_percentage` is `Some(p)` where `p.is_nan()` or `p.is_infinite()` |

Note: `MapStateError` derives `PartialEq` only (no `Eq`) because it holds `f64`. Errors for `items_path`, `item_processor`, `retry`, `catch`, and `transition` are prevented by the type system.

---

## Contract Signatures

### TaskState

```rust
impl TaskState {
    /// Validated constructor.
    ///
    /// PRE: image is a valid ImageRef (guaranteed by type)
    /// PRE: run is a valid ShellScript (guaranteed by type)
    /// PRE: all env keys are non-empty strings
    /// PRE: all env values are valid Expressions (guaranteed by type)
    /// PRE: if var is Some, it is a valid VariableName (guaranteed by type)
    /// PRE: if timeout is Some(t), then t >= 1
    /// PRE: if heartbeat is Some(h), then h >= 1
    /// PRE: if both timeout and heartbeat are Some, then heartbeat < timeout
    /// PRE: all Retriers in retry are valid (guaranteed by type)
    /// PRE: all Catchers in catch are valid (guaranteed by type)
    /// PRE: transition is a valid Transition (guaranteed by type)
    /// POST: returned Ok(Self) satisfies INV-TS1 through INV-TS11
    /// ERR: TaskStateError (see error taxonomy)
    pub fn new(
        image: ImageRef,
        run: ShellScript,
        env: HashMap<String, Expression>,
        var: Option<VariableName>,
        timeout: Option<u64>,
        heartbeat: Option<u64>,
        retry: Vec<Retrier>,
        catch: Vec<Catcher>,
        transition: Transition,
    ) -> Result<Self, TaskStateError>;

    /// PRE: self is valid
    /// POST: returned ImageRef satisfies INV-5
    #[must_use]
    pub fn image(&self) -> &ImageRef;

    /// PRE: self is valid
    /// POST: returned ShellScript satisfies INV-6
    #[must_use]
    pub fn run(&self) -> &ShellScript;

    /// PRE: self is valid
    /// POST: returned map has non-empty keys and Expression values satisfying INV-2
    #[must_use]
    pub fn env(&self) -> &HashMap<String, Expression>;

    /// PRE: self is valid
    /// POST: if Some, the VariableName satisfies INV-4
    #[must_use]
    pub fn var(&self) -> Option<&VariableName>;

    /// PRE: self is valid
    /// POST: if Some(t), then t >= 1
    #[must_use]
    pub fn timeout(&self) -> Option<u64>;

    /// PRE: self is valid
    /// POST: if Some(h), then h >= 1
    #[must_use]
    pub fn heartbeat(&self) -> Option<u64>;

    /// PRE: self is valid
    /// POST: returned slice may be empty; every element satisfies INV-R1..INV-R6
    #[must_use]
    pub fn retry(&self) -> &[Retrier];

    /// PRE: self is valid
    /// POST: returned slice may be empty; every element satisfies INV-C1..INV-C4
    #[must_use]
    pub fn catch(&self) -> &[Catcher];

    /// PRE: self is valid
    /// POST: returned Transition satisfies INV-T1
    #[must_use]
    pub fn transition(&self) -> &Transition;
}
```

### ParallelState

```rust
impl ParallelState {
    /// Validated constructor.
    ///
    /// PRE: branches is non-empty
    /// PRE: each StateMachine in branches is valid (guaranteed by type)
    /// PRE: transition is a valid Transition (guaranteed by type)
    /// POST: returned Ok(Self) satisfies INV-PS1 through INV-PS2
    /// ERR: ParallelStateError::EmptyBranches
    pub fn new(
        branches: Vec<StateMachine>,
        transition: Transition,
        fail_fast: Option<bool>,
    ) -> Result<Self, ParallelStateError>;

    /// PRE: self is valid
    /// POST: returned slice is non-empty (INV-PS1)
    #[must_use]
    pub fn branches(&self) -> &[StateMachine];

    /// PRE: self is valid
    /// POST: returned Transition satisfies INV-T1
    #[must_use]
    pub fn transition(&self) -> &Transition;

    /// PRE: self is valid
    /// POST: returns the raw fail_fast value; None means default (true at runtime)
    #[must_use]
    pub fn fail_fast(&self) -> Option<bool>;
}
```

### MapState

```rust
impl MapState {
    /// Validated constructor.
    ///
    /// PRE: items_path is a valid Expression (guaranteed by type)
    /// PRE: item_processor is a valid StateMachine (guaranteed by type)
    /// PRE: transition is a valid Transition (guaranteed by type)
    /// PRE: all Retriers in retry are valid (guaranteed by type)
    /// PRE: all Catchers in catch are valid (guaranteed by type)
    /// PRE: if tolerated_failure_percentage is Some(p), then 0.0 <= p <= 100.0 and p.is_finite()
    /// POST: returned Ok(Self) satisfies INV-MS1 through INV-MS5
    /// ERR: MapStateError (see error taxonomy)
    pub fn new(
        items_path: Expression,
        item_processor: StateMachine,
        max_concurrency: Option<u32>,
        transition: Transition,
        retry: Vec<Retrier>,
        catch: Vec<Catcher>,
        tolerated_failure_percentage: Option<f64>,
    ) -> Result<Self, MapStateError>;

    /// PRE: self is valid
    /// POST: returned Expression satisfies INV-2
    #[must_use]
    pub fn items_path(&self) -> &Expression;

    /// PRE: self is valid
    /// POST: returned StateMachine is valid
    #[must_use]
    pub fn item_processor(&self) -> &StateMachine;

    /// PRE: self is valid
    /// POST: returns raw max_concurrency; None means unlimited at runtime
    #[must_use]
    pub fn max_concurrency(&self) -> Option<u32>;

    /// PRE: self is valid
    /// POST: returned Transition satisfies INV-T1
    #[must_use]
    pub fn transition(&self) -> &Transition;

    /// PRE: self is valid
    /// POST: returned slice may be empty; every element satisfies INV-R1..INV-R6
    #[must_use]
    pub fn retry(&self) -> &[Retrier];

    /// PRE: self is valid
    /// POST: returned slice may be empty; every element satisfies INV-C1..INV-C4
    #[must_use]
    pub fn catch(&self) -> &[Catcher];

    /// PRE: self is valid
    /// POST: if Some(p), then 0.0 <= p <= 100.0 and p.is_finite()
    #[must_use]
    pub fn tolerated_failure_percentage(&self) -> Option<f64>;
}
```

---

## Serde Deserialization Contracts

### TaskState

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `image` -> `"image"` (already camelCase)
  - `run` -> `"run"` (already camelCase)
  - `env` -> `"env"` (already camelCase; `#[serde(default)]` for empty map; `#[serde(skip_serializing_if = "HashMap::is_empty")]`)
  - `var` -> `"var"` (already camelCase; `#[serde(skip_serializing_if = "Option::is_none")]`)
  - `timeout` -> `"timeout"` (already camelCase; `#[serde(skip_serializing_if = "Option::is_none")]`)
  - `heartbeat` -> `"heartbeat"` (already camelCase; `#[serde(skip_serializing_if = "Option::is_none")]`)
  - `retry` -> `"retry"` (already camelCase; `#[serde(default)]` for empty vec; `#[serde(skip_serializing_if = "Vec::is_empty")]`)
  - `catch` -> `"catch"` (already camelCase; `#[serde(default)]` for empty vec; `#[serde(skip_serializing_if = "Vec::is_empty")]`)
  - `transition` -> flattened via `#[serde(flatten)]` into parent: `"next"/"end"` keys appear at same level
- Deserialization uses `#[serde(try_from = "RawTaskState")]` where `RawTaskState` captures raw fields, then validates via `TaskState::new()`
- Env HashMap deserializes as `{"KEY": "value", ...}` -- keys are plain strings, values are Expression-validated
- **Postcondition**: any successfully deserialized TaskState satisfies INV-TS1 through INV-TS11

### ParallelState

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `branches` -> `"branches"` (array of StateMachine objects)
  - `fail_fast` -> `"failFast"` (`#[serde(skip_serializing_if = "Option::is_none")]`)
  - `transition` -> flattened via `#[serde(flatten)]`: `"next"/"end"` keys at same level
- Deserialization uses `#[serde(try_from = "RawParallelState")]` where `RawParallelState` captures raw fields, then validates via `ParallelState::new()`
- `branches` deserializes as a JSON/YAML array; each element is a full StateMachine definition
- A YAML/JSON document with empty `branches` array MUST produce a deserialization error
- **Postcondition**: any successfully deserialized ParallelState satisfies INV-PS1 through INV-PS2

### MapState

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `items_path` -> `"itemsPath"`
  - `item_processor` -> `"itemProcessor"` (nested StateMachine object)
  - `max_concurrency` -> `"maxConcurrency"` (`#[serde(skip_serializing_if = "Option::is_none")]`)
  - `transition` -> flattened via `#[serde(flatten)]`: `"next"/"end"` keys at same level
  - `retry` -> `"retry"` (`#[serde(default)]`; `#[serde(skip_serializing_if = "Vec::is_empty")]`)
  - `catch` -> `"catch"` (`#[serde(default)]`; `#[serde(skip_serializing_if = "Vec::is_empty")]`)
  - `tolerated_failure_percentage` -> `"toleratedFailurePercentage"` (`#[serde(skip_serializing_if = "Option::is_none")]`)
- Deserialization uses `#[serde(try_from = "RawMapState")]` where `RawMapState` captures raw fields, then validates via `MapState::new()`
- A YAML/JSON document with `toleratedFailurePercentage` outside 0.0..=100.0 MUST produce a deserialization error
- **Postcondition**: any successfully deserialized MapState satisfies INV-MS1 through INV-MS5

---

## Non-goals

- No `Default` impl for TaskState, ParallelState, or MapState (no sensible defaults for required fields)
- No `Ord`/`PartialOrd` for any type (ordering is not meaningful)
- No `Hash` for any type (f64 fields prevent it)
- No builder pattern (validated constructor is sufficient for now)
- No execution logic (running containers, spawning branches, iterating maps is runtime concern)
- No validation of `env` key format beyond non-empty (container runtimes accept diverse env var names)
- No semantic validation of `retry`/`catch` ordering (first-match-wins is runtime behaviour)
- No `max_concurrency = 0` special handling at type layer (0 = unlimited is a runtime interpretation)
- `fail_fast` runtime default of `true` is NOT encoded in the type; it stores `Option<bool>` faithfully

---

## Given-When-Then Scenarios

### TaskState

#### Scenario TS-1: Valid construction with all fields

```
Given:
  - image = ImageRef::new("alpine:3.19").unwrap()
  - run = ShellScript::new("echo hello").unwrap()
  - env = HashMap from {"API_KEY" => Expression::new("$.secrets.api_key").unwrap()}
  - var = Some(VariableName::new("output").unwrap())
  - timeout = Some(300)
  - heartbeat = Some(60)
  - retry = vec![valid_retrier]
  - catch = vec![valid_catcher]
  - transition = Transition::next(StateName::new("NextStep").unwrap())
When:  TaskState::new(image, run, env, var, timeout, heartbeat, retry, catch, transition)
Then:
  - Returns Ok(TaskState)
  - image().as_str() == "alpine:3.19"
  - run().as_str() == "echo hello"
  - env().len() == 1
  - env().get("API_KEY") is Some
  - var() returns Some(&VariableName("output"))
  - timeout() returns Some(300)
  - heartbeat() returns Some(60)
  - retry().len() == 1
  - catch().len() == 1
  - transition().is_next() == true
```

#### Scenario TS-2: Valid construction with minimal fields (defaults)

```
Given:
  - image = ImageRef::new("ubuntu:22.04").unwrap()
  - run = ShellScript::new("ls -la").unwrap()
  - env = HashMap::new()
  - var = None
  - timeout = None
  - heartbeat = None
  - retry = vec![]
  - catch = vec![]
  - transition = Transition::end()
When:  TaskState::new(image, run, env, var, timeout, heartbeat, retry, catch, transition)
Then:
  - Returns Ok(TaskState)
  - env().is_empty() == true
  - var() returns None
  - timeout() returns None
  - heartbeat() returns None
  - retry().is_empty() == true
  - catch().is_empty() == true
  - transition().is_end() == true
```

#### Scenario TS-3: Reject timeout = 0

```
Given: timeout = Some(0)
When:  TaskState::new(image, run, env, None, Some(0), None, vec![], vec![], transition)
Then:
  - Returns Err(TaskStateError::TimeoutTooSmall(0))
```

#### Scenario TS-4: Reject heartbeat = 0

```
Given: heartbeat = Some(0)
When:  TaskState::new(image, run, env, None, None, Some(0), vec![], vec![], transition)
Then:
  - Returns Err(TaskStateError::HeartbeatTooSmall(0))
```

#### Scenario TS-5: Reject heartbeat >= timeout

```
Given: timeout = Some(60), heartbeat = Some(60)
When:  TaskState::new(image, run, env, None, Some(60), Some(60), vec![], vec![], transition)
Then:
  - Returns Err(TaskStateError::HeartbeatExceedsTimeout { heartbeat: 60, timeout: 60 })
```

#### Scenario TS-6: Reject heartbeat > timeout

```
Given: timeout = Some(30), heartbeat = Some(60)
When:  TaskState::new(image, run, env, None, Some(30), Some(60), vec![], vec![], transition)
Then:
  - Returns Err(TaskStateError::HeartbeatExceedsTimeout { heartbeat: 60, timeout: 30 })
```

#### Scenario TS-7: Allow heartbeat without timeout

```
Given: timeout = None, heartbeat = Some(30)
When:  TaskState::new(image, run, env, None, None, Some(30), vec![], vec![], transition)
Then:
  - Returns Ok(TaskState)
  - heartbeat() returns Some(30)
  - timeout() returns None
```

#### Scenario TS-8: Reject empty env key

```
Given: env = HashMap from {"" => Expression::new("value").unwrap()}
When:  TaskState::new(image, run, env, None, None, None, vec![], vec![], transition)
Then:
  - Returns Err(TaskStateError::EmptyEnvKey)
```

#### Scenario TS-9: Boundary -- timeout = 1 (minimum)

```
Given: timeout = Some(1)
When:  TaskState::new(image, run, env, None, Some(1), None, vec![], vec![], transition)
Then:
  - Returns Ok(TaskState)
  - timeout() returns Some(1)
```

#### Scenario TS-10: Boundary -- heartbeat just below timeout

```
Given: timeout = Some(10), heartbeat = Some(9)
When:  TaskState::new(image, run, env, None, Some(10), Some(9), vec![], vec![], transition)
Then:
  - Returns Ok(TaskState)
  - heartbeat() returns Some(9)
  - timeout() returns Some(10)
```

#### Scenario TS-11: Serde roundtrip (JSON) -- full

```
Given: a valid TaskState with:
  - image = "alpine:3.19"
  - run = "echo hello"
  - env = {"API_KEY": "$.secrets.key"}
  - var = Some("output")
  - timeout = Some(300)
  - heartbeat = Some(60)
  - retry = [valid_retrier]
  - catch = [valid_catcher]
  - transition = Transition::Next("NextStep")
When:  serde_json::to_string(&ts) is called
Then:
  - Produces JSON with camelCase keys:
    {
      "image": "alpine:3.19",
      "run": "echo hello",
      "env": {"API_KEY": "$.secrets.key"},
      "var": "output",
      "timeout": 300,
      "heartbeat": 60,
      "retry": [...],
      "catch": [...],
      "next": "NextStep"
    }
  - Note: transition is flattened; "next" appears at root level
When:  serde_json::from_str::<TaskState>(json) is called on the result
Then:
  - Returns Ok(ts) where ts == original
```

#### Scenario TS-12: Serde omits default/empty fields

```
Given: a valid TaskState with env = {}, var = None, timeout = None, heartbeat = None, retry = [], catch = []
When:  serde_json::to_string(&ts) is called
Then:
  - JSON output does NOT contain "env", "var", "timeout", "heartbeat", "retry", or "catch" keys
  - JSON output DOES contain "image", "run", and transition fields ("next" or "end")
```

#### Scenario TS-13: Serde deserialize with defaults

```
Given: JSON:
  {"image": "alpine:latest", "run": "ls", "end": true}
When:  serde_json::from_str::<TaskState>(json) is called
Then:
  - Returns Ok(TaskState)
  - env().is_empty() == true
  - var() is None
  - timeout() is None
  - heartbeat() is None
  - retry().is_empty() == true
  - catch().is_empty() == true
  - transition().is_end() == true
```

#### Scenario TS-14: Serde rejects heartbeat >= timeout on deserialize

```
Given: JSON:
  {"image": "alpine:latest", "run": "ls", "timeout": 10, "heartbeat": 10, "end": true}
When:  serde_json::from_str::<TaskState>(json) is called
Then:
  - Returns Err (deserialization error about heartbeat exceeding timeout)
```

#### Scenario TS-15: Serde env map with multiple entries

```
Given: JSON:
  {
    "image": "node:20",
    "run": "npm test",
    "env": {"NODE_ENV": "test", "CI": "true"},
    "end": true
  }
When:  serde_json::from_str::<TaskState>(json) is called
Then:
  - Returns Ok(TaskState)
  - env().len() == 2
  - env().get("NODE_ENV") == Some(&Expression("test"))
  - env().get("CI") == Some(&Expression("true"))
```

#### Scenario TS-16: YAML deserialization

```
Given: YAML string:
  image: alpine:3.19
  run: echo hello
  env:
    GREETING: hello
  timeout: 120
  next: ProcessResult
When:  serde_yaml::from_str::<TaskState>(yaml) is called
Then:
  - Returns Ok(TaskState)
  - image().as_str() == "alpine:3.19"
  - env().get("GREETING") == Some(&Expression("hello"))
  - timeout() returns Some(120)
  - transition() == Transition::Next(StateName("ProcessResult"))
```

---

### ParallelState

#### Scenario PS-1: Valid construction with multiple branches

```
Given:
  - branches = vec![state_machine_a, state_machine_b]
  - transition = Transition::next(StateName::new("MergeResults").unwrap())
  - fail_fast = Some(true)
When:  ParallelState::new(branches, transition, fail_fast)
Then:
  - Returns Ok(ParallelState)
  - branches().len() == 2
  - transition().is_next() == true
  - fail_fast() returns Some(true)
```

#### Scenario PS-2: Valid construction with single branch

```
Given:
  - branches = vec![state_machine_a]
  - transition = Transition::end()
  - fail_fast = None
When:  ParallelState::new(branches, transition, None)
Then:
  - Returns Ok(ParallelState)
  - branches().len() == 1
  - fail_fast() returns None
```

#### Scenario PS-3: Reject empty branches

```
Given: branches = vec![]
When:  ParallelState::new(vec![], transition, None)
Then:
  - Returns Err(ParallelStateError::EmptyBranches)
```

#### Scenario PS-4: fail_fast explicit false

```
Given:
  - branches = vec![state_machine_a, state_machine_b]
  - fail_fast = Some(false)
When:  ParallelState::new(branches, transition, Some(false))
Then:
  - Returns Ok(ParallelState)
  - fail_fast() returns Some(false)
```

#### Scenario PS-5: Serde roundtrip (JSON)

```
Given: a valid ParallelState with:
  - branches = [machine_a, machine_b]
  - fail_fast = Some(true)
  - transition = Transition::Next("Merge")
When:  serde_json::to_string(&ps) is called
Then:
  - Produces JSON:
    {
      "branches": [{...}, {...}],
      "failFast": true,
      "next": "Merge"
    }
  - Note: transition is flattened; "next" appears at root level
When:  serde_json::from_str::<ParallelState>(json) is called on the result
Then:
  - Returns Ok(ps) where ps == original
```

#### Scenario PS-6: Serde omits None failFast

```
Given: a valid ParallelState with fail_fast = None
When:  serde_json::to_string(&ps) is called
Then:
  - JSON output does NOT contain "failFast" key
```

#### Scenario PS-7: Serde rejects empty branches on deserialize

```
Given: JSON:
  {"branches": [], "end": true}
When:  serde_json::from_str::<ParallelState>(json) is called
Then:
  - Returns Err (deserialization error about empty branches)
```

#### Scenario PS-8: YAML deserialization

```
Given: YAML string:
  branches:
    - startAt: Step1
      states:
        Step1:
          type: Task
          ...
    - startAt: Step2
      states:
        Step2:
          type: Task
          ...
  failFast: false
  next: CollectResults
When:  serde_yaml::from_str::<ParallelState>(yaml) is called
Then:
  - Returns Ok(ParallelState)
  - branches().len() == 2
  - fail_fast() returns Some(false)
```

---

### MapState

#### Scenario MS-1: Valid construction with all fields

```
Given:
  - items_path = Expression::new("$.items").unwrap()
  - item_processor = valid_state_machine
  - max_concurrency = Some(10)
  - transition = Transition::next(StateName::new("Aggregate").unwrap())
  - retry = vec![valid_retrier]
  - catch = vec![valid_catcher]
  - tolerated_failure_percentage = Some(25.0)
When:  MapState::new(items_path, item_processor, max_concurrency, transition, retry, catch, tolerated_failure_percentage)
Then:
  - Returns Ok(MapState)
  - items_path().as_str() == "$.items"
  - max_concurrency() returns Some(10)
  - transition().is_next() == true
  - retry().len() == 1
  - catch().len() == 1
  - tolerated_failure_percentage() returns Some(25.0)
```

#### Scenario MS-2: Valid construction with minimal fields

```
Given:
  - items_path = Expression::new("$.data").unwrap()
  - item_processor = valid_state_machine
  - max_concurrency = None
  - transition = Transition::end()
  - retry = vec![]
  - catch = vec![]
  - tolerated_failure_percentage = None
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], None)
Then:
  - Returns Ok(MapState)
  - max_concurrency() returns None
  - retry().is_empty() == true
  - catch().is_empty() == true
  - tolerated_failure_percentage() returns None
```

#### Scenario MS-3: Reject tolerated_failure_percentage < 0.0

```
Given: tolerated_failure_percentage = Some(-1.0)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(-1.0))
Then:
  - Returns Err(MapStateError::InvalidToleratedFailurePercentage(-1.0))
```

#### Scenario MS-4: Reject tolerated_failure_percentage > 100.0

```
Given: tolerated_failure_percentage = Some(100.1)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(100.1))
Then:
  - Returns Err(MapStateError::InvalidToleratedFailurePercentage(100.1))
```

#### Scenario MS-5: Reject tolerated_failure_percentage = NaN

```
Given: tolerated_failure_percentage = Some(f64::NAN)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(f64::NAN))
Then:
  - Returns Err(MapStateError::NonFiniteToleratedFailurePercentage)
```

#### Scenario MS-6: Reject tolerated_failure_percentage = infinity

```
Given: tolerated_failure_percentage = Some(f64::INFINITY)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(f64::INFINITY))
Then:
  - Returns Err(MapStateError::NonFiniteToleratedFailurePercentage)
```

#### Scenario MS-7: Boundary -- tolerated_failure_percentage = 0.0

```
Given: tolerated_failure_percentage = Some(0.0)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(0.0))
Then:
  - Returns Ok(MapState)
  - tolerated_failure_percentage() returns Some(0.0)
```

#### Scenario MS-8: Boundary -- tolerated_failure_percentage = 100.0

```
Given: tolerated_failure_percentage = Some(100.0)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(100.0))
Then:
  - Returns Ok(MapState)
  - tolerated_failure_percentage() returns Some(100.0)
```

#### Scenario MS-9: max_concurrency = 0 is accepted (unlimited)

```
Given: max_concurrency = Some(0)
When:  MapState::new(items_path, item_processor, Some(0), transition, vec![], vec![], None)
Then:
  - Returns Ok(MapState)
  - max_concurrency() returns Some(0)
```

#### Scenario MS-10: Serde roundtrip (JSON)

```
Given: a valid MapState with:
  - items_path = "$.items"
  - item_processor = valid_state_machine
  - max_concurrency = Some(5)
  - transition = Transition::Next("Done")
  - retry = [valid_retrier]
  - catch = [valid_catcher]
  - tolerated_failure_percentage = Some(10.0)
When:  serde_json::to_string(&ms) is called
Then:
  - Produces JSON with camelCase keys:
    {
      "itemsPath": "$.items",
      "itemProcessor": {...},
      "maxConcurrency": 5,
      "retry": [...],
      "catch": [...],
      "toleratedFailurePercentage": 10.0,
      "next": "Done"
    }
  - Note: transition is flattened; "next" appears at root level
When:  serde_json::from_str::<MapState>(json) is called on the result
Then:
  - Returns Ok(ms) where ms == original
```

#### Scenario MS-11: Serde omits default/None fields

```
Given: a valid MapState with max_concurrency = None, retry = [], catch = [], tolerated_failure_percentage = None
When:  serde_json::to_string(&ms) is called
Then:
  - JSON output does NOT contain "maxConcurrency", "retry", "catch", or "toleratedFailurePercentage" keys
  - JSON output DOES contain "itemsPath", "itemProcessor", and transition fields
```

#### Scenario MS-12: Serde rejects invalid tolerated_failure_percentage on deserialize

```
Given: JSON:
  {"itemsPath": "$.items", "itemProcessor": {...}, "toleratedFailurePercentage": 150.0, "end": true}
When:  serde_json::from_str::<MapState>(json) is called
Then:
  - Returns Err (deserialization error about tolerated failure percentage)
```

#### Scenario MS-13: YAML deserialization

```
Given: YAML string:
  itemsPath: "$.orders"
  itemProcessor:
    startAt: Process
    states:
      Process:
        type: Task
        ...
  maxConcurrency: 3
  toleratedFailurePercentage: 5.0
  next: Summary
When:  serde_yaml::from_str::<MapState>(yaml) is called
Then:
  - Returns Ok(MapState)
  - items_path().as_str() == "$.orders"
  - max_concurrency() returns Some(3)
  - tolerated_failure_percentage() returns Some(5.0)
```

#### Scenario MS-14: Reject negative infinity tolerated_failure_percentage

```
Given: tolerated_failure_percentage = Some(f64::NEG_INFINITY)
When:  MapState::new(items_path, item_processor, None, transition, vec![], vec![], Some(f64::NEG_INFINITY))
Then:
  - Returns Err(MapStateError::NonFiniteToleratedFailurePercentage)
```
