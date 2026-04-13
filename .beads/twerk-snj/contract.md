# Contract Specification: twerk-snj

## ASL Core: State Variant Types (Choice, Wait, Pass, Terminal)

## Context

- **Feature**: Create four ASL state variant types that compose the validated NewTypes from twerk-fq8 and Transition from twerk-9xv
- **Files**:
  - `crates/twerk-core/src/asl/choice.rs` -- ChoiceRule struct + ChoiceState struct
  - `crates/twerk-core/src/asl/wait.rs` -- WaitDuration enum + WaitState struct
  - `crates/twerk-core/src/asl/pass.rs` -- PassState struct
  - `crates/twerk-core/src/asl/terminal.rs` -- SucceedState + FailState structs
- **Domain terms**:
  - **ASL**: Amazon States Language -- JSON/YAML-based language for defining state machines
  - **Choice state**: Branching state that evaluates rules against input and routes to the first matching rule's target; has an optional default fallback
  - **ChoiceRule**: A single branch in a Choice state -- pairs a boolean Expression condition with a target StateName
  - **Wait state**: Pauses execution for a fixed or dynamically-resolved duration before transitioning
  - **WaitDuration**: Mutually exclusive specification of how long to wait (fixed seconds, fixed timestamp, or dynamic path variants)
  - **Pass state**: No-op state that optionally injects a result value, then transitions
  - **Succeed state**: Terminal state indicating successful execution; no transition, no fields
  - **Fail state**: Terminal state indicating failed execution; carries optional error name and cause description
  - **Terminal state**: A state with no outgoing transition -- execution ends here
- **Dependency types** (from twerk-fq8, `asl/types.rs`):
  - `StateName` -- validated 1-256 char string (INV-1: non-empty, <= 256 chars)
  - `Expression` -- validated non-empty string (INV-2)
  - `JsonPath` -- validated non-empty string starting with `$` (INV-3)
  - `VariableName` -- validated identifier, 1-128 chars, `[a-zA-Z_][a-zA-Z0-9_]*` (INV-4)
- **Dependency types** (from twerk-9xv, `asl/transition.rs`):
  - `Transition` -- enum: `Next(StateName)` | `End` with custom serde (INV-T1: exactly one variant)
- **Assumptions**:
  - All dependency types already exist and are validated at construction (parse-don't-validate)
  - ChoiceState and WaitState use validated constructors returning `Result<Self, Error>`
  - PassState, SucceedState, and FailState do not need fallible constructors (no domain invariants beyond what the type system provides)
  - All types are immutable after construction (no setters)
  - Serde deserialization must re-validate all invariants
  - `HashMap<VariableName, Expression>` in ChoiceRule.assign requires VariableName to impl Hash (it does, per twerk-fq8 contract)
  - ChoiceState has no Transition field -- routing is entirely through choices and default
  - SucceedState and FailState have no Transition field -- they are terminal
- **Open questions**: None

---

## Types

### File: `crates/twerk-core/src/asl/choice.rs`

#### 1. ChoiceRule

```
struct ChoiceRule {
    condition: Expression,
    next:      StateName,
    assign:    Option<HashMap<VariableName, Expression>>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `condition: Expression`, `next: StateName`, `assign: Option<HashMap<VariableName, Expression>>` |
| Serde | `#[serde(rename_all = "camelCase")]` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq` |

#### 2. ChoiceState

```
struct ChoiceState {
    choices: Vec<ChoiceRule>,
    default: Option<StateName>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `choices: Vec<ChoiceRule>` (min 1), `default: Option<StateName>` |
| Serde | `#[serde(rename_all = "camelCase")]`, custom deserialization via `#[serde(try_from = "RawChoiceState")]` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq` |

### File: `crates/twerk-core/src/asl/wait.rs`

#### 3. WaitDuration

```
enum WaitDuration {
    Seconds(u64),
    Timestamp(String),
    SecondsPath(JsonPath),
    TimestampPath(JsonPath),
}
```

| Attribute | Value |
|-----------|-------|
| Variants | `Seconds(u64)`, `Timestamp(String)`, `SecondsPath(JsonPath)`, `TimestampPath(JsonPath)` |
| Serde | Custom Serialize and Deserialize -- exactly one of four mutually exclusive fields |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` |

#### 4. WaitState

```
struct WaitState {
    duration:   WaitDuration,
    transition: Transition,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `duration: WaitDuration`, `transition: Transition` |
| Serde | Custom deserialization: flattens WaitDuration fields and Transition fields into a single map |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq` |

### File: `crates/twerk-core/src/asl/pass.rs`

#### 5. PassState

```
struct PassState {
    result:     Option<serde_json::Value>,
    transition: Transition,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `result: Option<serde_json::Value>`, `transition: Transition` |
| Serde | `result` serialized with `#[serde(skip_serializing_if = "Option::is_none")]`; `transition` flattened via `#[serde(flatten)]` |
| Derives | `Debug`, `Clone`, `PartialEq` |
| Note | No `Eq` because `serde_json::Value` wraps floats (f64) which do not impl `Eq` |

### File: `crates/twerk-core/src/asl/terminal.rs`

#### 6. SucceedState

```
struct SucceedState;
```

| Attribute | Value |
|-----------|-------|
| Fields | None (unit struct) |
| Serde | Serializes as empty map `{}`; deserializes from empty map or map with no recognized fields |
| Derives | `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Default` |

#### 7. FailState

```
struct FailState {
    error: Option<String>,
    cause: Option<String>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | `error: Option<String>`, `cause: Option<String>` |
| Serde | `#[serde(rename_all = "camelCase")]`, both fields with `#[serde(skip_serializing_if = "Option::is_none")]` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` |

---

## Invariants

These must ALWAYS hold for any instance of the type that exists in memory:

| ID | Type | Invariant |
|----|------|-----------|
| INV-CR1 | `ChoiceRule` | `self.condition` satisfies Expression invariants (INV-2: non-empty) |
| INV-CR2 | `ChoiceRule` | `self.next` satisfies StateName invariants (INV-1: non-empty, <= 256 chars) |
| INV-CR3 | `ChoiceRule` | If `self.assign` is `Some(map)`, then every key satisfies VariableName invariants (INV-4) and every value satisfies Expression invariants (INV-2) |
| INV-CS1 | `ChoiceState` | `!self.choices.is_empty()` -- at least one choice rule required |
| INV-CS2 | `ChoiceState` | Every element in `self.choices` satisfies INV-CR1, INV-CR2, INV-CR3 |
| INV-CS3 | `ChoiceState` | If `self.default` is `Some(name)`, then `name` satisfies StateName invariants (INV-1) |
| INV-WD1 | `WaitDuration` | Exactly one variant is active (enforced by enum representation) |
| INV-WD2 | `WaitDuration::Timestamp` | Inner string is a non-empty ISO 8601 timestamp |
| INV-WD3 | `WaitDuration::SecondsPath` | Inner `JsonPath` satisfies INV-3 (non-empty, starts with `$`) |
| INV-WD4 | `WaitDuration::TimestampPath` | Inner `JsonPath` satisfies INV-3 (non-empty, starts with `$`) |
| INV-WS1 | `WaitState` | `self.duration` satisfies INV-WD1 through INV-WD4 |
| INV-WS2 | `WaitState` | `self.transition` satisfies Transition invariants (INV-T1) |
| INV-PS1 | `PassState` | `self.transition` satisfies Transition invariants (INV-T1) |
| INV-SS1 | `SucceedState` | No fields -- invariant trivially holds. Always terminal (no transition). |
| INV-FS1 | `FailState` | Always terminal (no transition field). Error and cause are optional free-form strings. |

---

## Trait Requirements

### ChoiceRule

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Derive with `#[serde(rename_all = "camelCase")]` |

### ChoiceState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "RawChoiceState")]` to enforce INV-CS1 |

### WaitDuration

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Custom impl: serializes as a map with exactly one field |
| `Deserialize` | Custom impl: reads map, enforces exactly one of four fields |
| `Display` | Manual impl: e.g. `"seconds: 10"`, `"timestamp: 2024-..."`, `"seconds_path: $.delay"`, `"timestamp_path: $.when"` |

### WaitState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Serialize` | Custom impl: flattens duration fields and transition fields into one map |
| `Deserialize` | Custom impl: reads combined map, extracts duration and transition |

### PassState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `result` skipped if None, `transition` flattened |
| `Deserialize` | Derive with `transition` flattened |

**NOT** derived for PassState: `Eq` (contains `serde_json::Value` which wraps f64).

### SucceedState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `Copy` | Derive (unit struct) |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Default` | Derive |
| `Serialize` | Derive (serializes as empty map `{}`) |
| `Deserialize` | Derive (deserializes from empty map) |

### FailState

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]`, both fields skip if None |
| `Deserialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Display` | Manual impl: formats error/cause for human-readable output |

---

## Error Taxonomy

### `ChoiceStateError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChoiceStateError {
    #[error("choice state must have at least one choice rule")]
    EmptyChoices,
}
```

| Variant | Trigger |
|---------|---------|
| `EmptyChoices` | `choices` vec is empty at construction or deserialization |

Note: `ChoiceStateError` does not need variants for invalid `condition`, `next`, or `assign` because those fields use already-validated NewTypes. If they arrive via serde, the inner types' own deserialization validates them before ChoiceState construction.

### `WaitDurationError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WaitDurationError {
    #[error("wait duration must specify exactly one of: seconds, timestamp, seconds_path, timestamp_path")]
    NoFieldSpecified,
    #[error("wait duration has multiple fields set: {fields:?}")]
    MultipleFieldsSpecified { fields: Vec<String> },
    #[error("wait timestamp cannot be empty")]
    EmptyTimestamp,
}
```

| Variant | Trigger |
|---------|---------|
| `NoFieldSpecified` | Deserializing a map that contains none of the four duration fields |
| `MultipleFieldsSpecified { fields }` | Deserializing a map that contains more than one of the four duration fields |
| `EmptyTimestamp` | `timestamp` field is present but is an empty string |

Note: `SecondsPath` and `TimestampPath` use `JsonPath` which validates itself at deserialization (non-empty, starts with `$`). No separate error variant needed.

### No error types for PassState, SucceedState, FailState

- `PassState` has no domain invariants requiring a validated constructor. The `Transition` field validates itself via its own serde. The `result` field is any arbitrary JSON value.
- `SucceedState` is a unit struct. No construction can fail.
- `FailState` has only optional free-form strings. No invariants to validate.

---

## Contract Signatures

### ChoiceRule

```rust
impl ChoiceRule {
    /// Direct constructor. All fields are pre-validated types.
    ///
    /// PRE: condition is a valid Expression (guaranteed by type)
    /// PRE: next is a valid StateName (guaranteed by type)
    /// PRE: assign, if Some, has valid VariableName keys and Expression values (guaranteed by types)
    /// POST: returned Self satisfies INV-CR1 through INV-CR3
    #[must_use]
    pub fn new(
        condition: Expression,
        next: StateName,
        assign: Option<HashMap<VariableName, Expression>>,
    ) -> Self;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned Expression satisfies INV-2
    #[must_use]
    pub fn condition(&self) -> &Expression;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned StateName satisfies INV-1
    #[must_use]
    pub fn next(&self) -> &StateName;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: if Some, all keys satisfy INV-4 and all values satisfy INV-2
    #[must_use]
    pub fn assign(&self) -> Option<&HashMap<VariableName, Expression>>;
}
```

### ChoiceState

```rust
impl ChoiceState {
    /// Validated constructor.
    ///
    /// PRE: choices is non-empty
    /// PRE: each ChoiceRule in choices satisfies INV-CR1 through INV-CR3 (guaranteed by type)
    /// PRE: default, if Some, is a valid StateName (guaranteed by type)
    /// POST: returned Ok(Self) satisfies INV-CS1 through INV-CS3
    /// ERR: ChoiceStateError::EmptyChoices
    pub fn new(
        choices: Vec<ChoiceRule>,
        default: Option<StateName>,
    ) -> Result<Self, ChoiceStateError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice is non-empty
    #[must_use]
    pub fn choices(&self) -> &[ChoiceRule];

    /// PRE: self is valid (guaranteed by construction)
    /// POST: if Some, the StateName satisfies INV-1
    #[must_use]
    pub fn default(&self) -> Option<&StateName>;
}
```

### WaitDuration

```rust
impl WaitDuration {
    /// Returns true if this is a fixed-seconds duration.
    #[must_use]
    pub fn is_seconds(&self) -> bool;

    /// Returns true if this is a fixed-timestamp duration.
    #[must_use]
    pub fn is_timestamp(&self) -> bool;

    /// Returns true if this is a dynamic seconds-path duration.
    #[must_use]
    pub fn is_seconds_path(&self) -> bool;

    /// Returns true if this is a dynamic timestamp-path duration.
    #[must_use]
    pub fn is_timestamp_path(&self) -> bool;
}
```

### WaitDuration Serde (Custom Deserializer)

```rust
/// Custom deserializer for WaitDuration.
///
/// Accepts exactly one of four mutually exclusive fields:
///   { "seconds": 10 }              -> WaitDuration::Seconds(10)
///   { "timestamp": "2024-..." }    -> WaitDuration::Timestamp("2024-...")
///   { "seconds_path": "$.delay" }  -> WaitDuration::SecondsPath(JsonPath("$.delay"))
///   { "timestamp_path": "$.when" } -> WaitDuration::TimestampPath(JsonPath("$.when"))
///
/// Field names use snake_case in the serialized form (ASL convention for Wait).
///
/// PRE: Input is a YAML/JSON map
/// POST on success: WaitDuration value satisfying INV-WD1
/// ERR: WaitDurationError::NoFieldSpecified if none of the four fields present
/// ERR: WaitDurationError::MultipleFieldsSpecified if more than one field present
/// ERR: WaitDurationError::EmptyTimestamp if timestamp field is empty string
/// ERR: JsonPathError (via inner type) if seconds_path or timestamp_path fail JsonPath validation
impl<'de> Deserialize<'de> for WaitDuration { ... }

/// Custom serializer for WaitDuration.
///
/// WaitDuration::Seconds(n)         -> { "seconds": n }
/// WaitDuration::Timestamp(t)       -> { "timestamp": "t" }
/// WaitDuration::SecondsPath(p)     -> { "seconds_path": "p" }
/// WaitDuration::TimestampPath(p)   -> { "timestamp_path": "p" }
impl Serialize for WaitDuration { ... }
```

### WaitState

```rust
impl WaitState {
    /// Direct constructor. Duration and transition are pre-validated types.
    ///
    /// PRE: duration satisfies INV-WD1 through INV-WD4 (guaranteed by type)
    /// PRE: transition satisfies INV-T1 (guaranteed by type)
    /// POST: returned Self satisfies INV-WS1 and INV-WS2
    #[must_use]
    pub fn new(duration: WaitDuration, transition: Transition) -> Self;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned WaitDuration satisfies INV-WD1
    #[must_use]
    pub fn duration(&self) -> &WaitDuration;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned Transition satisfies INV-T1
    #[must_use]
    pub fn transition(&self) -> &Transition;
}
```

### WaitState Serde

```rust
/// Custom serializer/deserializer for WaitState.
///
/// Flattens WaitDuration fields and Transition fields into a single map:
///   { "seconds": 10, "next": "Step2" }
///   { "timestamp": "2024-01-01T00:00:00Z", "end": true }
///   { "seconds_path": "$.delay", "next": "ProcessResult" }
///
/// Deserialization:
///   1. Reads the full map
///   2. Extracts and validates duration (exactly one of four fields)
///   3. Extracts and validates transition (next/end, mutually exclusive)
///   4. Rejects unknown fields (optional, strict mode)
///
/// POST on success: WaitState satisfying INV-WS1 and INV-WS2
/// ERR: WaitDurationError if duration fields are invalid
/// ERR: TransitionError if transition fields are invalid
```

### PassState

```rust
impl PassState {
    /// Direct constructor. Transition is a pre-validated type.
    ///
    /// PRE: transition satisfies INV-T1 (guaranteed by type)
    /// POST: returned Self satisfies INV-PS1
    #[must_use]
    pub fn new(result: Option<serde_json::Value>, transition: Transition) -> Self;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns the injected result, if any
    #[must_use]
    pub fn result(&self) -> Option<&serde_json::Value>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned Transition satisfies INV-T1
    #[must_use]
    pub fn transition(&self) -> &Transition;
}
```

### SucceedState

```rust
impl SucceedState {
    /// Unit constructor. Always succeeds.
    ///
    /// PRE: none
    /// POST: returned Self satisfies INV-SS1
    #[must_use]
    pub fn new() -> Self;
}
```

### FailState

```rust
impl FailState {
    /// Constructor with optional error and cause.
    ///
    /// PRE: none (all fields optional, free-form strings)
    /// POST: returned Self satisfies INV-FS1
    #[must_use]
    pub fn new(error: Option<String>, cause: Option<String>) -> Self;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns the error name, if any
    #[must_use]
    pub fn error(&self) -> Option<&str>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns the cause description, if any
    #[must_use]
    pub fn cause(&self) -> Option<&str>;
}

impl fmt::Display for FailState {
    /// Formats the FailState for human-readable output.
    ///
    /// POST: if both error and cause: "FAIL: {error} ({cause})"
    /// POST: if only error: "FAIL: {error}"
    /// POST: if only cause: "FAIL: ({cause})"
    /// POST: if neither: "FAIL"
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}
```

---

## Serde Deserialization Contracts

### ChoiceRule

- Derives `Serialize` and `Deserialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `condition` -> `"condition"`
  - `next` -> `"next"`
  - `assign` -> `"assign"` (skipped if None via `#[serde(skip_serializing_if = "Option::is_none")]`)
- Inner types (`Expression`, `StateName`, `VariableName`) validate themselves during deserialization
- **Postcondition**: any successfully deserialized ChoiceRule satisfies INV-CR1 through INV-CR3

### ChoiceState

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `choices` -> `"choices"`
  - `default` -> `"default"` (skipped if None via `#[serde(skip_serializing_if = "Option::is_none")]`)
- Deserialization uses `#[serde(try_from = "RawChoiceState")]` where `RawChoiceState` captures raw fields, then validates `choices` non-empty via `ChoiceState::new()`
- A YAML/JSON document with empty `choices` array MUST produce a deserialization error
- **Postcondition**: any successfully deserialized ChoiceState satisfies INV-CS1 through INV-CS3

### WaitDuration

- **Custom Serialize and Deserialize** (NOT derived)
- Serialization:
  - `Seconds(n)` -> `{"seconds": n}`
  - `Timestamp(t)` -> `{"timestamp": "t"}`
  - `SecondsPath(p)` -> `{"seconds_path": "p.as_str()"}`
  - `TimestampPath(p)` -> `{"timestamp_path": "p.as_str()"}`
- Deserialization reads a map and expects exactly one of:
  - `seconds` key with u64 value -> `WaitDuration::Seconds`
  - `timestamp` key with non-empty string value -> `WaitDuration::Timestamp`
  - `seconds_path` key with string value -> validates as `JsonPath`, produces `WaitDuration::SecondsPath`
  - `timestamp_path` key with string value -> validates as `JsonPath`, produces `WaitDuration::TimestampPath`
- Errors if zero fields present, more than one present, or inner type validation fails
- **Postcondition**: any successfully deserialized WaitDuration satisfies INV-WD1 through INV-WD4

### WaitState

- **Custom Serialize and Deserialize** (NOT derived)
- All fields from both WaitDuration and Transition are flattened into a single map
- Serialization example: `{"seconds": 10, "next": "Step2"}` or `{"timestamp_path": "$.when", "end": true}`
- Deserialization:
  1. Reads the combined flat map
  2. Partitions fields into duration fields (`seconds`, `timestamp`, `seconds_path`, `timestamp_path`) and transition fields (`next`, `end`)
  3. Validates each group independently using WaitDuration and Transition deserializers
  4. Constructs WaitState from the two validated components
- Errors propagate from inner WaitDuration or Transition deserialization
- **Postcondition**: any successfully deserialized WaitState satisfies INV-WS1 and INV-WS2

### PassState

- Derives `Serialize` and `Deserialize`
- Field mapping:
  - `result` -> `"result"` (skipped if None via `#[serde(skip_serializing_if = "Option::is_none")]`)
  - `transition` -> flattened via `#[serde(flatten)]` (produces `next`/`end` keys at same level)
- Serialization example: `{"result": {"key": "value"}, "next": "Step2"}` or `{"end": true}`
- Transition validates itself via its own custom deserializer
- **Postcondition**: any successfully deserialized PassState satisfies INV-PS1

### SucceedState

- Derives `Serialize` and `Deserialize`
- Serializes as `{}` (empty map)
- Deserializes from an empty map (or a map with no recognized fields)
- **Postcondition**: any successfully deserialized SucceedState satisfies INV-SS1

### FailState

- Derives `Serialize` and `Deserialize` with `#[serde(rename_all = "camelCase")]`
- Field mapping:
  - `error` -> `"error"` (skipped if None)
  - `cause` -> `"cause"` (skipped if None)
- Serialization examples: `{}`, `{"error": "MyError"}`, `{"error": "MyError", "cause": "something broke"}`, `{"cause": "unknown"}`
- **Postcondition**: any successfully deserialized FailState satisfies INV-FS1

---

## Non-goals

- No `Default` impl for ChoiceState, WaitState, or PassState (no sensible defaults for these domain types)
- No `Ord`/`PartialOrd` for any type (ordering is not meaningful in ASL context)
- No runtime evaluation of Choice conditions or Wait durations (that is execution-layer concern)
- No `Hash` for PassState (contains `serde_json::Value` which wraps f64)
- No builder pattern (validated constructors are sufficient; fields are few and explicit)
- No ISO 8601 parsing or validation for `WaitDuration::Timestamp` beyond non-empty (parsing is execution-layer)
- FailState does not validate error/cause content (free-form strings per ASL spec)
- SucceedState does not need Display (unit struct with no meaningful content)

---

## Given-When-Then Scenarios

### ChoiceRule

#### Scenario CR-1: Construct valid ChoiceRule without assign

```
Given: a valid Expression "$.value > 10" and a valid StateName "HighValuePath"
When:  ChoiceRule::new(condition, next, None) is called
Then:
  - Returns a ChoiceRule
  - condition() returns &Expression("$.value > 10")
  - next() returns &StateName("HighValuePath")
  - assign() returns None
```

#### Scenario CR-2: Construct valid ChoiceRule with assign

```
Given: a valid Expression "$.ready == true", StateName "Process",
       and assign map { VariableName("result") -> Expression("$.output") }
When:  ChoiceRule::new(condition, next, Some(assign_map)) is called
Then:
  - Returns a ChoiceRule
  - assign() returns Some(map) with 1 entry
```

#### Scenario CR-3: Serialize ChoiceRule roundtrip

```
Given: a ChoiceRule with condition "$.x > 0", next "Positive", no assign
When:  serde_json::to_string(&rule) is called
Then:
  - Returns Ok('{"condition":"$.x > 0","next":"Positive"}')
When:  serde_json::from_str::<ChoiceRule>(json) is called on the result
Then:
  - Returns Ok(rule) where rule == original
```

#### Scenario CR-4: Serialize ChoiceRule with assign roundtrip

```
Given: a ChoiceRule with assign map { "out" -> "$.result" }
When:  serde_json::to_string(&rule) is called
Then:
  - JSON includes "assign" key with the map
When:  serde_json::from_str::<ChoiceRule>(json) is called on the result
Then:
  - Returns Ok(rule) where rule == original
```

---

### ChoiceState

#### Scenario CS-1: Construct valid ChoiceState with one rule, no default

```
Given: a vec with one valid ChoiceRule, no default
When:  ChoiceState::new(choices, None) is called
Then:
  - Returns Ok(ChoiceState)
  - choices() returns a slice of length 1
  - default() returns None
```

#### Scenario CS-2: Construct valid ChoiceState with multiple rules and default

```
Given: a vec with 3 valid ChoiceRules, default = StateName("Fallback")
When:  ChoiceState::new(choices, Some(default)) is called
Then:
  - Returns Ok(ChoiceState)
  - choices() returns a slice of length 3
  - default() returns Some(&StateName("Fallback"))
```

#### Scenario CS-3: Reject empty choices

```
Given: an empty vec, no default
When:  ChoiceState::new(vec![], None) is called
Then:
  - Returns Err(ChoiceStateError::EmptyChoices)
```

#### Scenario CS-4: Deserialize valid ChoiceState

```
Given: JSON '{"choices": [{"condition": "$.x > 0", "next": "Pos"}], "default": "Neg"}'
When:  serde_json::from_str::<ChoiceState>(json) is called
Then:
  - Returns Ok(ChoiceState) with 1 choice and default "Neg"
```

#### Scenario CS-5: Reject deserialization with empty choices array

```
Given: JSON '{"choices": []}'
When:  serde_json::from_str::<ChoiceState>(json) is called
Then:
  - Returns Err (deserialization error containing "at least one")
```

#### Scenario CS-6: Serialize ChoiceState roundtrip

```
Given: a valid ChoiceState with 2 rules and default "Fallback"
When:  serde_json::to_string(&state) then serde_json::from_str(json)
Then:
  - Roundtrip produces an equal ChoiceState
```

---

### WaitDuration

#### Scenario WD-1: Construct Seconds variant

```
Given: nothing
When:  WaitDuration::Seconds(30) is constructed
Then:
  - is_seconds() returns true
  - is_timestamp() returns false
  - is_seconds_path() returns false
  - is_timestamp_path() returns false
```

#### Scenario WD-2: Construct Timestamp variant

```
Given: nothing
When:  WaitDuration::Timestamp("2024-12-31T23:59:59Z".into()) is constructed
Then:
  - is_timestamp() returns true
  - All other is_*() return false
```

#### Scenario WD-3: Construct SecondsPath variant

```
Given: a valid JsonPath "$.config.delay"
When:  WaitDuration::SecondsPath(path) is constructed
Then:
  - is_seconds_path() returns true
  - All other is_*() return false
```

#### Scenario WD-4: Construct TimestampPath variant

```
Given: a valid JsonPath "$.schedule.when"
When:  WaitDuration::TimestampPath(path) is constructed
Then:
  - is_timestamp_path() returns true
  - All other is_*() return false
```

#### Scenario WD-5: Deserialize seconds

```
Given: JSON '{"seconds": 60}'
When:  deserialized as WaitDuration (via WaitState or standalone)
Then:
  - Returns Ok(WaitDuration::Seconds(60))
```

#### Scenario WD-6: Deserialize timestamp

```
Given: JSON '{"timestamp": "2024-01-01T00:00:00Z"}'
When:  deserialized as WaitDuration
Then:
  - Returns Ok(WaitDuration::Timestamp("2024-01-01T00:00:00Z"))
```

#### Scenario WD-7: Deserialize seconds_path

```
Given: JSON '{"seconds_path": "$.delay"}'
When:  deserialized as WaitDuration
Then:
  - Returns Ok(WaitDuration::SecondsPath(JsonPath("$.delay")))
```

#### Scenario WD-8: Deserialize timestamp_path

```
Given: JSON '{"timestamp_path": "$.when"}'
When:  deserialized as WaitDuration
Then:
  - Returns Ok(WaitDuration::TimestampPath(JsonPath("$.when")))
```

#### Scenario WD-9: Reject no duration field

```
Given: JSON '{}'
When:  deserialized as WaitDuration
Then:
  - Returns Err containing "exactly one"
```

#### Scenario WD-10: Reject multiple duration fields

```
Given: JSON '{"seconds": 10, "timestamp": "2024-01-01T00:00:00Z"}'
When:  deserialized as WaitDuration
Then:
  - Returns Err containing "multiple fields"
```

#### Scenario WD-11: Reject empty timestamp string

```
Given: JSON '{"timestamp": ""}'
When:  deserialized as WaitDuration
Then:
  - Returns Err containing "empty"
```

#### Scenario WD-12: Reject invalid JsonPath in seconds_path

```
Given: JSON '{"seconds_path": "no-dollar"}'
When:  deserialized as WaitDuration
Then:
  - Returns Err (JsonPath validation failure -- must start with '$')
```

#### Scenario WD-13: Serialize seconds roundtrip

```
Given: WaitDuration::Seconds(10)
When:  serde_json::to_string(&wd) then serde_json::from_str(json)
Then:
  - Roundtrip produces WaitDuration::Seconds(10)
```

#### Scenario WD-14: Serialize timestamp_path roundtrip

```
Given: WaitDuration::TimestampPath(JsonPath("$.schedule"))
When:  serde_json::to_string(&wd) then serde_json::from_str(json)
Then:
  - Roundtrip produces equal WaitDuration
```

#### Scenario WD-15: Display formatting

```
Given: WaitDuration::Seconds(30)
When:  format!("{}", wd)
Then:  "seconds: 30"

Given: WaitDuration::Timestamp("2024-01-01T00:00:00Z")
When:  format!("{}", wd)
Then:  "timestamp: 2024-01-01T00:00:00Z"

Given: WaitDuration::SecondsPath(JsonPath("$.delay"))
When:  format!("{}", wd)
Then:  "seconds_path: $.delay"

Given: WaitDuration::TimestampPath(JsonPath("$.when"))
When:  format!("{}", wd)
Then:  "timestamp_path: $.when"
```

---

### WaitState

#### Scenario WS-1: Construct WaitState with seconds and next

```
Given: WaitDuration::Seconds(10), Transition::next(StateName("ProcessResult"))
When:  WaitState::new(duration, transition) is called
Then:
  - Returns a WaitState
  - duration() returns &WaitDuration::Seconds(10)
  - transition() returns &Transition::Next(StateName("ProcessResult"))
```

#### Scenario WS-2: Construct WaitState with timestamp and end

```
Given: WaitDuration::Timestamp("2024-12-31T23:59:59Z"), Transition::end()
When:  WaitState::new(duration, transition) is called
Then:
  - Returns a WaitState
  - duration() returns the timestamp variant
  - transition() returns &Transition::End
```

#### Scenario WS-3: Deserialize flattened WaitState with seconds + next

```
Given: JSON '{"seconds": 10, "next": "Step2"}'
When:  serde_json::from_str::<WaitState>(json) is called
Then:
  - Returns Ok(WaitState) with Seconds(10) and Transition::Next("Step2")
```

#### Scenario WS-4: Deserialize flattened WaitState with timestamp_path + end

```
Given: JSON '{"timestamp_path": "$.when", "end": true}'
When:  serde_json::from_str::<WaitState>(json) is called
Then:
  - Returns Ok(WaitState) with TimestampPath("$.when") and Transition::End
```

#### Scenario WS-5: Reject WaitState with no duration fields

```
Given: JSON '{"next": "Step2"}'
When:  serde_json::from_str::<WaitState>(json) is called
Then:
  - Returns Err (WaitDuration error)
```

#### Scenario WS-6: Reject WaitState with no transition fields

```
Given: JSON '{"seconds": 10}'
When:  serde_json::from_str::<WaitState>(json) is called
Then:
  - Returns Err (Transition error -- neither next nor end)
```

#### Scenario WS-7: Serialize WaitState roundtrip

```
Given: WaitState with Seconds(5) and Transition::next("Done")
When:  serde_json::to_string(&ws) then serde_json::from_str(json)
Then:
  - Roundtrip produces equal WaitState
  - JSON is '{"seconds":5,"next":"Done"}'
```

---

### PassState

#### Scenario PS-1: Construct PassState with result and next

```
Given: result = Some(json!({"key": "value"})), transition = Transition::next("Process")
When:  PassState::new(result, transition) is called
Then:
  - result() returns Some(&json!({"key": "value"}))
  - transition() returns &Transition::Next(StateName("Process"))
```

#### Scenario PS-2: Construct PassState with no result and end

```
Given: result = None, transition = Transition::end()
When:  PassState::new(None, transition) is called
Then:
  - result() returns None
  - transition() returns &Transition::End
```

#### Scenario PS-3: Deserialize PassState with result

```
Given: JSON '{"result": {"output": 42}, "next": "Step2"}'
When:  serde_json::from_str::<PassState>(json) is called
Then:
  - Returns Ok(PassState) with result = Some(json!({"output": 42})) and next = "Step2"
```

#### Scenario PS-4: Deserialize PassState without result

```
Given: JSON '{"end": true}'
When:  serde_json::from_str::<PassState>(json) is called
Then:
  - Returns Ok(PassState) with result = None and Transition::End
```

#### Scenario PS-5: Serialize PassState roundtrip (with result)

```
Given: PassState with result Some(json!("hello")) and next "Done"
When:  serde_json::to_string(&ps) then serde_json::from_str(json)
Then:
  - Roundtrip produces equal PassState
  - JSON includes "result" key
```

#### Scenario PS-6: Serialize PassState roundtrip (without result)

```
Given: PassState with result None and end
When:  serde_json::to_string(&ps) is called
Then:
  - JSON does NOT include "result" key
  - JSON is '{"end":true}'
```

---

### SucceedState

#### Scenario SS-1: Construct SucceedState

```
Given: nothing
When:  SucceedState::new() is called (or SucceedState::default())
Then:
  - Returns SucceedState (unit struct)
```

#### Scenario SS-2: Serialize SucceedState

```
Given: SucceedState
When:  serde_json::to_string(&ss) is called
Then:
  - Returns Ok('{}')
```

#### Scenario SS-3: Deserialize SucceedState

```
Given: JSON '{}'
When:  serde_json::from_str::<SucceedState>(json) is called
Then:
  - Returns Ok(SucceedState)
```

#### Scenario SS-4: SucceedState equality

```
Given: s1 = SucceedState, s2 = SucceedState
When:  s1 == s2
Then:  true (always -- unit struct)
```

#### Scenario SS-5: SucceedState is Copy

```
Given: s1 = SucceedState
When:  s2 = s1 (copy, not move)
Then:  s1 and s2 both remain usable
```

---

### FailState

#### Scenario FS-1: Construct FailState with both error and cause

```
Given: error = Some("TaskFailed"), cause = Some("Lambda timed out")
When:  FailState::new(error, cause) is called
Then:
  - error() returns Some("TaskFailed")
  - cause() returns Some("Lambda timed out")
```

#### Scenario FS-2: Construct FailState with only error

```
Given: error = Some("InternalError"), cause = None
When:  FailState::new(error, None) is called
Then:
  - error() returns Some("InternalError")
  - cause() returns None
```

#### Scenario FS-3: Construct FailState with only cause

```
Given: error = None, cause = Some("unexpected EOF")
When:  FailState::new(None, cause) is called
Then:
  - error() returns None
  - cause() returns Some("unexpected EOF")
```

#### Scenario FS-4: Construct empty FailState

```
Given: error = None, cause = None
When:  FailState::new(None, None) is called
Then:
  - error() returns None
  - cause() returns None
```

#### Scenario FS-5: Deserialize FailState with both fields

```
Given: JSON '{"error": "MyError", "cause": "something broke"}'
When:  serde_json::from_str::<FailState>(json) is called
Then:
  - Returns Ok(FailState) with error = Some("MyError"), cause = Some("something broke")
```

#### Scenario FS-6: Deserialize empty FailState

```
Given: JSON '{}'
When:  serde_json::from_str::<FailState>(json) is called
Then:
  - Returns Ok(FailState) with error = None, cause = None
```

#### Scenario FS-7: Serialize FailState skips None fields

```
Given: FailState with error = Some("E"), cause = None
When:  serde_json::to_string(&fs) is called
Then:
  - Returns Ok('{"error":"E"}')
  - JSON does NOT contain "cause" key
```

#### Scenario FS-8: Serialize FailState roundtrip

```
Given: FailState with error = Some("TaskFailed"), cause = Some("timeout")
When:  serde_json::to_string(&fs) then serde_json::from_str(json)
Then:
  - Roundtrip produces equal FailState
```

#### Scenario FS-9: Display formatting

```
Given: FailState with error = Some("TaskFailed"), cause = Some("timeout")
When:  format!("{}", fs)
Then:  "FAIL: TaskFailed (timeout)"

Given: FailState with error = Some("E"), cause = None
When:  format!("{}", fs)
Then:  "FAIL: E"

Given: FailState with error = None, cause = Some("oops")
When:  format!("{}", fs)
Then:  "FAIL: (oops)"

Given: FailState with error = None, cause = None
When:  format!("{}", fs)
Then:  "FAIL"
```

#### Scenario FS-10: FailState equality

```
Given: f1 = FailState::new(Some("E"), Some("C")),
       f2 = FailState::new(Some("E"), Some("C"))
When:  f1 == f2
Then:  true

Given: f1 = FailState::new(Some("E1"), None),
       f2 = FailState::new(Some("E2"), None)
When:  f1 == f2
Then:  false
```

---

## Exit Criteria

- [ ] Every type has explicit field listing with types and constraints
- [ ] Every invariant has an ID and is referenced by at least one scenario
- [ ] INV-CS1 (choices non-empty) has both happy-path and rejection scenarios
- [ ] INV-WD1 (mutual exclusivity) has scenarios for zero, one, and multiple fields
- [ ] WaitDuration custom deserializer has scenarios for all four variants + error paths
- [ ] WaitState flattened serde has roundtrip and error scenarios
- [ ] All terminal states (Succeed, Fail) have no Transition field verified by type definition
- [ ] Every error variant has at least one Given-When-Then scenario that triggers it
- [ ] ChoiceStateError::EmptyChoices is tested in CS-3 and CS-5
- [ ] WaitDurationError::NoFieldSpecified is tested in WD-9
- [ ] WaitDurationError::MultipleFieldsSpecified is tested in WD-10
- [ ] WaitDurationError::EmptyTimestamp is tested in WD-11
- [ ] All serde roundtrips (serialize then deserialize) produce equal values
