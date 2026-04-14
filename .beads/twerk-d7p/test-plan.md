# Test Plan: twerk-d7p — Newtype Refactor (Port, RetryLimit, RetryAttempt, Progress, TaskCount, TaskPosition)

## Summary

- **Bead**: twerk-d7p
- **Behaviors identified**: 64
- **Trophy allocation**: 60 unit / 23 integration / 2 e2e / 2 static
- **Proptest invariants**: 8
- **Fuzz targets**: 6
- **Kani harnesses**: 4
- **Mutation checkpoints**: 12 critical mutations
- **Target mutation kill rate**: ≥90%

---

## 1. Behavior Inventory

### Port (u16)

1. **Port constructor accepts valid u16** — `Port::new(value)` returns `Ok(Port)` when `1 <= value <= 65535`
2. **Port constructor rejects zero** — `Port::new(0)` returns `Err(PortError::OutOfRange)` with value 0
3. **Port constructor rejects value > 65535** — `Port::new(65536)` returns `Err(PortError::OutOfRange)`
4. **Port minimum boundary** — `Port::new(1)` returns `Ok(Port)` with value 1
5. **Port maximum boundary** — `Port::new(65535)` returns `Ok(Port)` with value 65535
6. **Port value accessor** — `port.value()` returns the inner `u16`
7. **Port deref to u16** — `*port` yields `u16`
8. **Port AsRef<u16>** — `port.as_ref()` yields `&u16`
9. **Port debug format** — `format!("{:?}", port)` contains `"Port("`
10. **Port display format** — `format!("{}", port)` shows the raw u16 value
11. **Port serialization round-trip** — `serde_json::from_slice::<Port>(&serde_json::to_vec(&port).unwrap())` equals original
12. **Port equality** — two `Port::new(80).unwrap()` instances are equal

### RetryLimit (u32)

13. **RetryLimit constructor always succeeds** — `RetryLimit::new(value)` always returns `Ok(RetryLimit)` for any u32
14. **RetryLimit from_option accepts Some** — `RetryLimit::from_option(Some(3))` returns `Ok(RetryLimit)`
15. **RetryLimit from_option rejects None** — `RetryLimit::from_option(None)` returns `Err(RetryLimitError::NoneNotAllowed)`
16. **RetryLimit value accessor** — `retry_limit.value()` returns the inner `u32`
17. **RetryLimit deref to u32** — `*retry_limit` yields `u32`
18. **RetryLimit AsRef<u32>** — `retry_limit.as_ref()` yields `&u32`
19. **RetryLimit serialization round-trip** — round-trip through JSON equals original
20. **RetryLimit equality** — two `RetryLimit::new(5).unwrap()` instances are equal

### RetryAttempt (u32)

21. **RetryAttempt constructor always succeeds** — `RetryAttempt::new(value)` always returns `Ok(RetryAttempt)` for any u32
22. **RetryAttempt value accessor** — `attempt.value()` returns the inner `u32`
23. **RetryAttempt deref to u32** — `*attempt` yields `u32`
24. **RetryAttempt AsRef<u32>** — `attempt.as_ref()` yields `&u32`
25. **RetryAttempt serialization round-trip** — round-trip through JSON equals original
26. **RetryAttempt equality** — two `RetryAttempt::new(1).unwrap()` instances are equal

### Progress (f64)

27. **Progress constructor accepts valid range** — `Progress::new(value)` returns `Ok(Progress)` when `0.0 <= value <= 100.0`
28. **Progress constructor rejects negative** — `Progress::new(-0.001)` returns `Err(ProgressError::OutOfRange)`
29. **Progress constructor rejects > 100.0** — `Progress::new(100.001)` returns `Err(ProgressError::OutOfRange)`
30. **Progress constructor rejects NaN** — `Progress::new(f64::NAN)` returns `Err(ProgressError::NaN)`
31. **Progress minimum boundary zero** — `Progress::new(0.0)` returns `Ok(Progress)` with value 0.0
32. **Progress maximum boundary 100.0** — `Progress::new(100.0)` returns `Ok(Progress)` with value 100.0
33. **Progress value accessor** — `progress.value()` returns the inner `f64`
34. **Progress deref to f64** — `*progress` yields `f64`
35. **Progress AsRef<f64>** — `progress.as_ref()` yields `&f64`
36. **Progress serialization round-trip** — round-trip through JSON equals original
37. **Progress equality** — two `Progress::new(50.0).unwrap()` instances are equal

### TaskCount (u32)

38. **TaskCount constructor always succeeds** — `TaskCount::new(value)` always returns `Ok(TaskCount)` for any u32
39. **TaskCount from_option accepts Some** — `TaskCount::from_option(Some(10))` returns `Ok(TaskCount)`
40. **TaskCount from_option rejects None** — `TaskCount::from_option(None)` returns `Err(TaskCountError::NoneNotAllowed)`
41. **TaskCount value accessor** — `count.value()` returns the inner `u32`
42. **TaskCount deref to u32** — `*count` yields `u32`
43. **TaskCount AsRef<u32>** — `count.as_ref()` yields `&u32`
44. **TaskCount serialization round-trip** — round-trip through JSON equals original
45. **TaskCount equality** — two `TaskCount::new(7).unwrap()` instances are equal

### TaskPosition (i64)

46. **TaskPosition constructor always succeeds** — `TaskPosition::new(value)` always returns `Ok(TaskPosition)` for any i64
47. **TaskPosition value accessor** — `position.value()` returns the inner `i64`
48. **TaskPosition deref to i64** — `*position` yields `i64`
49. **TaskPosition AsRef<i64>** — `position.as_ref()` yields `&i64`
50. **TaskPosition serialization round-trip** — round-trip through JSON equals original
51. **TaskPosition equality** — two `TaskPosition::new(42).unwrap()` instances are equal
52. **TaskPosition negative values allowed** — `TaskPosition::new(-1)` returns `Ok(TaskPosition)` with value -1

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Static** | 2 | `clippy::pedantic`, `cargo-deny` — zero-cost, catches entire classes of bugs at compile time. Apply to all 6 newtypes. |
| **Unit** | 60 | Pure constructor/validation logic + trait implementations (Deref, AsRef, Display, Debug, PartialEq). Each of the 52 behaviors gets dedicated unit test. Additional boundary and edge-case tests reach 5× density (60 tests for 12 public functions). |
| **Integration** | 23 | Serde round-trip (real serialization), trait composition chains, error Display equality, E2E round-trips through public API. |
| **E2E** | 2 | Full round-trip from typed `Port` → JSON bytes → parsed back to `Port` through public API (no direct module access). |

**Target ratio**: 68% integration / 23% unit / 3% e2e / 3% static — skewed toward integration as per Testing Trophy, but unit density meets the ≥5× contract requirement.

---

## 3. BDD Scenarios

### Type: Port

#### Behavior: Port constructor accepts valid u16 when value is in range 1..=65535
```
Given: no prior Port instance
When: Port::new(8080) is called
Then: Result is Ok(Port) and port.value() == 8080
```

#### Behavior: Port constructor rejects when value is zero
```
Given: no prior Port instance
When: Port::new(0) is called
Then: Result is Err(PortError::OutOfRange) with value == 0 and min == 1 and max == 65535
```

#### Behavior: Port constructor rejects when value exceeds 65535
```
Given: no prior Port instance
When: Port::new(65536) is called
Then: Result is Err(PortError::OutOfRange) with value == 65536 and min == 1 and max == 65535
```

#### Behavior: Port constructor rejects when value is far out of range
```
Given: no prior Port instance
When: Port::new(100000) is called
Then: Result is Err(PortError::OutOfRange) with value == 100000 and min == 1 and max == 65535
```

#### Behavior: Port minimum boundary is 1
```
Given: no prior Port instance
When: Port::new(1) is called
Then: Result is Ok(Port) and port.value() == 1
```

#### Behavior: Port accepts middle value 32768
```
Given: no prior Port instance
When: Port::new(32768) is called
Then: Result is Ok(Port) and port.value() == 32768
```

#### Behavior: Port maximum boundary is 65535
```
Given: no prior Port instance
When: Port::new(65535) is called
Then: Result is Ok(Port) and port.value() == 65535
```

#### Behavior: Port value accessor returns inner u16
```
Given: Port::new(443).unwrap() as port
When: port.value() is called
Then: returns 443u16
```

#### Behavior: Port deref yields u16
```
Given: Port::new(80).unwrap() as port
When: *port is dereferenced
Then: yields 80u16
```

#### Behavior: Port AsRef yields reference to u16
```
Given: Port::new(80).unwrap() as port
When: port.as_ref() is called
Then: yields &80u16
```

#### Behavior: Port debug format contains type name
```
Given: Port::new(22).unwrap() as port
When: format!("{:?}", port) is called
Then: string contains "Port("
```

#### Behavior: Port display format shows raw value
```
Given: Port::new(22).unwrap() as port
When: format!("{}", port) is called
Then: string equals "22"
```

#### Behavior: Port serialization round-trips correctly
```
Given: Port::new(8888).unwrap() as port
When: port is serialized to JSON then deserialized
Then: Result equals original Port(8888)
```

#### Behavior: Port equality holds for same values
```
Given: Port::new(80).unwrap() as p1 and Port::new(80).unwrap() as p2
When: p1 == p2 is evaluated
Then: result is true
```

#### Behavior: Port inequality holds for different values
```
Given: Port::new(80).unwrap() as p1 and Port::new(8080).unwrap() as p2
When: p1 == p2 is evaluated
Then: result is false
```

#### Behavior: PortError::OutOfRange Display shows value and bounds
```
Given: Port::new(0).unwrap_err() as err
When: format!("{}", err) is called
Then: string equals "Port 0 out of valid range 1..=65535"
```

#### Behavior: PortError::OutOfRange equality
```
Given: Port::new(0).unwrap_err() as err1 and Port::new(0).unwrap_err() as err2
When: err1 == err2 is evaluated
Then: result is true
```

---

### Type: RetryLimit

#### Behavior: RetryLimit constructor always succeeds for any u32
```
Given: no prior RetryLimit instance
When: RetryLimit::new(0) is called
Then: Result is Ok(RetryLimit) and retry_limit.value() == 0
```

#### Behavior: RetryLimit constructor succeeds at u32::MAX
```
Given: no prior RetryLimit instance
When: RetryLimit::new(4294967295) is called
Then: Result is Ok(RetryLimit) and retry_limit.value() == 4294967295
```

#### Behavior: RetryLimit from_option accepts Some value
```
Given: no prior RetryLimit instance
When: RetryLimit::from_option(Some(3)) is called
Then: Result is Ok(RetryLimit) and retry_limit.value() == 3
```

#### Behavior: RetryLimit from_option rejects None
```
Given: no prior RetryLimit instance
When: RetryLimit::from_option(None) is called
Then: Result is Err(RetryLimitError::NoneNotAllowed)
```

#### Behavior: RetryLimit value accessor returns inner u32
```
Given: RetryLimit::new(5).unwrap() as retry_limit
When: retry_limit.value() is called
Then: returns 5u32
```

#### Behavior: RetryLimit deref yields u32
```
Given: RetryLimit::new(10).unwrap() as retry_limit
When: *retry_limit is dereferenced
Then: yields 10u32
```

#### Behavior: RetryLimit AsRef yields reference to u32
```
Given: RetryLimit::new(10).unwrap() as retry_limit
When: retry_limit.as_ref() is called
Then: yields &10u32
```

#### Behavior: RetryLimit serialization round-trips correctly
```
Given: RetryLimit::new(7).unwrap() as retry_limit
When: retry_limit is serialized to JSON then deserialized
Then: Result equals original RetryLimit(7)
```

#### Behavior: RetryLimit equality holds for same values
```
Given: RetryLimit::new(5).unwrap() as r1 and RetryLimit::new(5).unwrap() as r2
When: r1 == r2 is evaluated
Then: result is true
```

#### Behavior: RetryLimitError::NoneNotAllowed Display
```
Given: RetryLimit::from_option(None).unwrap_err() as err
When: format!("{}", err) is called
Then: string equals "Optional retry limit must be present"
```

#### Behavior: RetryLimitError::NoneNotAllowed equality
```
Given: RetryLimit::from_option(None).unwrap_err() as err1 and RetryLimit::from_option(None).unwrap_err() as err2
When: err1 == err2 is evaluated
Then: result is true
```

---

### Type: RetryAttempt

#### Behavior: RetryAttempt constructor always succeeds for any u32
```
Given: no prior RetryAttempt instance
When: RetryAttempt::new(0) is called
Then: Result is Ok(RetryAttempt) and attempt.value() == 0
```

#### Behavior: RetryAttempt constructor succeeds at u32::MAX
```
Given: no prior RetryAttempt instance
When: RetryAttempt::new(4294967295) is called
Then: Result is Ok(RetryAttempt) and attempt.value() == 4294967295
```

#### Behavior: RetryAttempt constructor succeeds at mid-range
```
Given: no prior RetryAttempt instance
When: RetryAttempt::new(2147483647) is called
Then: Result is Ok(RetryAttempt) and attempt.value() == 2147483647
```

#### Behavior: RetryAttempt value accessor returns inner u32
```
Given: RetryAttempt::new(1).unwrap() as attempt
When: attempt.value() is called
Then: returns 1u32
```

#### Behavior: RetryAttempt deref yields u32
```
Given: RetryAttempt::new(1).unwrap() as attempt
When: *attempt is dereferenced
Then: yields 1u32
```

#### Behavior: RetryAttempt AsRef yields reference to u32
```
Given: RetryAttempt::new(1).unwrap() as attempt
When: attempt.as_ref() is called
Then: yields &1u32
```

#### Behavior: RetryAttempt serialization round-trips correctly
```
Given: RetryAttempt::new(4).unwrap() as attempt
When: attempt is serialized to JSON then deserialized
Then: Result equals original RetryAttempt(4)
```

#### Behavior: RetryAttempt equality holds for same values
```
Given: RetryAttempt::new(2).unwrap() as a1 and RetryAttempt::new(2).unwrap() as a2
When: a1 == a2 is evaluated
Then: result is true
```

---

### Type: Progress

#### Behavior: Progress constructor accepts valid range when value is 0.0..=100.0
```
Given: no prior Progress instance
When: Progress::new(50.0) is called
Then: Result is Ok(Progress) and progress.value() == 50.0
```

#### Behavior: Progress constructor rejects negative value
```
Given: no prior Progress instance
When: Progress::new(-0.001) is called
Then: Result is Err(ProgressError::OutOfRange) with value == -0.001
```

#### Behavior: Progress constructor rejects value greater than 100.0
```
Given: no prior Progress instance
When: Progress::new(100.001) is called
Then: Result is Err(ProgressError::OutOfRange) with value == 100.001
```

#### Behavior: Progress constructor rejects infinity
```
Given: no prior Progress instance
When: Progress::new(f64::INFINITY) is called
Then: Result is Err(ProgressError::OutOfRange) with value == inf
```

#### Behavior: Progress constructor rejects NaN
```
Given: no prior Progress instance
When: Progress::new(f64::NAN) is called
Then: Result is Err(ProgressError::NaN)
```

#### Behavior: Progress minimum boundary is 0.0
```
Given: no prior Progress instance
When: Progress::new(0.0) is called
Then: Result is Ok(Progress) and progress.value() == 0.0
```

#### Behavior: Progress maximum boundary is 100.0
```
Given: no prior Progress instance
When: Progress::new(100.0) is called
Then: Result is Ok(Progress) and progress.value() == 100.0
```

#### Behavior: Progress accepts subnormal positive value
```
Given: no prior Progress instance
When: Progress::new(0.0000001) is called
Then: Result is Ok(Progress) and progress.value() == 0.0000001
```

#### Behavior: Progress accepts value just below 100.0
```
Given: no prior Progress instance
When: Progress::new(99.9999) is called
Then: Result is Ok(Progress) and progress.value() == 99.9999
```

#### Behavior: Progress value accessor returns inner f64
```
Given: Progress::new(75.5).unwrap() as progress
When: progress.value() is called
Then: returns 75.5f64
```

#### Behavior: Progress deref yields f64
```
Given: Progress::new(33.3).unwrap() as progress
When: *progress is dereferenced
Then: yields 33.3f64
```

#### Behavior: Progress AsRef yields reference to f64
```
Given: Progress::new(33.3).unwrap() as progress
When: progress.as_ref() is called
Then: yields &33.3f64
```

#### Behavior: Progress serialization round-trips correctly
```
Given: Progress::new(62.5).unwrap() as progress
When: progress is serialized to JSON then deserialized
Then: Result equals original Progress(62.5)
```

#### Behavior: Progress equality holds for same values
```
Given: Progress::new(50.0).unwrap() as p1 and Progress::new(50.0).unwrap() as p2
When: p1 == p2 is evaluated
Then: result is true
```

#### Behavior: ProgressError::OutOfRange Display for negative
```
Given: Progress::new(-0.001).unwrap_err() as err
When: format!("{}", err) is called
Then: string equals "Progress -0.001 out of valid range 0.0..=100.0"
```

#### Behavior: ProgressError::NaN Display
```
Given: Progress::new(f64::NAN).unwrap_err() as err
When: format!("{}", err) is called
Then: string contains "NaN"
```

#### Behavior: ProgressError equality
```
Given: Progress::new(-0.001).unwrap_err() as err1 and Progress::new(-0.001).unwrap_err() as err2
When: err1 == err2 is evaluated
Then: result is true
```

---

### Type: TaskCount

#### Behavior: TaskCount constructor always succeeds for any u32
```
Given: no prior TaskCount instance
When: TaskCount::new(0) is called
Then: Result is Ok(TaskCount) and count.value() == 0
```

#### Behavior: TaskCount constructor succeeds at u32::MAX
```
Given: no prior TaskCount instance
When: TaskCount::new(4294967295) is called
Then: Result is Ok(TaskCount) and count.value() == 4294967295
```

#### Behavior: TaskCount from_option accepts Some value
```
Given: no prior TaskCount instance
When: TaskCount::from_option(Some(10)) is called
Then: Result is Ok(TaskCount) and count.value() == 10
```

#### Behavior: TaskCount from_option rejects None
```
Given: no prior TaskCount instance
When: TaskCount::from_option(None) is called
Then: Result is Err(TaskCountError::NoneNotAllowed)
```

#### Behavior: TaskCount value accessor returns inner u32
```
Given: TaskCount::new(7).unwrap() as count
When: count.value() is called
Then: returns 7u32
```

#### Behavior: TaskCount deref yields u32
```
Given: TaskCount::new(7).unwrap() as count
When: *count is dereferenced
Then: yields 7u32
```

#### Behavior: TaskCount AsRef yields reference to u32
```
Given: TaskCount::new(7).unwrap() as count
When: count.as_ref() is called
Then: yields &7u32
```

#### Behavior: TaskCount serialization round-trips correctly
```
Given: TaskCount::new(42).unwrap() as count
When: count is serialized to JSON then deserialized
Then: Result equals original TaskCount(42)
```

#### Behavior: TaskCount equality holds for same values
```
Given: TaskCount::new(7).unwrap() as c1 and TaskCount::new(7).unwrap() as c2
When: c1 == c2 is evaluated
Then: result is true
```

#### Behavior: TaskCountError::NoneNotAllowed Display
```
Given: TaskCount::from_option(None).unwrap_err() as err
When: format!("{}", err) is called
Then: string equals "Optional task count must be present"
```

#### Behavior: TaskCountError equality
```
Given: TaskCount::from_option(None).unwrap_err() as err1 and TaskCount::from_option(None).unwrap_err() as err2
When: err1 == err2 is evaluated
Then: result is true
```

---

### Type: TaskPosition

#### Behavior: TaskPosition constructor always succeeds for any i64
```
Given: no prior TaskPosition instance
When: TaskPosition::new(0) is called
Then: Result is Ok(TaskPosition) and position.value() == 0
```

#### Behavior: TaskPosition accepts negative values
```
Given: no prior TaskPosition instance
When: TaskPosition::new(-1) is called
Then: Result is Ok(TaskPosition) and position.value() == -1
```

#### Behavior: TaskPosition accepts i64::MIN
```
Given: no prior TaskPosition instance
When: TaskPosition::new(-9223372036854775808) is called
Then: Result is Ok(TaskPosition) and position.value() == -9223372036854775808
```

#### Behavior: TaskPosition accepts i64::MAX
```
Given: no prior TaskPosition instance
When: TaskPosition::new(9223372036854775807) is called
Then: Result is Ok(TaskPosition) and position.value() == 9223372036854775807
```

#### Behavior: TaskPosition value accessor returns inner i64
```
Given: TaskPosition::new(99).unwrap() as position
When: position.value() is called
Then: returns 99i64
```

#### Behavior: TaskPosition deref yields i64
```
Given: TaskPosition::new(99).unwrap() as position
When: *position is dereferenced
Then: yields 99i64
```

#### Behavior: TaskPosition AsRef yields reference to i64
```
Given: TaskPosition::new(99).unwrap() as position
When: position.as_ref() is called
Then: yields &99i64
```

#### Behavior: TaskPosition serialization round-trips correctly
```
Given: TaskPosition::new(-5).unwrap() as position
When: position is serialized to JSON then deserialized
Then: Result equals original TaskPosition(-5)
```

#### Behavior: TaskPosition equality holds for same values
```
Given: TaskPosition::new(42).unwrap() as p1 and TaskPosition::new(42).unwrap() as p2
When: p1 == p2 is evaluated
Then: result is true
```

#### Behavior: TaskPosition Display shows raw i64
```
Given: TaskPosition::new(-123).unwrap() as position
When: format!("{}", position) is called
Then: string equals "-123"
```

#### Behavior: TaskPosition Debug format contains type name
```
Given: TaskPosition::new(5).unwrap() as position
When: format!("{:?}", position) is called
Then: string contains "TaskPosition("
```

---

## 4. Proptest Invariants

### Proptest: Port round-trip through constructor preserves value
```
Invariant: For any u16 value in 1..=65535, Port::new(v).unwrap().value() == v
Strategy: any u16 in range 1..=65535 (use union of [1..=1], [2..=65534], [65535..=65535])
Anti-invariant: values 0 and 65536 are handled by error path, not this invariant
```

### Proptest: Port error variant carries correct metadata
```
Invariant: Port::new(0).unwrap_err() has OutOfRange variant with value==0 and min==1 and max==65535
Strategy: constant inputs 0 and 65536
```

### Proptest: RetryLimit round-trip through constructor preserves value
```
Invariant: For any u32, RetryLimit::new(v).unwrap().value() == v
Strategy: any u32 (full u32 range)
```

### Proptest: RetryAttempt round-trip through constructor preserves value
```
Invariant: For any u32, RetryAttempt::new(v).unwrap().value() == v
Strategy: any u32 (full u32 range)
```

### Proptest: Progress round-trip through constructor preserves value
```
Invariant: For any f64 value in 0.0..=100.0 (excluding NaN), Progress::new(v).unwrap().value() == v
Strategy: any f64 in range 0.0..=100.0 (use specific range, not arbitrary f64 which includes NaN/inf)
Anti-invariant: NaN values are handled by error path, infinity is out of range
```

### Proptest: Progress error variants carry correct metadata
```
Invariant: Progress::new(f64::NAN).unwrap_err() has NaN variant (no metadata needed)
           Progress::new(-0.001).unwrap_err() has OutOfRange variant with value < 0.0
           Progress::new(100.001).unwrap_err() has OutOfRange variant with value > 100.0
Strategy: constant inputs f64::NAN, -0.001, 100.001
```

### Proptest: TaskCount round-trip through constructor preserves value
```
Invariant: For any u32, TaskCount::new(v).unwrap().value() == v
Strategy: any u32 (full u32 range)
```

### Proptest: TaskPosition round-trip through constructor preserves value
```
Invariant: For any i64, TaskPosition::new(v).unwrap().value() == v
Strategy: any i64 (full i64 range, including i64::MIN, i64::MAX, 0, -1)
```

---

## 5. Fuzz Targets

### Fuzz Target: Port deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, wrong validation bounds, or type confusion (string vs number)
Corpus seeds:
  - "80" (valid port)
  - "0" (invalid: zero)
  - "65535" (valid max)
  - "65536" (invalid: out of range)
  - "-1" (invalid: negative)
  - "8080\r\n" (trailing whitespace)
  - "" (empty)
```

### Fuzz Target: RetryLimit deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, type confusion with non-u32 numbers
Corpus seeds:
  - "0" (valid zero)
  - "3" (valid)
  - "4294967295" (u32::MAX)
  - "" (empty)
  - "null" (should this error? — from_option would fail)
```

### Fuzz Target: RetryAttempt deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, type confusion
Corpus seeds:
  - "0" (valid zero)
  - "1" (valid)
  - "100" (valid)
```

### Fuzz Target: Progress deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, NaN propagation, out-of-range bounds
Corpus seeds:
  - "0.0" (valid min)
  - "50.0" (valid middle)
  - "100.0" (valid max)
  - "-0.001" (invalid: negative)
  - "100.001" (invalid: over max)
  - "NaN" (invalid: NaN)
  - "null" (invalid: null)
  - "Infinity" (invalid: out of range)
```

### Fuzz Target: TaskCount deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, type confusion with non-u32 numbers
Corpus seeds:
  - "0" (valid zero)
  - "10" (valid)
  - "1000" (valid)
```

### Fuzz Target: TaskPosition deserialization from raw JSON bytes
```
Input type: arbitrary bytes interpreted as JSON number string
Risk: Panic on invalid JSON, type confusion with non-i64 numbers, negative number handling
Corpus seeds:
  - "0" (valid zero)
  - "-1" (valid negative)
  - "1" (valid positive)
  - "-9223372036854775808" (i64::MIN)
  - "9223372036854775807" (i64::MAX)
```

---

## 6. Kani Harnesses

### Kani Harness: Port constructor validation is exhaustive
```
Property: Port::new(v) returns Ok only when 1 <= v <= 65535; Err otherwise
Bound: v in 0..=65536 (three critical values: 0, 1, 65535, 65536, plus interior values)
Rationale: Formal proof that the boundary conditions are exactly at 1 and 65535. Unit tests can check specific values; Kani proves no other u16 values slip through.
```

### Kani Harness: Progress constructor validation is exhaustive
```
Property: Progress::new(v) returns Ok only when 0.0 <= v <= 100.0 and !v.is_nan(); Err otherwise
Bound: v in {NaN, -inf, -0.001, 0.0, 0.001, 99.999, 100.0, 100.001, inf} — representative set
Rationale: f64 has many special values (NaN, +/-inf, -0.0, denormals). Kani proves the exact boundary without floating-point edge-case gaps.
```

### Kani Harness: All six newtypes maintain inner value identity through Deref
```
Property: For each newtype T with inner type U: for all valid constructor inputs v,
          *T::new(v).unwrap() == v  (via Deref coercion)
Bound: One representative value per type (Port: 80, RetryLimit: 5, RetryAttempt: 1, Progress: 50.0, TaskCount: 7, TaskPosition: -3)
Rationale: Ensures Deref implementation does not mutate or misinterpret the inner value.
```

### Kani Harness: Port arithmetic cannot overflow u16 in basic operations
```
Property: For any two valid Port values p1, p2: p1.value() + p2.value() fits in u32 without overflow in common operations
Bound: p1, p2 in 1..=65535
Rationale: When ports are used in network operations, arithmetic on them must be safe. u16 addition can overflow (max result 131070). Proving no overflow in bounded u16 arithmetic.
```

---

## 7. Mutation Testing Checkpoints

### Critical Mutations to Survive

| Mutation | Target | Must Be Caught By |
|----------|--------|-------------------|
| Change `value == 0` to `value < 1` in Port | `Port::new` | `fn port_returns_err_when_value_is_zero` |
| Change `value > 65535` to `value >= 65535` in Port | `Port::new` | `fn port_returns_err_when_value_exceeds_65535` |
| Change `value < 0.0` to `value <= 0.0` in Progress | `Progress::new` | `fn progress_returns_err_when_value_is_negative` |
| Change `value > 100.0` to `value >= 100.0` in Progress | `Progress::new` | `fn progress_returns_err_when_value_exceeds_100` |
| Remove `!value.is_nan()` check in Progress | `Progress::new` | `fn progress_returns_err_when_value_is_nan` |
| Change `None` check to `Some` in RetryLimit::from_option | `RetryLimit::from_option` | `fn retry_limit_returns_err_when_none_passed` |
| Change `None` check to `Some` in TaskCount::from_option | `TaskCount::from_option` | `fn task_count_returns_err_when_none_passed` |
| Swap `min`/`max` values in PortError::OutOfRange display | `PortError::Display` | `fn port_error_display_shows_correct_min_max` |
| Change Progress error variant ordering | `ProgressError` enum | `fn progress_error_display_shows_out_of_range_for_negative` |
| Remove `#[derive(PartialEq)]` from any newtype | `T::eq` | `fn retry_limit_equals_same_value` |
| Change `Deref` target to wrong primitive | `Port Deref` | `fn port_deref_yields_u16` |
| Change `AsRef` target to wrong primitive | `Port AsRef` | `fn port_asref_yields_u16_ref` |

**Mutation kill rate threshold**: ≥90%

---

## 8. Combinatorial Coverage Matrix

### Port

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: valid middle | 80u16 | Ok(Port(80)) | unit |
| happy path: minimum boundary | 1u16 | Ok(Port(1)) | unit |
| happy path: maximum boundary | 65535u16 | Ok(Port(65535)) | unit |
| error: zero | 0u16 | Err(PortError::OutOfRange) | unit |
| error: exceeds max | 65536u16 | Err(PortError::OutOfRange) | unit |
| error: far out of range | 100000u16 | Err(PortError::OutOfRange) | unit |
| accessor: value() | Port(443) | 443u16 | unit |
| Deref: *port | Port(80) | 80u16 | unit |
| AsRef: as_ref() | Port(80) | &80u16 | unit |
| Display | Port(22) | "22" | unit |
| Debug | Port(22) | contains "Port(" | unit |
| serde round-trip | Port(8888) | Port(8888) | integration |
| serde round-trip: min | Port(1) | Port(1) | integration |
| serde round-trip: max | Port(65535) | Port(65535) | integration |
| equality: same | Port(80), Port(80) | true | unit |
| equality: different | Port(80), Port(8080) | false | unit |
| error Display | Port(0).unwrap_err() | contains "0" and "1" and "65535" | unit |
| error equality | Port(0).unwrap_err(), Port(0).unwrap_err() | true | unit |
| proptest invariant | any u16 1..=65535 | value() == original | proptest |

### RetryLimit

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: new valid | u32::new(5) | Ok(RetryLimit(5)) | unit |
| happy path: new at max | u32::MAX | Ok(RetryLimit(u32::MAX)) | unit |
| happy path: from_option Some | Some(3) | Ok(RetryLimit(3)) | unit |
| error: from_option None | None | Err(RetryLimitError::NoneNotAllowed) | unit |
| accessor: value() | RetryLimit(5) | 5u32 | unit |
| Deref | RetryLimit(10) | 10u32 | unit |
| AsRef | RetryLimit(10) | &10u32 | unit |
| Display | RetryLimit(7) | "7" | unit |
| serde round-trip | RetryLimit(7) | RetryLimit(7) | integration |
| equality: same | RetryLimit(5), RetryLimit(5) | true | unit |
| error Display | RetryLimit::from_option(None).unwrap_err() | "Optional retry limit must be present" | unit |
| error equality | RetryLimit::from_option(None).unwrap_err(), x2 | true | unit |
| proptest invariant | any u32 | value() == original | proptest |

### RetryAttempt

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: new valid | u32::new(0) | Ok(RetryAttempt(0)) | unit |
| happy path: new at max | u32::MAX | Ok(RetryAttempt(u32::MAX)) | unit |
| happy path: new mid | u32::new(2147483647) | Ok(RetryAttempt(2147483647)) | unit |
| accessor: value() | RetryAttempt(1) | 1u32 | unit |
| Deref | RetryAttempt(1) | 1u32 | unit |
| AsRef | RetryAttempt(1) | &1u32 | unit |
| serde round-trip | RetryAttempt(4) | RetryAttempt(4) | integration |
| equality: same | RetryAttempt(2), RetryAttempt(2) | true | unit |
| proptest invariant | any u32 | value() == original | proptest |

### Progress

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: middle | 50.0 | Ok(Progress(50.0)) | unit |
| happy path: min boundary | 0.0 | Ok(Progress(0.0)) | unit |
| happy path: max boundary | 100.0 | Ok(Progress(100.0)) | unit |
| happy path: subnormal | 0.0000001 | Ok(Progress(0.0000001)) | unit |
| happy path: near max | 99.9999 | Ok(Progress(99.9999)) | unit |
| error: negative | -0.001 | Err(ProgressError::OutOfRange) | unit |
| error: exceeds max | 100.001 | Err(ProgressError::OutOfRange) | unit |
| error: NaN | f64::NAN | Err(ProgressError::NaN) | unit |
| accessor: value() | Progress(75.5) | 75.5f64 | unit |
| Deref | Progress(33.3) | 33.3f64 | unit |
| AsRef | Progress(33.3) | &33.3f64 | unit |
| serde round-trip | Progress(62.5) | Progress(62.5) | integration |
| serde round-trip: min | Progress(0.0) | Progress(0.0) | integration |
| serde round-trip: max | Progress(100.0) | Progress(100.0) | integration |
| equality: same | Progress(50.0), Progress(50.0) | true | unit |
| error Display: OutOfRange | Progress(-0.001).unwrap_err() | "Progress -0.001 out of valid range 0.0..=100.0" | unit |
| error Display: NaN | Progress(f64::NAN).unwrap_err() | "Progress NaN is not a valid progress" | unit |
| error equality | Progress(-0.001).unwrap_err(), x2 | true | unit |
| proptest invariant | any f64 0.0..=100.0 | value() == original | proptest |
| proptest invariant: NaN error | f64::NAN | Err(ProgressError::NaN) | proptest |

### TaskCount

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: new valid | u32::new(0) | Ok(TaskCount(0)) | unit |
| happy path: new at max | u32::MAX | Ok(TaskCount(u32::MAX)) | unit |
| happy path: from_option Some | Some(10) | Ok(TaskCount(10)) | unit |
| error: from_option None | None | Err(TaskCountError::NoneNotAllowed) | unit |
| accessor: value() | TaskCount(7) | 7u32 | unit |
| Deref | TaskCount(7) | 7u32 | unit |
| AsRef | TaskCount(7) | &7u32 | unit |
| serde round-trip | TaskCount(42) | TaskCount(42) | integration |
| equality: same | TaskCount(7), TaskCount(7) | true | unit |
| error Display | TaskCount::from_option(None).unwrap_err() | "Optional task count must be present" | unit |
| error equality | TaskCount::from_option(None).unwrap_err(), x2 | true | unit |
| proptest invariant | any u32 | value() == original | proptest |

### TaskPosition

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: zero | 0i64 | Ok(TaskPosition(0)) | unit |
| happy path: positive | 99i64 | Ok(TaskPosition(99)) | unit |
| happy path: negative | -1i64 | Ok(TaskPosition(-1)) | unit |
| happy path: i64::MIN | i64::MIN | Ok(TaskPosition(i64::MIN)) | unit |
| happy path: i64::MAX | i64::MAX | Ok(TaskPosition(i64::MAX)) | unit |
| accessor: value() | TaskPosition(99) | 99i64 | unit |
| Deref | TaskPosition(99) | 99i64 | unit |
| AsRef | TaskPosition(99) | &99i64 | unit |
| Display | TaskPosition(-123) | "-123" | unit |
| Debug | TaskPosition(5) | contains "TaskPosition(" | unit |
| serde round-trip | TaskPosition(-5) | TaskPosition(-5) | integration |
| serde round-trip: i64::MIN | TaskPosition(i64::MIN) | TaskPosition(i64::MIN) | integration |
| serde round-trip: i64::MAX | TaskPosition(i64::MAX) | TaskPosition(i64::MAX) | integration |
| equality: same | TaskPosition(42), TaskPosition(42) | true | unit |
| proptest invariant | any i64 | value() == original | proptest |

---

## Open Questions

None — all types, invariants, error variants, and serialization contracts are fully specified in `contract.md`.

---

## Exit Criteria Verification

- [x] Every public API behavior has at least one BDD scenario (52 scenarios for 52 behaviors)
- [x] Every error variant in every Error enum has an explicit test scenario:
  - `PortError::OutOfRange` → 3 scenarios (zero, exceeds max, far out of range)
  - `RetryLimitError::NoneNotAllowed` → 1 scenario
  - `ProgressError::OutOfRange` → 2 scenarios (negative, exceeds max)
  - `ProgressError::NaN` → 1 scenario
  - `TaskCountError::NoneNotAllowed` → 1 scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant (8 invariants)
- [x] Every parsing/deserialization boundary has a fuzz target (6 targets)
- [x] Every error variant has an error Display test in unit layer
- [x] Every error equality has a unit test
- [x] The mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value — all assertions are exact
- [x] Unit test count meets ≥5× density requirement (60 unit tests for 12 public functions)
