# Contract Specification: twerk-fq8

## ASL Core: NewType Foundations (StateName, Expression, ErrorCode, etc.)

## Context

- **Feature**: Create validated NewType wrappers for Amazon States Language primitives
- **Files**:
  - `crates/twerk-core/src/asl/types.rs` — String/numeric NewTypes
  - `crates/twerk-core/src/asl/error_code.rs` — ErrorCode enum
- **Domain terms**:
  - **ASL**: Amazon States Language — JSON-based language for defining state machines
  - **NewType**: Single-field struct wrapping a primitive, enforcing invariants at construction
  - **Parse-don't-validate**: Constructor returns `Result<Self, Error>`; if you hold a value, it is valid
  - **Transparent serde**: `#[serde(transparent)]` — serialises as the inner value, not as `{"0": ...}`
- **Reference pattern**: `crates/twerk-core/src/domain_types.rs` (QueueName, CronExpression, GoDuration, Priority, RetryLimit)
- **Assumptions**:
  - The `asl` module does not yet exist; it will be created as a new submodule of `twerk-core`
  - All types are immutable after construction (no setters)
  - String-wrapping types use `impl Into<String>` on `new()` to accept both `&str` and `String`
  - `BackoffRate` wraps `f64`, so it cannot derive `Eq` or `Hash`
  - ErrorCode `Custom(String)` allows arbitrary user-defined error names
- **Open questions**: None (bead description is sufficiently prescriptive)

---

## Types

### File: `crates/twerk-core/src/asl/types.rs`

#### 1. StateName

```
struct StateName(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | 1-256 UTF-8 characters, non-empty after trimming |
| Serde | `#[serde(transparent)]` |
| Must-use | `"StateName should be used; it validates at construction"` |

#### 2. Expression

```
struct Expression(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | Non-empty |
| Serde | `#[serde(transparent)]` |
| Must-use | `"Expression should be used; it validates at construction"` |

#### 3. JsonPath

```
struct JsonPath(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | Non-empty, must start with `$` |
| Serde | `#[serde(transparent)]` |
| Must-use | `"JsonPath should be used; it validates at construction"` |

#### 4. VariableName

```
struct VariableName(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | Non-empty; starts with ASCII letter or `_`; contains only ASCII alphanumeric or `_`; max 128 chars |
| Serde | `#[serde(transparent)]` |
| Must-use | `"VariableName should be used; it validates at construction"` |

#### 5. ImageRef

```
struct ImageRef(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | Non-empty, no ASCII whitespace |
| Serde | `#[serde(transparent)]` |
| Must-use | `"ImageRef should be used; it validates at construction"` |

#### 6. ShellScript

```
struct ShellScript(String)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `String` |
| Constraint | Non-empty |
| Serde | `#[serde(transparent)]` |
| Must-use | `"ShellScript should be used; it validates at construction"` |

#### 7. BackoffRate

```
struct BackoffRate(f64)
```

| Attribute | Value |
|-----------|-------|
| Inner type | `f64` |
| Constraint | Finite and > 0.0 |
| Serde | `#[serde(transparent)]` |
| Must-use | `"BackoffRate should be used; it validates at construction"` |

### File: `crates/twerk-core/src/asl/error_code.rs`

#### 8. ErrorCode

```
enum ErrorCode {
    All,
    Timeout,
    TaskFailed,
    Permissions,
    ResultPathMatchFailure,
    ParameterPathFailure,
    BranchFailed,
    NoChoiceMatched,
    IntrinsicFailure,
    HeartbeatTimeout,
    Custom(String),
}
```

| Attribute | Value |
|-----------|-------|
| Serde | Custom Serialize/Deserialize: known variants as lowercase strings, Custom as raw string |
| Display | Same as serde serialized form |
| FromStr | Case-insensitive parse for known variants; unrecognised strings become `Custom(s)` |

**Serialization mapping:**

| Variant | Serialized string |
|---------|-------------------|
| `All` | `"all"` |
| `Timeout` | `"timeout"` |
| `TaskFailed` | `"taskfailed"` |
| `Permissions` | `"permissions"` |
| `ResultPathMatchFailure` | `"resultpathmatchfailure"` |
| `ParameterPathFailure` | `"parameterpathfailure"` |
| `BranchFailed` | `"branchfailed"` |
| `NoChoiceMatched` | `"nochoicematched"` |
| `IntrinsicFailure` | `"intrinsicfailure"` |
| `HeartbeatTimeout` | `"heartbeattimeout"` |
| `Custom(s)` | The raw string `s` |

---

## Invariants

These must ALWAYS hold for any instance of the type that exists in memory:

| ID | Type | Invariant |
|----|------|-----------|
| INV-1 | `StateName` | `!self.0.is_empty() && self.0.len() <= 256` |
| INV-2 | `Expression` | `!self.0.is_empty()` |
| INV-3 | `JsonPath` | `!self.0.is_empty() && self.0.starts_with('$')` |
| INV-4 | `VariableName` | `!self.0.is_empty() && self.0.len() <= 128 && first char is ASCII letter or '_' && all chars are ASCII alphanumeric or '_'` |
| INV-5 | `ImageRef` | `!self.0.is_empty() && no ASCII whitespace in self.0` |
| INV-6 | `ShellScript` | `!self.0.is_empty()` |
| INV-7 | `BackoffRate` | `self.0.is_finite() && self.0 > 0.0` |
| INV-8 | `ErrorCode::Custom` | Inner string is non-empty and does NOT match any known variant's lowercase serialized form |

---

## Trait Requirements

### String-wrapping types (StateName, Expression, JsonPath, VariableName, ImageRef, ShellScript)

Every string-wrapping type MUST implement:

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Derive, `#[serde(transparent)]` |
| `Deserialize` | Derive, `#[serde(transparent)]` |
| `Display` | Manual impl: `f.write_str(&self.0)` |
| `FromStr` | Manual impl: delegates to `Self::new(s)` |
| `AsRef<str>` | Manual impl: `&self.0` |
| `Deref<Target=str>` | Manual impl: `&self.0` |

### BackoffRate

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `Copy` | Derive (f64 is Copy) |
| `PartialEq` | Derive |
| `Serialize` | Derive, `#[serde(transparent)]` |
| `Deserialize` | Derive, `#[serde(transparent)]` |
| `Display` | Manual impl: format the f64 |
| `FromStr` | Manual impl: parse f64, then validate > 0.0 |

**NOT** derived for BackoffRate: `Eq`, `Hash` (f64 does not satisfy these).

### ErrorCode

| Trait | Notes |
|-------|-------|
| `Debug` | Derive |
| `Clone` | Derive |
| `PartialEq` | Derive |
| `Eq` | Derive |
| `Hash` | Derive |
| `Serialize` | Custom impl (lowercase string mapping) |
| `Deserialize` | Custom impl (case-insensitive parse, unknown -> Custom) |
| `Display` | Manual impl (same output as Serialize) |
| `FromStr` | Manual impl: never fails; unknown strings become `Custom(s)` |

---

## Error Taxonomy

### `StateNameError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateNameError {
    #[error("state name cannot be empty")]
    Empty,
    #[error("state name length {0} exceeds maximum of 256 characters")]
    TooLong(usize),
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty or contains only whitespace |
| `TooLong(n)` | Input byte length exceeds 256 |

### `ExpressionError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExpressionError {
    #[error("expression cannot be empty")]
    Empty,
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty |

### `JsonPathError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JsonPathError {
    #[error("JSON path cannot be empty")]
    Empty,
    #[error("JSON path must start with '$', got '{0}'")]
    MissingDollarPrefix(String),
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty |
| `MissingDollarPrefix(s)` | First character is not `$` |

### `VariableNameError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VariableNameError {
    #[error("variable name cannot be empty")]
    Empty,
    #[error("variable name length {0} exceeds maximum of 128 characters")]
    TooLong(usize),
    #[error("variable name must start with ASCII letter or underscore, got '{0}'")]
    InvalidStart(char),
    #[error("variable name contains invalid character '{0}'")]
    InvalidCharacter(char),
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty |
| `TooLong(n)` | Input length exceeds 128 |
| `InvalidStart(c)` | First char is not `[a-zA-Z_]` |
| `InvalidCharacter(c)` | Any char is not `[a-zA-Z0-9_]` |

### `ImageRefError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageRefError {
    #[error("image reference cannot be empty")]
    Empty,
    #[error("image reference contains whitespace")]
    ContainsWhitespace,
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty |
| `ContainsWhitespace` | Any ASCII whitespace character found |

### `ShellScriptError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShellScriptError {
    #[error("shell script cannot be empty")]
    Empty,
}
```

| Variant | Trigger |
|---------|---------|
| `Empty` | Input is empty |

### `BackoffRateError`

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum BackoffRateError {
    #[error("backoff rate must be positive, got {0}")]
    NotPositive(f64),
    #[error("backoff rate must be finite, got {0}")]
    NotFinite(f64),
    #[error("failed to parse backoff rate: {0}")]
    ParseError(String),
}
```

| Variant | Trigger |
|---------|---------|
| `NotPositive(v)` | Value is <= 0.0 (including negative zero) |
| `NotFinite(v)` | Value is NaN or infinity |
| `ParseError(s)` | `FromStr` parse of f64 failed |

Note: `BackoffRateError` derives `PartialEq` only (no `Eq`) because it contains `f64`.

---

## Contract Signatures

### StateName

```rust
impl StateName {
    /// PRE: `name` is non-empty and <= 256 bytes after `Into<String>`
    /// POST: returned `Ok(Self)` satisfies INV-1
    /// ERR: StateNameError::Empty | StateNameError::TooLong
    pub fn new(name: impl Into<String>) -> Result<Self, StateNameError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice is non-empty and <= 256 bytes
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### Expression

```rust
impl Expression {
    /// PRE: `expr` is non-empty after `Into<String>`
    /// POST: returned `Ok(Self)` satisfies INV-2
    /// ERR: ExpressionError::Empty
    pub fn new(expr: impl Into<String>) -> Result<Self, ExpressionError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice is non-empty
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### JsonPath

```rust
impl JsonPath {
    /// PRE: `path` is non-empty and starts with '$'
    /// POST: returned `Ok(Self)` satisfies INV-3
    /// ERR: JsonPathError::Empty | JsonPathError::MissingDollarPrefix
    pub fn new(path: impl Into<String>) -> Result<Self, JsonPathError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice starts with '$'
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### VariableName

```rust
impl VariableName {
    /// PRE: `name` is a valid identifier (1-128 chars, starts with [a-zA-Z_], body [a-zA-Z0-9_])
    /// POST: returned `Ok(Self)` satisfies INV-4
    /// ERR: VariableNameError::Empty | TooLong | InvalidStart | InvalidCharacter
    pub fn new(name: impl Into<String>) -> Result<Self, VariableNameError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice is a valid identifier
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### ImageRef

```rust
impl ImageRef {
    /// PRE: `image` is non-empty, no whitespace
    /// POST: returned `Ok(Self)` satisfies INV-5
    /// ERR: ImageRefError::Empty | ImageRefError::ContainsWhitespace
    pub fn new(image: impl Into<String>) -> Result<Self, ImageRefError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice contains no whitespace
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### ShellScript

```rust
impl ShellScript {
    /// PRE: `script` is non-empty
    /// POST: returned `Ok(Self)` satisfies INV-6
    /// ERR: ShellScriptError::Empty
    pub fn new(script: impl Into<String>) -> Result<Self, ShellScriptError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned slice is non-empty
    #[must_use]
    pub fn as_str(&self) -> &str;
}
```

### BackoffRate

```rust
impl BackoffRate {
    /// PRE: `rate` is finite and > 0.0
    /// POST: returned `Ok(Self)` satisfies INV-7
    /// ERR: BackoffRateError::NotFinite | BackoffRateError::NotPositive
    pub fn new(rate: f64) -> Result<Self, BackoffRateError>;

    /// PRE: self is valid (guaranteed by construction)
    /// POST: returned value is finite and > 0.0
    #[must_use]
    pub fn value(self) -> f64;
}
```

### ErrorCode

```rust
impl ErrorCode {
    /// PRE: none (always succeeds; unknown strings become Custom)
    /// POST: returned value is a valid ErrorCode variant
    /// NOTE: This is the FromStr impl; it is infallible for ErrorCode
}
```

### FromStr for all string types

```rust
/// For each string-wrapping type T:
impl FromStr for T {
    type Err = TError;
    /// PRE: same as T::new()
    /// POST: same as T::new()
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// For BackoffRate:
impl FromStr for BackoffRate {
    type Err = BackoffRateError;
    /// PRE: `s` is a valid f64 string representing a finite value > 0.0
    /// POST: same as BackoffRate::new()
    /// ERR: BackoffRateError::ParseError if f64 parse fails,
    ///       then delegates to new() for domain validation
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

/// For ErrorCode:
impl FromStr for ErrorCode {
    type Err = std::convert::Infallible;
    /// PRE: none
    /// POST: known lowercase strings map to known variants; all others become Custom(s.to_owned())
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}
```

---

## Serde Deserialization Contracts

### String NewTypes (StateName, Expression, JsonPath, VariableName, ImageRef, ShellScript)

- `#[serde(transparent)]` on the struct
- Deserialisation MUST invoke validation (via `#[serde(try_from = "String")]` or a custom deserializer)
- A JSON string that violates the type's invariant MUST produce a serde deserialization error
- Postcondition: any successfully deserialized value satisfies all invariants

### BackoffRate

- `#[serde(transparent)]` on the struct
- Deserialisation MUST validate > 0.0 and finite
- A JSON number <= 0.0, NaN, or infinity MUST produce a serde deserialization error

### ErrorCode

- Custom `Serialize`: writes the lowercase string form
- Custom `Deserialize`: reads a string, performs case-insensitive match against known variants, falls back to `Custom(s)`
- A JSON value that is not a string MUST produce a deserialization error

---

## Non-goals

- No runtime mutation of any NewType value (all are immutable by design)
- No `Default` impl for any type (there is no sensible default for validated domain primitives)
- No `Ord`/`PartialOrd` for string types (not needed for ASL; can be added later if required)
- No deep validation of Expression content (template evaluation is a separate concern)
- No deep validation of JsonPath syntax beyond the `$` prefix (full JSONPath parsing is out of scope)
- No deep validation of ImageRef format beyond non-empty/no-whitespace (OCI reference parsing is out of scope)
- No validation of ShellScript content (arbitrary shell is valid)

---

## Given-When-Then Scenarios

### StateName

#### Scenario SN-1: Valid state name

```
Given: a string "HelloWorld" (10 chars)
When:  StateName::new("HelloWorld") is called
Then:
  - Returns Ok(StateName)
  - as_str() returns "HelloWorld"
  - Display formats as "HelloWorld"
  - to_string() returns "HelloWorld"
```

#### Scenario SN-2: Exactly 256 characters (upper boundary)

```
Given: a string of exactly 256 'a' characters
When:  StateName::new(s) is called
Then:
  - Returns Ok(StateName)
  - as_str().len() == 256
```

#### Scenario SN-3: Exactly 1 character (lower boundary)

```
Given: a string "X"
When:  StateName::new("X") is called
Then:
  - Returns Ok(StateName)
  - as_str() returns "X"
```

#### Scenario SN-4: Empty string rejected

```
Given: an empty string ""
When:  StateName::new("") is called
Then:
  - Returns Err(StateNameError::Empty)
```

#### Scenario SN-5: 257 characters rejected

```
Given: a string of 257 'a' characters
When:  StateName::new(s) is called
Then:
  - Returns Err(StateNameError::TooLong(257))
```

#### Scenario SN-6: FromStr roundtrip

```
Given: a valid state name string "MyState"
When:  "MyState".parse::<StateName>() is called
Then:
  - Returns Ok(StateName)
  - to_string() returns "MyState"
```

#### Scenario SN-7: Serde roundtrip

```
Given: StateName::new("Foo").unwrap()
When:  serde_json::to_string(&sn) is called
Then:
  - Returns Ok("\"Foo\"") (transparent, no wrapper object)
When:  serde_json::from_str::<StateName>("\"Foo\"") is called
Then:
  - Returns Ok(StateName) where as_str() == "Foo"
```

#### Scenario SN-8: Serde rejects invalid on deserialize

```
Given: JSON string "\"\""  (empty string)
When:  serde_json::from_str::<StateName>("\"\"") is called
Then:
  - Returns Err (deserialization error due to empty name)
```

#### Scenario SN-9: Deref allows str methods

```
Given: StateName::new("Hello").unwrap()
When:  sn.contains("ell") is called via Deref
Then:
  - Returns true
```

#### Scenario SN-10: Unicode characters allowed

```
Given: a string with valid Unicode chars "状態名"
When:  StateName::new("状態名") is called
Then:
  - Returns Ok(StateName) (Unicode is allowed; constraint is on char count or byte length per impl choice)
```

---

### Expression

#### Scenario EX-1: Valid template expression

```
Given: a string "$.input.name"
When:  Expression::new("$.input.name") is called
Then:
  - Returns Ok(Expression)
  - as_str() returns "$.input.name"
```

#### Scenario EX-2: Empty rejected

```
Given: an empty string ""
When:  Expression::new("") is called
Then:
  - Returns Err(ExpressionError::Empty)
```

#### Scenario EX-3: Intrinsic function expression

```
Given: a string "States.Format('Hello {}', $.name)"
When:  Expression::new(s) is called
Then:
  - Returns Ok(Expression) (no content validation beyond non-empty)
```

#### Scenario EX-4: FromStr roundtrip

```
Given: "$.foo"
When:  "$.foo".parse::<Expression>() is called
Then:
  - Returns Ok(Expression)
  - to_string() returns "$.foo"
```

#### Scenario EX-5: Serde roundtrip

```
Given: Expression::new("$.bar").unwrap()
When:  serialized and deserialized via serde_json
Then:
  - Produces "\"$.bar\"" as JSON
  - Deserializes back to an equal Expression
```

---

### JsonPath

#### Scenario JP-1: Valid JSONPath

```
Given: a string "$.store.book[0].title"
When:  JsonPath::new("$.store.book[0].title") is called
Then:
  - Returns Ok(JsonPath)
  - as_str() returns "$.store.book[0].title"
```

#### Scenario JP-2: Root only

```
Given: a string "$"
When:  JsonPath::new("$") is called
Then:
  - Returns Ok(JsonPath) (single '$' is valid root reference)
```

#### Scenario JP-3: Empty rejected

```
Given: an empty string ""
When:  JsonPath::new("") is called
Then:
  - Returns Err(JsonPathError::Empty)
```

#### Scenario JP-4: Missing dollar prefix rejected

```
Given: a string "store.book"
When:  JsonPath::new("store.book") is called
Then:
  - Returns Err(JsonPathError::MissingDollarPrefix("store.book".to_owned()))
```

#### Scenario JP-5: Serde rejects invalid on deserialize

```
Given: JSON string "\"no.dollar\""
When:  serde_json::from_str::<JsonPath>("\"no.dollar\"") is called
Then:
  - Returns Err (missing dollar prefix)
```

#### Scenario JP-6: FromStr roundtrip

```
Given: "$.x"
When:  "$.x".parse::<JsonPath>()
Then:
  - Returns Ok, to_string() == "$.x"
```

---

### VariableName

#### Scenario VN-1: Valid simple name

```
Given: a string "my_var"
When:  VariableName::new("my_var") is called
Then:
  - Returns Ok(VariableName)
  - as_str() returns "my_var"
```

#### Scenario VN-2: Starts with underscore

```
Given: "_private"
When:  VariableName::new("_private") is called
Then:
  - Returns Ok(VariableName)
```

#### Scenario VN-3: Single character

```
Given: "x"
When:  VariableName::new("x") is called
Then:
  - Returns Ok(VariableName)
```

#### Scenario VN-4: Empty rejected

```
Given: ""
When:  VariableName::new("") is called
Then:
  - Returns Err(VariableNameError::Empty)
```

#### Scenario VN-5: Starts with digit rejected

```
Given: "1abc"
When:  VariableName::new("1abc") is called
Then:
  - Returns Err(VariableNameError::InvalidStart('1'))
```

#### Scenario VN-6: Contains hyphen rejected

```
Given: "my-var"
When:  VariableName::new("my-var") is called
Then:
  - Returns Err(VariableNameError::InvalidCharacter('-'))
```

#### Scenario VN-7: 129 characters rejected

```
Given: a string of 129 'a' characters
When:  VariableName::new(s) is called
Then:
  - Returns Err(VariableNameError::TooLong(129))
```

#### Scenario VN-8: Exactly 128 characters accepted

```
Given: a string of 128 'a' characters (starting with 'a')
When:  VariableName::new(s) is called
Then:
  - Returns Ok(VariableName)
```

#### Scenario VN-9: Contains space rejected

```
Given: "my var"
When:  VariableName::new("my var") is called
Then:
  - Returns Err(VariableNameError::InvalidCharacter(' '))
```

#### Scenario VN-10: Serde roundtrip

```
Given: VariableName::new("count").unwrap()
When:  serialized and deserialized via serde_json
Then:
  - JSON is "\"count\""
  - Roundtrips to equal value
```

---

### ImageRef

#### Scenario IR-1: Valid Docker image

```
Given: "docker.io/library/alpine:latest"
When:  ImageRef::new(s) is called
Then:
  - Returns Ok(ImageRef)
  - as_str() returns "docker.io/library/alpine:latest"
```

#### Scenario IR-2: Simple image name

```
Given: "ubuntu"
When:  ImageRef::new("ubuntu") is called
Then:
  - Returns Ok(ImageRef)
```

#### Scenario IR-3: Image with digest

```
Given: "alpine@sha256:abcdef1234567890"
When:  ImageRef::new(s) is called
Then:
  - Returns Ok(ImageRef)
```

#### Scenario IR-4: Empty rejected

```
Given: ""
When:  ImageRef::new("") is called
Then:
  - Returns Err(ImageRefError::Empty)
```

#### Scenario IR-5: Contains space rejected

```
Given: "my image"
When:  ImageRef::new("my image") is called
Then:
  - Returns Err(ImageRefError::ContainsWhitespace)
```

#### Scenario IR-6: Contains tab rejected

```
Given: "my\timage"
When:  ImageRef::new(s) is called
Then:
  - Returns Err(ImageRefError::ContainsWhitespace)
```

#### Scenario IR-7: Serde roundtrip

```
Given: ImageRef::new("nginx:1.25").unwrap()
When:  serialized and deserialized
Then:
  - JSON is "\"nginx:1.25\""
  - Roundtrips to equal value
```

---

### ShellScript

#### Scenario SS-1: Valid script

```
Given: "echo hello"
When:  ShellScript::new("echo hello") is called
Then:
  - Returns Ok(ShellScript)
  - as_str() returns "echo hello"
```

#### Scenario SS-2: Multi-line script

```
Given: "#!/bin/bash\necho hello\nexit 0"
When:  ShellScript::new(s) is called
Then:
  - Returns Ok(ShellScript) (newlines are valid shell content)
```

#### Scenario SS-3: Empty rejected

```
Given: ""
When:  ShellScript::new("") is called
Then:
  - Returns Err(ShellScriptError::Empty)
```

#### Scenario SS-4: Serde roundtrip

```
Given: ShellScript::new("ls -la").unwrap()
When:  serialized and deserialized
Then:
  - JSON is "\"ls -la\""
  - Roundtrips to equal value
```

---

### BackoffRate

#### Scenario BR-1: Valid rate

```
Given: f64 value 2.0
When:  BackoffRate::new(2.0) is called
Then:
  - Returns Ok(BackoffRate)
  - value() returns 2.0
```

#### Scenario BR-2: Small positive value (lower boundary)

```
Given: f64::MIN_POSITIVE (smallest positive f64)
When:  BackoffRate::new(f64::MIN_POSITIVE) is called
Then:
  - Returns Ok(BackoffRate)
```

#### Scenario BR-3: Zero rejected

```
Given: 0.0
When:  BackoffRate::new(0.0) is called
Then:
  - Returns Err(BackoffRateError::NotPositive(0.0))
```

#### Scenario BR-4: Negative rejected

```
Given: -1.5
When:  BackoffRate::new(-1.5) is called
Then:
  - Returns Err(BackoffRateError::NotPositive(-1.5))
```

#### Scenario BR-5: NaN rejected

```
Given: f64::NAN
When:  BackoffRate::new(f64::NAN) is called
Then:
  - Returns Err(BackoffRateError::NotFinite(f64::NAN))
```

#### Scenario BR-6: Positive infinity rejected

```
Given: f64::INFINITY
When:  BackoffRate::new(f64::INFINITY) is called
Then:
  - Returns Err(BackoffRateError::NotFinite(f64::INFINITY))
```

#### Scenario BR-7: Negative infinity rejected

```
Given: f64::NEG_INFINITY
When:  BackoffRate::new(f64::NEG_INFINITY) is called
Then:
  - Returns Err(BackoffRateError::NotFinite(f64::NEG_INFINITY))
```

#### Scenario BR-8: FromStr valid

```
Given: "1.5"
When:  "1.5".parse::<BackoffRate>() is called
Then:
  - Returns Ok(BackoffRate) where value() == 1.5
```

#### Scenario BR-9: FromStr non-numeric rejected

```
Given: "abc"
When:  "abc".parse::<BackoffRate>() is called
Then:
  - Returns Err(BackoffRateError::ParseError(_))
```

#### Scenario BR-10: FromStr zero rejected

```
Given: "0.0"
When:  "0.0".parse::<BackoffRate>() is called
Then:
  - Returns Err(BackoffRateError::NotPositive(0.0))
```

#### Scenario BR-11: Serde roundtrip

```
Given: BackoffRate::new(3.14).unwrap()
When:  serialized and deserialized via serde_json
Then:
  - JSON is "3.14" (bare number, transparent)
  - Deserializes back to equal BackoffRate
```

#### Scenario BR-12: Serde rejects zero on deserialize

```
Given: JSON "0.0"
When:  serde_json::from_str::<BackoffRate>("0.0") is called
Then:
  - Returns Err (not positive)
```

#### Scenario BR-13: Display

```
Given: BackoffRate::new(2.5).unwrap()
When:  to_string() is called
Then:
  - Returns "2.5"
```

#### Scenario BR-14: Negative zero rejected

```
Given: -0.0_f64
When:  BackoffRate::new(-0.0) is called
Then:
  - Returns Err(BackoffRateError::NotPositive(-0.0))
  - Note: -0.0 == 0.0 in IEEE 754, so this is treated as not positive
```

---

### ErrorCode

#### Scenario EC-1: Serialize known variants

```
Given: ErrorCode::All
When:  serde_json::to_string(&code) is called
Then:
  - Returns Ok("\"all\"")

Given: ErrorCode::TaskFailed
When:  serde_json::to_string(&code) is called
Then:
  - Returns Ok("\"taskfailed\"")

Given: ErrorCode::HeartbeatTimeout
When:  serde_json::to_string(&code) is called
Then:
  - Returns Ok("\"heartbeattimeout\"")
```

#### Scenario EC-2: Serialize Custom

```
Given: ErrorCode::Custom("MyApp.CustomError".to_owned())
When:  serde_json::to_string(&code) is called
Then:
  - Returns Ok("\"MyApp.CustomError\"") (raw string, no transformation)
```

#### Scenario EC-3: Deserialize known variants (case-insensitive)

```
Given: JSON "\"all\""
When:  serde_json::from_str::<ErrorCode>(s) is called
Then:
  - Returns Ok(ErrorCode::All)

Given: JSON "\"ALL\""
When:  serde_json::from_str::<ErrorCode>(s) is called
Then:
  - Returns Ok(ErrorCode::All)

Given: JSON "\"TaskFailed\""
When:  serde_json::from_str::<ErrorCode>(s) is called
Then:
  - Returns Ok(ErrorCode::TaskFailed)
```

#### Scenario EC-4: Deserialize unknown becomes Custom

```
Given: JSON "\"MyCustomError\""
When:  serde_json::from_str::<ErrorCode>(s) is called
Then:
  - Returns Ok(ErrorCode::Custom("MyCustomError".to_owned()))
```

#### Scenario EC-5: Display matches serialized form

```
Given: ErrorCode::Timeout
When:  to_string() is called
Then:
  - Returns "timeout"

Given: ErrorCode::Custom("Foo".to_owned())
When:  to_string() is called
Then:
  - Returns "Foo"
```

#### Scenario EC-6: FromStr known variant

```
Given: "timeout"
When:  "timeout".parse::<ErrorCode>() is called
Then:
  - Returns Ok(ErrorCode::Timeout)
```

#### Scenario EC-7: FromStr unknown variant

```
Given: "SomeRandomError"
When:  "SomeRandomError".parse::<ErrorCode>() is called
Then:
  - Returns Ok(ErrorCode::Custom("SomeRandomError".to_owned()))
```

#### Scenario EC-8: FromStr is infallible

```
Given: any string s
When:  s.parse::<ErrorCode>() is called
Then:
  - Always returns Ok(_)
  - type Err = std::convert::Infallible
```

#### Scenario EC-9: Clone and equality

```
Given: ErrorCode::BranchFailed
When:  code.clone() is called
Then:
  - Returns a value == original
  - code == code.clone()
```

#### Scenario EC-10: All known variants roundtrip through serde

```
Given: each of [All, Timeout, TaskFailed, Permissions, ResultPathMatchFailure,
        ParameterPathFailure, BranchFailed, NoChoiceMatched, IntrinsicFailure,
        HeartbeatTimeout]
When:  serialized to JSON string and deserialized back
Then:
  - Each deserializes to the same variant it started as
```

#### Scenario EC-11: Hash consistency

```
Given: ErrorCode::All and ErrorCode::All
When:  both are inserted into a HashSet
Then:
  - HashSet contains exactly 1 element (duplicates merged)
```

#### Scenario EC-12: Custom equality

```
Given: ErrorCode::Custom("A".to_owned()) and ErrorCode::Custom("B".to_owned())
When:  compared with ==
Then:
  - Returns false

Given: ErrorCode::Custom("A".to_owned()) and ErrorCode::Custom("A".to_owned())
When:  compared with ==
Then:
  - Returns true
```

---

## Validation Order Contract

For every `new()` constructor, validation checks MUST be performed in this order:

1. **Finiteness** (BackoffRate only): reject NaN/Infinity before range check
2. **Emptiness**: reject empty input first
3. **Length**: reject too-long input second
4. **Format**: reject malformed content last (invalid chars, missing prefix, etc.)

This ensures the most specific applicable error variant is returned. When multiple
violations exist (e.g., a 300-char string starting with a digit for VariableName),
the first check in order wins: `TooLong(300)` is returned, not `InvalidStart`.

---

## Module Structure Contract

```
crates/twerk-core/src/
  asl/
    mod.rs          -- pub mod types; pub mod error_code; re-exports
    types.rs        -- StateName, Expression, JsonPath, VariableName, ImageRef, ShellScript, BackoffRate
    error_code.rs   -- ErrorCode enum
```

- `asl/mod.rs` MUST re-export all public types and error types
- `lib.rs` MUST declare `pub mod asl;`
- All error types MUST use `thiserror::Error` derive macro (matching domain_types.rs pattern)
