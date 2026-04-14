# Contract Specification: twerk-d7p

## Context

- **Feature**: Refactor - Create Port, RetryLimit, Progress, and Quantity newtypes
- **Domain terms**: Network ports, retry mechanics, task progress tracking, count/position semantics
- **Assumptions**:
  - u16 range is 0-65535; valid TCP/UDP ports are 1-65535
  - f64 Progress uses percentage semantics (0.0-100.0 inclusive)
  - i64 TaskPosition supports negative values (e.g., relative offsets from end)
  - All newtypes are zero-cost abstractions over primitive types
- **Open questions**: None

---

## Type: Port

### Type Definition
```rust
pub struct Port(u16);
```

### Invariants
1. `self.0 >= 1 && self.0 <= 65535` — Port must be in valid TCP/UDP range

### Preconditions
- `new(value: u16) -> Result<Port, PortError>` fails if `value == 0 || value > 65535`

### Postconditions
- `Port::new(value).unwrap()` guarantees `port.value() == value`
- `port.value()` always returns a u16 in range 1..=65535

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| `PortError::OutOfRange` | value == 0 or value > 65535 | Invalid port number |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `u16` value
- `Deserialize` accepts raw `u16` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = u16>` — `*port` yields `u16`
- `AsRef<u16>` — `port.as_ref()` yields `&u16`

---

## Type: RetryLimit

### Type Definition
```rust
pub struct RetryLimit(u32);
```

### Invariants
1. `self.0 >= 0` — RetryLimit is non-negative (u32 is always >= 0)

### Preconditions
- `new(value: u32) -> Result<RetryLimit, RetryLimitError>` — always succeeds (u32 non-negative)
- `from_option(value: Option<u32>) -> Result<RetryLimit, RetryLimitError>` fails if `value.is_none()`

### Postconditions
- `RetryLimit::new(value).unwrap()` guarantees `retry_limit.value() == value`
- `retry_limit.value()` always returns a u32 >= 0

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| `RetryLimitError::NoneNotAllowed` | None passed to from_option | Optional limit must be present |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `u32` value
- `Deserialize` accepts raw `u32` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = u32>` — `*retry_limit` yields `u32`
- `AsRef<u32>` — `retry_limit.as_ref()` yields `&u32`

---

## Type: RetryAttempt

### Type Definition
```rust
pub struct RetryAttempt(u32);
```

### Invariants
1. `self.0 >= 0` — RetryAttempt is non-negative (u32 is always >= 0)

### Preconditions
- `new(value: u32) -> Result<RetryAttempt, RetryAttemptError>` — always succeeds (u32 non-negative)

### Postconditions
- `RetryAttempt::new(value).unwrap()` guarantees `attempt.value() == value`
- `attempt.value()` always returns a u32 >= 0

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| (no failure variants) | — | Construction cannot fail for u32 |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `u32` value
- `Deserialize` accepts raw `u32` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = u32>` — `*attempt` yields `u32`
- `AsRef<u32>` — `attempt.as_ref()` yields `&u32`

---

## Type: Progress

### Type Definition
```rust
pub struct Progress(f64);
```

### Invariants
1. `self.0 >= 0.0 && self.0 <= 100.0` — Progress must be percentage range
2. `!self.0.is_nan()` — Progress must not be NaN

### Preconditions
- `new(value: f64) -> Result<Progress, ProgressError>` fails if `value < 0.0 || value > 100.0 || value.is_nan()`

### Postconditions
- `Progress::new(value).unwrap()` guarantees `progress.value() == value`
- `progress.value()` always returns f64 in range 0.0..=100.0 and not NaN

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| `ProgressError::OutOfRange` | value < 0.0 or value > 100.0 | Percentage outside valid bounds |
| `ProgressError::NaN` | value.is_nan() | NaN is not a valid progress |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `f64` value
- `Deserialize` accepts raw `f64` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = f64>` — `*progress` yields `f64`
- `AsRef<f64>` — `progress.as_ref()` yields `&f64`

---

## Type: TaskCount

### Type Definition
```rust
pub struct TaskCount(u32);
```

### Invariants
1. `self.0 >= 0` — TaskCount is non-negative (u32 is always >= 0)

### Preconditions
- `new(value: u32) -> Result<TaskCount, TaskCountError>` — always succeeds (u32 non-negative)
- `from_option(value: Option<u32>) -> Result<TaskCount, TaskCountError>` fails if `value.is_none()`

### Postconditions
- `TaskCount::new(value).unwrap()` guarantees `count.value() == value`
- `count.value()` always returns a u32 >= 0

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| `TaskCountError::NoneNotAllowed` | None passed to from_option | Optional count must be present |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `u32` value
- `Deserialize` accepts raw `u32` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = u32>` — `*count` yields `u32`
- `AsRef<u32>` — `count.as_ref()` yields `&u32`

---

## Type: TaskPosition

### Type Definition
```rust
pub struct TaskPosition(i64);
```

### Invariants
1. No range restriction — TaskPosition can be any i64 including negative

### Preconditions
- `new(value: i64) -> Result<TaskPosition, TaskPositionError>` — always succeeds

### Postconditions
- `TaskPosition::new(value).unwrap()` guarantees `position.value() == value`
- `position.value()` always returns the original i64

### Error Taxonomy
| Variant | Condition | Semantics |
|---------|-----------|-----------|
| (no failure variants) | — | Construction cannot fail for i64 |

### Serialization Contract
- `#[derive(Serialize, Deserialize)]` — transparent, must round-trip identically
- `Serialize` outputs raw `i64` value
- `Deserialize` accepts raw `i64` and validates via constructor

### Deref/AsRef Contract
- `Deref<Target = i64>` — `*position` yields `i64`
- `AsRef<i64>` — `position.as_ref()` yields `&i64`

---

## Shared Error Enum Pattern

All error types follow consistent structure:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XxxError {
    OutOfRange { value: OriginalType, min: OriginalType, max: OriginalType },
    NoneNotAllowed,
    NaN,
}
```

Implement:
- `std::fmt::Display` — human-readable message
- `std::error::Error` — `source()` returns None
- `From<value_type>` conversions where applicable

---

## Common Precondition Checklist

For each newtype constructor `new(T) -> Result<Newtype, Error>`:
- [x] Range validation (Port: 1-65535, Progress: 0.0-100.0)
- [x] NaN rejection (Progress)
- [x] None rejection (RetryLimit, TaskCount via from_option)

---

## Common Postcondition Checklist

For each newtype:
- [x] `value()` accessor returns wrapped primitive
- [x] `PartialEq` comparison between same types works via inner equality
- [x] `Debug` formats as `Newtype(inner)`
- [x] `Display` formats inner value

---

## Non-goals

- No validation beyond stated invariants
- No business logic in types themselves
- No `From<Newtype>` implementations for primitive types (Deref provides implicit conversion)
- No custom serialization formats — transparent only
