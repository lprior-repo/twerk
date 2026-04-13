# Contract Specification: twerk-9xv

## ASL Core: Transition, Retrier, and Catcher Types

## Context

- **Feature**: Create three ASL composite types that compose the validated NewTypes from twerk-fq8
- **Files**:
  - `crates/twerk-core/src/asl/transition.rs` -- Transition enum (Next | End)
  - `crates/twerk-core/src/asl/retrier.rs` -- Retrier struct (retry policy)
  - `crates/twerk-core/src/asl/catcher.rs` -- Catcher struct (error catch-and-route)
- **Domain terms**:
  - **ASL**: Amazon States Language -- JSON/YAML-based language for defining state machines
  - **Transition**: How a state declares what happens after it completes -- either go to a named next state, or end the execution
  - **Retrier**: A retry policy attached to a state, matching specific errors and applying exponential backoff
  - **Catcher**: A fallback route attached to a state, matching specific errors after retries are exhausted and routing to a recovery state
  - **JitterStrategy**: Controls whether random jitter is added to computed retry delays (FULL = random 0..delay, NONE = exact delay)
  - **Exponential backoff**: delay = interval_seconds * (backoff_rate ^ attempt), capped by max_delay_seconds
- **Dependency types** (from twerk-fq8, `asl/types.rs` and `asl/error_code.rs`):
  - `StateName` -- validated 1-256 char string (INV: non-empty, <= 256 chars)
  - `Expression` -- validated non-empty string
  - `JsonPath` -- validated non-empty string starting with `$`
  - `VariableName` -- validated identifier (1-128 chars, starts with letter or `_`, body `[a-zA-Z0-9_]`)
  - `BackoffRate` -- validated f64 > 0.0 and finite (no Eq/Hash)
  - `ErrorCode` -- enum of known ASL errors plus Custom(String)
- **Assumptions**:
  - All dependency types already exist and are validated at construction (parse-don't-validate)
  - Transition is an enum, not a struct -- the mutual exclusivity is enforced by the type system
  - Retrier and Catcher are constructed via validated constructors returning `Result<Self, Error>`
  - All types are immutable after construction (no setters)
  - Serde deserialization must re-validate all invariants
  - `HashMap<VariableName, Expression>` in Catcher.assign requires VariableName to impl Hash (it does, per twerk-fq8 contract)
- **Open questions**: None

---

## Types

### File: `crates/twerk-core/src/asl/transition.rs`

#### 1. Transition

```
enum Transition {
    Next(StateName),
    End,
}
```

| Attribute | Value |
|-----------|-------|
| Variants | `Next(StateName)`, `End` |
| Serde | Custom deserializer: `{"next": "state-name"}` or `{"end": true}` -- mutually exclusive fields |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` |

#### 2. JitterStrategy

```
enum JitterStrategy {
    Full,
    None,
}
```

| Attribute | Value |
|-----------|-------|
| Variants | `Full`, `None` |
| Serde | `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` -- serialises as `"FULL"` / `"NONE"` |
| Derives | `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize` |
| Default | `JitterStrategy::None` |

### File: `crates/twerk-core/src/asl/retrier.rs`

#### 3. Retrier

```
struct Retrier {
    error_equals:     Vec<ErrorCode>,
    interval_seconds: u64,
    max_attempts:     u32,
    backoff_rate:     BackoffRate,
    max_delay_seconds: Option<u64>,
    jitter_strategy:  JitterStrategy,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | See above |
| Serde | `#[serde(rename_all = "camelCase")]` with custom deserialization that validates invariants |
| Derives | `Debug`, `Clone`, `PartialEq` |
| Note | No `Eq` or `Hash` because `BackoffRate` wraps `f64` |

### File: `crates/twerk-core/src/asl/catcher.rs`

#### 4. Catcher

```
struct Catcher {
    error_equals: Vec<ErrorCode>,
    next:         StateName,
    result_path:  Option<JsonPath>,
    assign:       Option<HashMap<VariableName, Expression>>,
}
```

| Attribute | Value |
|-----------|-------|
| Fields | See above |
| Serde | `#[serde(rename_all = "camelCase")]` |
| Derives | `Debug`, `Clone`, `PartialEq`, `Eq` |

---

## Invariants

These must ALWAYS hold for any instance of the type that exists in memory:

| ID | Type | Invariant |
|----|------|-----------|
| INV-T1 | `Transition` | Exactly one of `Next(name)` or `End` -- enforced by enum representation; never both, never neither |
| INV-T2 | `Transition::Next` | The inner `StateName` satisfies all StateName invariants (INV-1 from twerk-fq8) |
| INV-R1 | `Retrier` | `!self.error_equals.is_empty()` -- at least one error code to match |
| INV-R2 | `Retrier` | `self.interval_seconds >= 1` -- minimum 1 second between retries |
| INV-R3 | `Retrier` | `self.max_attempts >= 1` -- at least one attempt required |
| INV-R4 | `Retrier` | `self.backoff_rate` satisfies BackoffRate invariants (INV-7: finite, > 0.0) |
| INV-R5 | `Retrier` | If `self.max_delay_seconds` is `Some(d)`, then `d > self.interval_seconds` |
| INV-R6 | `Retrier` | `self.jitter_strategy` is a valid JitterStrategy variant |
| INV-C1 | `Catcher` | `!self.error_equals.is_empty()` -- at least one error code to match |
| INV-C2 | `Catcher` | `self.next` satisfies all StateName invariants (INV-1 from twerk-fq8) |
| INV-C3 | `Catcher` | If `self.result_path` is `Some(p)`, then `p` satisfies all JsonPath invariants (INV-3) |
| INV-C4 | `Catcher` | If `self.assign` is `Some(map)`, then every key satisfies VariableName invariants (INV-4) and every value satisfies Expression invariants (INV-2) |

---

## Trait Requirements

### Transition

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Custom impl: `Next(name)` -> `{"next": "name"}`, `End` -> `{"end": true}` |
| `Deserialize` | Custom impl: reads map with mutually exclusive `next`/`end` fields |
| `Display` | Manual impl: `Next("name")` formats as `"-> name"`, `End` formats as `"END"` |

### JitterStrategy

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `Copy` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` |
| `Deserialize` | Derive with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` |
| `Default` | Manual impl: returns `JitterStrategy::None` |
| `Display` | Manual impl: `"FULL"` or `"NONE"` |

### Retrier

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "...")]` to enforce invariants on deserialize |

**NOT** derived for Retrier: `Eq`, `Hash` (contains `BackoffRate` which wraps `f64`).

### Catcher

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Serialize` | Derive with `#[serde(rename_all = "camelCase")]` |
| `Deserialize` | Custom impl via `#[serde(try_from = "...")]` to enforce invariants on deserialize |

---

## Error Taxonomy

### `TransitionError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransitionError {
    #[error("transition has both 'next' and 'end' fields set")]
    BothNextAndEnd,
    #[error("transition has neither 'next' nor 'end' field")]
    NeitherNextNorEnd,
    #[error("transition 'end' field must be true, got false")]
    EndMustBeTrue,
    #[error("invalid state name in transition: {0}")]
    InvalidStateName(#[from] StateNameError),
}
```

| Variant | Trigger |
|---------|---------|
| `BothNextAndEnd` | Deserializing a map that contains both `next` and `end` keys |
| `NeitherNextNorEnd` | Deserializing a map that contains neither `next` nor `end` keys |
| `EndMustBeTrue` | Deserializing `{"end": false}` -- end must be `true` if present |
| `InvalidStateName(e)` | The `next` field value fails StateName validation |

### `RetrierError`

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RetrierError {
    #[error("retrier error_equals must not be empty")]
    EmptyErrorEquals,
    #[error("retrier interval_seconds must be >= 1, got {0}")]
    IntervalTooSmall(u64),
    #[error("retrier max_attempts must be >= 1, got {0}")]
    MaxAttemptsTooSmall(u32),
    #[error("retrier max_delay_seconds ({max_delay}) must be > interval_seconds ({interval})")]
    MaxDelayNotGreaterThanInterval { max_delay: u64, interval: u64 },
}
```

| Variant | Trigger |
|---------|---------|
| `EmptyErrorEquals` | `error_equals` vec is empty |
| `IntervalTooSmall(v)` | `interval_seconds` is 0 |
| `MaxAttemptsTooSmall(v)` | `max_attempts` is 0 |
| `MaxDelayNotGreaterThanInterval { .. }` | `max_delay_seconds` is `Some(d)` where `d <= interval_seconds` |

Note: `RetrierError` derives `PartialEq` only (no `Eq`) because the error messages may reference `BackoffRate` context in the future, and for consistency with the Retrier type itself.

### `CatcherError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatcherError {
    #[error("catcher error_equals must not be empty")]
    EmptyErrorEquals,
}
```

| Variant | Trigger |
|---------|---------|
| `EmptyErrorEquals` | `error_equals` vec is empty |

Note: `CatcherError` does not need variants for invalid `next`, `result_path`, or `assign` because those fields use already-validated NewTypes. If they arrive via serde, the inner types' own deserialization validates them before Catcher construction.

---

## Contract Signatures

### Transition

```rust
impl Transition {
    /// Convenience constructor for the Next variant.
    /// PRE: `name` is a valid StateName (already constructed and validated)
    /// POST: returns Transition::Next(name)
    /// INVARIANT: INV-T1, INV-T2 hold
    #[must_use]
    pub fn next(name: StateName) -> Self;

    /// Convenience constructor for the End variant.
    /// PRE: none
    /// POST: returns Transition::End
    /// INVARIANT: INV-T1 holds
    #[must_use]
    pub fn end() -> Self;

    /// Returns true if this transition goes to a named state.
    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns true iff self is Next(_)
    #[must_use]
    pub fn is_next(&self) -> bool;

    /// Returns true if this transition ends the execution.
    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns true iff self is End
    #[must_use]
    pub fn is_end(&self) -> bool;

    /// Returns the target state name if this is a Next transition.
    /// PRE: self is valid (guaranteed by construction)
    /// POST: returns Some(&StateName) iff self is Next, else None
    #[must_use]
    pub fn target_state(&self) -> Option<&StateName>;
}
```

### Transition Serde (Custom Deserializer)

```rust
/// Custom deserializer for Transition.
///
/// Accepts exactly one of two mutually exclusive shapes:
///   { "next": "<state-name>" }   -> Transition::Next(StateName)
///   { "end": true }              -> Transition::End
///
/// PRE: Input is a YAML/JSON map
/// POST on success: Transition value satisfying INV-T1
/// ERR: TransitionError::BothNextAndEnd if both fields present
/// ERR: TransitionError::NeitherNextNorEnd if neither field present
/// ERR: TransitionError::EndMustBeTrue if end field is false
/// ERR: TransitionError::InvalidStateName if next value fails StateName validation
impl<'de> Deserialize<'de> for Transition { ... }

/// Custom serializer for Transition.
///
/// Transition::Next(name) serializes as { "next": "<name>" }
/// Transition::End serializes as { "end": true }
impl Serialize for Transition { ... }
```

### JitterStrategy

```rust
impl Default for JitterStrategy {
    /// POST: returns JitterStrategy::None
    fn default() -> Self;
}

impl fmt::Display for JitterStrategy {
    /// POST: Full -> "FULL", None -> "NONE"
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}
```

### Retrier

```rust
impl Retrier {
    /// Validated constructor.
    ///
    /// PRE: error_equals is non-empty
    /// PRE: interval_seconds >= 1
    /// PRE: max_attempts >= 1
    /// PRE: backoff_rate is a valid BackoffRate (guaranteed by type)
    /// PRE: if max_delay_seconds is Some(d), then d > interval_seconds
    /// POST: returned Ok(Self) satisfies INV-R1 through INV-R6
    /// ERR: RetrierError (see error taxonomy)
    pub fn new(
        error_equals: Vec<ErrorCode>,
        interval_seconds: u64,
        max_attempts: u32,
        backoff_rate: BackoffRate,
        max_delay_seconds: Option<u64>,
        jitter_strategy: JitterStrategy,
    ) -> Result<Self, RetrierError>;

    /// PRE: self is valid
    /// POST: returned slice is non-empty
    #[must_use]
    pub fn error_equals(&self) -> &[ErrorCode];

    /// PRE: self is valid
    /// POST: returned value >= 1
    #[must_use]
    pub fn interval_seconds(&self) -> u64;

    /// PRE: self is valid
    /// POST: returned value >= 1
    #[must_use]
    pub fn max_attempts(&self) -> u32;

    /// PRE: self is valid
    /// POST: returned value satisfies BackoffRate invariants
    #[must_use]
    pub fn backoff_rate(&self) -> BackoffRate;

    /// PRE: self is valid
    /// POST: if Some(d), then d > self.interval_seconds()
    #[must_use]
    pub fn max_delay_seconds(&self) -> Option<u64>;

    /// PRE: self is valid
    /// POST: returns current jitter strategy
    #[must_use]
    pub fn jitter_strategy(&self) -> JitterStrategy;
}
```

### Catcher

```rust
impl Catcher {
    /// Validated constructor.
    ///
    /// PRE: error_equals is non-empty
    /// PRE: next is a valid StateName (guaranteed by type)
    /// PRE: result_path, if Some, is a valid JsonPath (guaranteed by type)
    /// PRE: assign, if Some, has valid VariableName keys and Expression values (guaranteed by types)
    /// POST: returned Ok(Self) satisfies INV-C1 through INV-C4
    /// ERR: CatcherError::EmptyErrorEquals
    pub fn new(
        error_equals: Vec<ErrorCode>,
        next: StateName,
        result_path: Option<JsonPath>,
        assign: Option<HashMap<VariableName, Expression>>,
    ) -> Result<Self, CatcherError>;

    /// PRE: self is valid
    /// POST: returned slice is non-empty
    #[must_use]
    pub fn error_equals(&self) -> &[ErrorCode];

    /// PRE: self is valid
    /// POST: returned StateName satisfies INV-1
    #[must_use]
    pub fn next(&self) -> &StateName;

    /// PRE: self is valid
    /// POST: if Some, the JsonPath satisfies INV-3
    #[must_use]
    pub fn result_path(&self) -> Option<&JsonPath>;

    /// PRE: self is valid
    /// POST: if Some, all keys satisfy INV-4 and all values satisfy INV-2
    #[must_use]
    pub fn assign(&self) -> Option<&HashMap<VariableName, Expression>>;
}
```

---

## Serde Deserialization Contracts

### Transition

- **Custom Serialize and Deserialize** (NOT derived)
- Serialization:
  - `Transition::Next(name)` -> `{"next": "<name.as_str()>"}`
  - `Transition::End` -> `{"end": true}`
- Deserialization reads a map and expects exactly one of:
  - `next` key with a string value -> validates as StateName, produces `Transition::Next`
  - `end` key with boolean `true` -> produces `Transition::End`
- Errors if both keys present, neither key present, or `end: false`
- **Postcondition**: any successfully deserialized Transition satisfies INV-T1 and INV-T2
- **Embedding**: When Transition fields appear as part of a parent struct (e.g., a State), the `next`/`end` keys are flattened into the parent via `#[serde(flatten)]`

### JitterStrategy

- `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` on the enum
- Serializes as `"FULL"` or `"NONE"`
- Deserialization is case-sensitive (ASL spec uses uppercase)
- An unrecognised string MUST produce a deserialization error
- **Postcondition**: any successfully deserialized value is `Full` or `None`

### Retrier

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `error_equals` -> `"errorEquals"`
  - `interval_seconds` -> `"intervalSeconds"`
  - `max_attempts` -> `"maxAttempts"`
  - `backoff_rate` -> `"backoffRate"`
  - `max_delay_seconds` -> `"maxDelaySeconds"` (skipped if None via `#[serde(skip_serializing_if = "Option::is_none")]`)
  - `jitter_strategy` -> `"jitterStrategy"` (defaults to `"NONE"` via `#[serde(default)]`)
- Deserialization uses `#[serde(try_from = "RawRetrier")]` where `RawRetrier` is an intermediate struct that captures raw fields, then validates via `Retrier::new()`
- A YAML/JSON document that violates any Retrier invariant MUST produce a deserialization error
- **Postcondition**: any successfully deserialized Retrier satisfies INV-R1 through INV-R6

### Catcher

- Derives `Serialize` with `#[serde(rename_all = "camelCase")]`
- Field name mapping:
  - `error_equals` -> `"errorEquals"`
  - `next` -> `"next"`
  - `result_path` -> `"resultPath"` (skipped if None)
  - `assign` -> `"assign"` (skipped if None)
- Deserialization uses `#[serde(try_from = "RawCatcher")]` where `RawCatcher` captures raw fields, then validates via `Catcher::new()`
- A YAML/JSON document with empty `errorEquals` MUST produce a deserialization error
- **Postcondition**: any successfully deserialized Catcher satisfies INV-C1 through INV-C4

---

## Non-goals

- No `Default` impl for Transition, Retrier, or Catcher (no sensible defaults for these domain types)
- No `Ord`/`PartialOrd` for any type (ordering is not meaningful in ASL context)
- No computation of actual retry delays (that is execution-layer concern, not type-layer)
- No validation of error_equals ordering semantics (first-match-wins is runtime behaviour)
- No `Hash` for Retrier (contains BackoffRate which wraps f64)
- No builder pattern (validated constructor is sufficient; fields are few and explicit)
- JitterStrategy does not need FromStr (only ever parsed via serde from ASL documents)

---

## Given-When-Then Scenarios

### Transition

#### Scenario TR-1: Construct Next variant

```
Given: a valid StateName "ProcessOrder"
When:  Transition::next(name) is called
Then:
  - Returns Transition::Next(StateName("ProcessOrder"))
  - is_next() returns true
  - is_end() returns false
  - target_state() returns Some(&StateName("ProcessOrder"))
```

#### Scenario TR-2: Construct End variant

```
Given: nothing
When:  Transition::end() is called
Then:
  - Returns Transition::End
  - is_next() returns false
  - is_end() returns true
  - target_state() returns None
```

#### Scenario TR-3: Deserialize next transition

```
Given: YAML string 'next: "ProcessOrder"'
When:  serde_yaml::from_str::<Transition>(yaml) is called
Then:
  - Returns Ok(Transition::Next(StateName("ProcessOrder")))
```

#### Scenario TR-4: Deserialize end transition

```
Given: YAML string 'end: true'
When:  serde_yaml::from_str::<Transition>(yaml) is called
Then:
  - Returns Ok(Transition::End)
```

#### Scenario TR-5: Reject both next and end

```
Given: JSON string '{"next": "Foo", "end": true}'
When:  serde_json::from_str::<Transition>(json) is called
Then:
  - Returns Err (deserialization error containing "both")
```

#### Scenario TR-6: Reject neither next nor end

```
Given: JSON string '{}'
When:  serde_json::from_str::<Transition>(json) is called
Then:
  - Returns Err (deserialization error containing "neither")
```

#### Scenario TR-7: Reject end: false

```
Given: JSON string '{"end": false}'
When:  serde_json::from_str::<Transition>(json) is called
Then:
  - Returns Err (deserialization error containing "must be true")
```

#### Scenario TR-8: Reject invalid state name in next

```
Given: JSON string '{"next": ""}'
When:  serde_json::from_str::<Transition>(json) is called
Then:
  - Returns Err (deserialization error about empty state name)
```

#### Scenario TR-9: Serialize Next roundtrip

```
Given: Transition::next(StateName::new("Foo").unwrap())
When:  serde_json::to_string(&t) is called
Then:
  - Returns Ok('{"next":"Foo"}')
When:  serde_json::from_str::<Transition>(json) is called on the result
Then:
  - Returns Ok(t) where t == original
```

#### Scenario TR-10: Serialize End roundtrip

```
Given: Transition::end()
When:  serde_json::to_string(&t) is called
Then:
  - Returns Ok('{"end":true}')
When:  serde_json::from_str::<Transition>(json) is called on the result
Then:
  - Returns Ok(Transition::End)
```

#### Scenario TR-11: Display formatting

```
Given: Transition::next(StateName::new("Step2").unwrap())
When:  format!("{}", t) is called
Then:
  - Returns "-> Step2"

Given: Transition::end()
When:  format!("{}", t) is called
Then:
  - Returns "END"
```

#### Scenario TR-12: Equality

```
Given: t1 = Transition::end(), t2 = Transition::end()
When:  t1 == t2
Then:  true

Given: t1 = Transition::next(name_a), t2 = Transition::next(name_a)
When:  t1 == t2
Then:  true

Given: t1 = Transition::next(name_a), t2 = Transition::end()
When:  t1 == t2
Then:  false
```

---

### JitterStrategy

#### Scenario JS-1: Default is None

```
Given: nothing
When:  JitterStrategy::default() is called
Then:
  - Returns JitterStrategy::None
```

#### Scenario JS-2: Serialize Full

```
Given: JitterStrategy::Full
When:  serde_json::to_string(&js) is called
Then:
  - Returns Ok('"FULL"')
```

#### Scenario JS-3: Serialize None

```
Given: JitterStrategy::None
When:  serde_json::to_string(&js) is called
Then:
  - Returns Ok('"NONE"')
```

#### Scenario JS-4: Deserialize FULL

```
Given: JSON string '"FULL"'
When:  serde_json::from_str::<JitterStrategy>(json) is called
Then:
  - Returns Ok(JitterStrategy::Full)
```

#### Scenario JS-5: Deserialize NONE

```
Given: JSON string '"NONE"'
When:  serde_json::from_str::<JitterStrategy>(json) is called
Then:
  - Returns Ok(JitterStrategy::None)
```

#### Scenario JS-6: Reject unknown string

```
Given: JSON string '"HALF"'
When:  serde_json::from_str::<JitterStrategy>(json) is called
Then:
  - Returns Err (deserialization error)
```

#### Scenario JS-7: Reject lowercase (case-sensitive)

```
Given: JSON string '"full"'
When:  serde_json::from_str::<JitterStrategy>(json) is called
Then:
  - Returns Err (deserialization error)
```

#### Scenario JS-8: Display

```
Given: JitterStrategy::Full
When:  format!("{}", js) is called
Then:
  - Returns "FULL"

Given: JitterStrategy::None
When:  format!("{}", js) is called
Then:
  - Returns "NONE"
```

#### Scenario JS-9: Serde roundtrip

```
Given: JitterStrategy::Full
When:  serialized then deserialized
Then:
  - Round-trips to equal value
```

---

### Retrier

#### Scenario RT-1: Valid construction with all fields

```
Given:
  - error_equals = vec![ErrorCode::Timeout, ErrorCode::TaskFailed]
  - interval_seconds = 2
  - max_attempts = 3
  - backoff_rate = BackoffRate::new(2.0).unwrap()
  - max_delay_seconds = Some(30)
  - jitter_strategy = JitterStrategy::Full
When:  Retrier::new(error_equals, interval_seconds, max_attempts, backoff_rate, max_delay_seconds, jitter_strategy)
Then:
  - Returns Ok(Retrier)
  - error_equals() returns &[ErrorCode::Timeout, ErrorCode::TaskFailed]
  - interval_seconds() returns 2
  - max_attempts() returns 3
  - backoff_rate().value() == 2.0
  - max_delay_seconds() returns Some(30)
  - jitter_strategy() returns JitterStrategy::Full
```

#### Scenario RT-2: Valid construction without optional fields

```
Given:
  - error_equals = vec![ErrorCode::All]
  - interval_seconds = 1
  - max_attempts = 1
  - backoff_rate = BackoffRate::new(1.0).unwrap()
  - max_delay_seconds = None
  - jitter_strategy = JitterStrategy::None
When:  Retrier::new(...)
Then:
  - Returns Ok(Retrier)
  - max_delay_seconds() returns None
  - jitter_strategy() returns JitterStrategy::None
```

#### Scenario RT-3: Reject empty error_equals

```
Given: error_equals = vec![]
When:  Retrier::new(error_equals, 1, 1, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Err(RetrierError::EmptyErrorEquals)
```

#### Scenario RT-4: Reject interval_seconds = 0

```
Given: interval_seconds = 0
When:  Retrier::new(errors, 0, 3, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Err(RetrierError::IntervalTooSmall(0))
```

#### Scenario RT-5: Reject max_attempts = 0

```
Given: max_attempts = 0
When:  Retrier::new(errors, 1, 0, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Err(RetrierError::MaxAttemptsTooSmall(0))
```

#### Scenario RT-6: Reject max_delay_seconds <= interval_seconds

```
Given: interval_seconds = 5, max_delay_seconds = Some(5)
When:  Retrier::new(errors, 5, 3, backoff_rate, Some(5), JitterStrategy::None)
Then:
  - Returns Err(RetrierError::MaxDelayNotGreaterThanInterval { max_delay: 5, interval: 5 })
```

#### Scenario RT-7: Reject max_delay_seconds < interval_seconds

```
Given: interval_seconds = 10, max_delay_seconds = Some(3)
When:  Retrier::new(errors, 10, 3, backoff_rate, Some(3), JitterStrategy::None)
Then:
  - Returns Err(RetrierError::MaxDelayNotGreaterThanInterval { max_delay: 3, interval: 10 })
```

#### Scenario RT-8: Boundary -- interval_seconds = 1 (minimum)

```
Given: interval_seconds = 1
When:  Retrier::new(errors, 1, 3, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Ok(Retrier)
  - interval_seconds() returns 1
```

#### Scenario RT-9: Boundary -- max_attempts = 1 (minimum)

```
Given: max_attempts = 1
When:  Retrier::new(errors, 1, 1, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Ok(Retrier)
  - max_attempts() returns 1
```

#### Scenario RT-10: Boundary -- max_delay_seconds just above interval

```
Given: interval_seconds = 5, max_delay_seconds = Some(6)
When:  Retrier::new(errors, 5, 3, backoff_rate, Some(6), JitterStrategy::None)
Then:
  - Returns Ok(Retrier)
  - max_delay_seconds() returns Some(6)
```

#### Scenario RT-11: Serde roundtrip (JSON)

```
Given: a valid Retrier with:
  - error_equals = [ErrorCode::Timeout]
  - interval_seconds = 2
  - max_attempts = 5
  - backoff_rate = 1.5
  - max_delay_seconds = Some(60)
  - jitter_strategy = JitterStrategy::Full
When:  serde_json::to_string(&r) is called
Then:
  - Produces JSON with camelCase keys:
    {
      "errorEquals": ["timeout"],
      "intervalSeconds": 2,
      "maxAttempts": 5,
      "backoffRate": 1.5,
      "maxDelaySeconds": 60,
      "jitterStrategy": "FULL"
    }
When:  serde_json::from_str::<Retrier>(json) is called on the result
Then:
  - Returns Ok(r) where r == original
```

#### Scenario RT-12: Serde omits None fields

```
Given: a valid Retrier with max_delay_seconds = None
When:  serde_json::to_string(&r) is called
Then:
  - JSON output does NOT contain "maxDelaySeconds" key
```

#### Scenario RT-13: Serde defaults jitter_strategy to NONE

```
Given: JSON without "jitterStrategy" key:
  {"errorEquals": ["all"], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0}
When:  serde_json::from_str::<Retrier>(json) is called
Then:
  - Returns Ok(Retrier) with jitter_strategy() == JitterStrategy::None
```

#### Scenario RT-14: Serde rejects invalid retrier on deserialize

```
Given: JSON with empty errorEquals:
  {"errorEquals": [], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0}
When:  serde_json::from_str::<Retrier>(json) is called
Then:
  - Returns Err (deserialization error about empty errorEquals)
```

#### Scenario RT-15: Serde rejects zero interval on deserialize

```
Given: JSON with intervalSeconds = 0
When:  serde_json::from_str::<Retrier>(json) is called
Then:
  - Returns Err (deserialization error about interval)
```

#### Scenario RT-16: YAML deserialization

```
Given: YAML string:
  errorEquals:
    - timeout
    - taskfailed
  intervalSeconds: 3
  maxAttempts: 5
  backoffRate: 2.0
  jitterStrategy: FULL
When:  serde_yaml::from_str::<Retrier>(yaml) is called
Then:
  - Returns Ok(Retrier)
  - error_equals() == &[ErrorCode::Timeout, ErrorCode::TaskFailed]
  - jitter_strategy() == JitterStrategy::Full
```

#### Scenario RT-17: Single error_equals entry (boundary)

```
Given: error_equals = vec![ErrorCode::All]
When:  Retrier::new(error_equals, 1, 1, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Ok(Retrier)
  - error_equals().len() == 1
```

#### Scenario RT-18: Large max_attempts value

```
Given: max_attempts = u32::MAX
When:  Retrier::new(errors, 1, u32::MAX, backoff_rate, None, JitterStrategy::None)
Then:
  - Returns Ok(Retrier) (no upper bound on max_attempts beyond u32)
```

---

### Catcher

#### Scenario CA-1: Valid construction with all fields

```
Given:
  - error_equals = vec![ErrorCode::Timeout, ErrorCode::TaskFailed]
  - next = StateName::new("HandleError").unwrap()
  - result_path = Some(JsonPath::new("$.error").unwrap())
  - assign = Some(HashMap from {VariableName::new("retries").unwrap() => Expression::new("$.retryCount").unwrap()})
When:  Catcher::new(error_equals, next, result_path, assign)
Then:
  - Returns Ok(Catcher)
  - error_equals() returns &[ErrorCode::Timeout, ErrorCode::TaskFailed]
  - next() returns &StateName("HandleError")
  - result_path() returns Some(&JsonPath("$.error"))
  - assign() returns Some(&map) with 1 entry
```

#### Scenario CA-2: Valid construction with minimal fields

```
Given:
  - error_equals = vec![ErrorCode::All]
  - next = StateName::new("Fallback").unwrap()
  - result_path = None
  - assign = None
When:  Catcher::new(error_equals, next, None, None)
Then:
  - Returns Ok(Catcher)
  - result_path() returns None
  - assign() returns None
```

#### Scenario CA-3: Reject empty error_equals

```
Given: error_equals = vec![]
When:  Catcher::new(vec![], next, None, None)
Then:
  - Returns Err(CatcherError::EmptyErrorEquals)
```

#### Scenario CA-4: Serde roundtrip (JSON)

```
Given: a valid Catcher with:
  - error_equals = [ErrorCode::Timeout]
  - next = StateName("RecoveryState")
  - result_path = Some(JsonPath("$.error"))
  - assign = None
When:  serde_json::to_string(&c) is called
Then:
  - Produces JSON with camelCase keys:
    {
      "errorEquals": ["timeout"],
      "next": "RecoveryState",
      "resultPath": "$.error"
    }
When:  serde_json::from_str::<Catcher>(json) is called on the result
Then:
  - Returns Ok(c) where c == original
```

#### Scenario CA-5: Serde omits None fields

```
Given: a valid Catcher with result_path = None, assign = None
When:  serde_json::to_string(&c) is called
Then:
  - JSON output does NOT contain "resultPath" or "assign" keys
```

#### Scenario CA-6: Serde rejects empty errorEquals on deserialize

```
Given: JSON with empty errorEquals:
  {"errorEquals": [], "next": "Foo"}
When:  serde_json::from_str::<Catcher>(json) is called
Then:
  - Returns Err (deserialization error about empty errorEquals)
```

#### Scenario CA-7: Serde with assign map

```
Given: JSON:
  {
    "errorEquals": ["all"],
    "next": "HandleAll",
    "assign": {"error_msg": "$.Cause"}
  }
When:  serde_json::from_str::<Catcher>(json) is called
Then:
  - Returns Ok(Catcher)
  - assign() returns Some with key VariableName("error_msg") and value Expression("$.Cause")
```

#### Scenario CA-8: YAML deserialization

```
Given: YAML string:
  errorEquals:
    - timeout
  next: RecoveryState
  resultPath: "$.error"
When:  serde_yaml::from_str::<Catcher>(yaml) is called
Then:
  - Returns Ok(Catcher)
  - next() == &StateName("RecoveryState")
  - result_path() == Some(&JsonPath("$.error"))
```

#### Scenario CA-9: Single error_equals entry (boundary)

```
Given: error_equals = vec![ErrorCode::Custom("MyError".into())]
When:  Catcher::new(error_equals, next, None, None)
Then:
  - Returns Ok(Catcher)
  - error_equals().len() == 1
```

#### Scenario CA-10: Multiple catchers in a list (integration context)

```
Given: JSON array:
  [
    {"errorEquals": ["timeout"], "next": "TimeoutHandler"},
    {"errorEquals": ["all"], "next": "DefaultHandler"}
  ]
When:  serde_json::from_str::<Vec<Catcher>>(json) is called
Then:
  - Returns Ok(vec) with 2 catchers
  - vec[0].next() == &StateName("TimeoutHandler")
  - vec[1].next() == &StateName("DefaultHandler")
```

#### Scenario CA-11: Multiple retriers in a list (integration context)

```
Given: JSON array:
  [
    {"errorEquals": ["timeout"], "intervalSeconds": 1, "maxAttempts": 3, "backoffRate": 2.0},
    {"errorEquals": ["all"], "intervalSeconds": 5, "maxAttempts": 2, "backoffRate": 1.5, "jitterStrategy": "FULL"}
  ]
When:  serde_json::from_str::<Vec<Retrier>>(json) is called
Then:
  - Returns Ok(vec) with 2 retriers
  - vec[0].error_equals() == &[ErrorCode::Timeout]
  - vec[1].jitter_strategy() == JitterStrategy::Full
```

---

## Exit Criteria

- [ ] Every invariant (INV-T1 through INV-C4) has at least one scenario verifying it
- [ ] Every error variant has at least one scenario triggering it
- [ ] Every public method has at least one scenario calling it
- [ ] Serde roundtrip verified for all four types (Transition, JitterStrategy, Retrier, Catcher)
- [ ] Serde rejection verified for all invalid inputs
- [ ] Boundary values tested (min interval, min attempts, max_delay edge)
- [ ] Both JSON and YAML deserialization covered
- [ ] Optional field omission verified in serialization
- [ ] Default behaviour verified (JitterStrategy default, jitter_strategy serde default)
