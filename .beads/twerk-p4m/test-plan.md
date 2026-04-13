bead_id: twerk-p4m
bead_title: data: Define TriggerState enum and TriggerId type in twerk-core
phase: state-1.5-test-planning-retry1
updated_at: 2026-04-13T19:00:00Z

# Test Plan: TriggerState Enum & TriggerId Type (v2 — revised after review rejection)

## Summary
- Behaviors identified: 42
- Trophy allocation: 78 unit / 0 integration / 0 e2e / 6 static
- Proptest invariants: 6
- Fuzz targets: 2
- Kani harnesses: 2
- Mutation testing threshold: >= 90% kill rate
- Test density: 78 unit tests / 14 public functions+traits = **5.57x** (target: >=5x)

## Deviation Justification

This bead adds two pure data types with no I/O, no async, no cross-component interaction.
The Testing Trophy target of ~60% integration does not apply here — there is nothing
to integrate-test. **All tests are unit tests in `#[cfg(test)]` modules** (matching the
existing pattern in `id.rs`, `job.rs`, `task.rs`, `domain_types.rs`). Static analysis
(clippy, cargo-deny, type-checking) catches derive and trait coherence errors at compile
time.

Rationale: 100% unit + static. The types are pure value types with zero dependencies on
external state. Every behavior is exercised through the public API (`new()`, `as_str()`,
`FromStr`, `Display`, `Serialize`/`Deserialize`, `Default`, `From<String>`, `From<&str>`,
`AsRef<str>`, `Deref`, `Borrow<str>`, `Clone`, `PartialEq`, `Eq`, `Hash`). Integration
tests would add no coverage beyond what unit tests already provide.

---

## 1. Behavior Inventory

### TriggerState (behaviors 1-22)

1. TriggerState serializes Active to "ACTIVE" when serialized via serde_json
2. TriggerState serializes Paused to "PAUSED" when serialized via serde_json
3. TriggerState serializes Disabled to "DISABLED" when serialized via serde_json
4. TriggerState serializes Error to "ERROR" when serialized via serde_json
5. TriggerState defaults to Active when Default::default() is called
6. TriggerState formats Active as "ACTIVE" when Display is used
7. TriggerState formats Paused as "PAUSED" when Display is used
8. TriggerState formats Disabled as "DISABLED" when Display is used
9. TriggerState formats Error as "ERROR" when Display is used
10. TriggerState parses "active" case-insensitively when FromStr is used
11. TriggerState parses "ACTIVE" case-insensitively when FromStr is used
12. TriggerState parses "Paused" case-insensitively when FromStr is used
13. TriggerState parses "error" case-insensitively when FromStr is used
14. TriggerState rejects unknown string "DESTROYED" when FromStr receives unrecognized input
15. TriggerState rejects empty string when FromStr receives ""
16. TriggerState rejects whitespace-only string when FromStr receives "   "
17. TriggerState rejects prefix of valid name "ACTIV" when FromStr receives partial match
18. TriggerState rejects trailing whitespace "ACTIVE " when FromStr receives trailing space
19. TriggerState deserializes Active from JSON when serde_json::from_str is used
20. TriggerState deserializes Paused from JSON when serde_json::from_str is used
21. TriggerState deserializes Disabled from JSON when serde_json::from_str is used
22. TriggerState deserializes Error from JSON when serde_json::from_str is used
23. TriggerState rejects unknown JSON value "UNKNOWN" when deserializing invalid string
24. TriggerState Display matches serde output for every variant when both are used
25. ParseTriggerStateError displays "unknown TriggerState: {input}" when Display is called
26. ParseTriggerStateError implements std::error::Error when used as error trait object
27. ParseTriggerStateError PartialEq compares inner strings when two errors are compared
28. ParseTriggerStateError Clone produces identical copy when clone is called
29. TriggerState copies without heap allocation when Copy is used
30. TriggerState PartialEq reflexive when same variant compared to itself
31. TriggerState Eq symmetry when Active == Active
32. TriggerState Hash suitability when used as HashSet key

### TriggerId (behaviors 33-52)

33. TriggerId constructs successfully when input is 3-char valid string "abc"
34. TriggerId constructs successfully when input is exactly 64 chars
35. TriggerId constructs successfully when input has dashes and underscores "a_b-c"
36. TriggerId constructs successfully when input is CJK "日本語" (3 chars)
37. TriggerId rejects empty input when new() is called
38. TriggerId rejects too-short input (2 chars "ab") when new() is called
39. TriggerId rejects too-short input (1 char "a") when new() is called
40. TriggerId rejects too-long input (65 chars) when new() is called
41. TriggerId rejects too-long input (100 chars) when new() is called
42. TriggerId rejects invalid character "@" when input contains "abc@def"
43. TriggerId rejects invalid character " " (space) when input contains "abc def"
44. TriggerId rejects invalid character emoji when input contains "abc-\u{1F525}"
45. TriggerId rejects null byte when input contains "abc\x00def"
46. TriggerId preserves input exactly when constructed successfully
47. TriggerId rejects leading whitespace when input has " abc"
48. TriggerId rejects trailing whitespace when input has "abc "
49. TriggerId preserves mixed case when input has "MyTrigger_01"
50. TriggerId as_str returns original string when queried
51. TriggerId Display returns original string when formatted
52. TriggerId serializes as transparent JSON string when serialized
53. TriggerId deserializes from valid JSON string when deserialized
54. TriggerId rejects too-short (2-char) JSON when deserializing "ab"
55. TriggerId rejects too-short (1-char) JSON when deserializing "x"
56. TriggerId rejects empty JSON when deserializing ""
57. TriggerId rejects too-long (65-char) JSON when deserializing 65-char string
58. TriggerId Default returns empty string when Default::default() is called
59. TriggerId FromStr delegates to new() when parsing valid string
60. TriggerId FromStr delegates to new() when parsing invalid short string
61. TriggerId From<String> bypasses validation when constructed from 1-char String
62. TriggerId From<&str> bypasses validation when constructed from 1-char &str
63. TriggerId AsRef<str> returns inner string when as_ref() is called
64. TriggerId Deref<Target=str> returns inner string when dereferenced
65. TriggerId Borrow<str> returns inner string when borrow() is called
66. TriggerId Clone produces equal copy when clone is called
67. TriggerId PartialEq reflexive when same value compared to itself
68. TriggerId Eq+Hash suitable for HashMap key when used in collections

### IdError Display through TriggerId::new() (behaviors 69-72)

69. IdError::Empty displays "empty" when returned from TriggerId::new("")
70. IdError::TooLong displays length value when returned from TriggerId::new() with 65-char input
71. IdError::TooShort displays length value when returned from TriggerId::new() with 2-char input
72. IdError::InvalidCharacters displays "invalid" when returned from TriggerId::new() with "bad@id"

---

## 2. Trophy Allocation

| # | Behavior | Layer | Justification |
|---|----------|-------|---------------|
| 1-4 | TriggerState serde serialization (4 variants) | unit | Pure `serde_json::to_string`, no I/O |
| 5 | TriggerState default | unit | `Default` trait, pure constructor |
| 6-9 | TriggerState Display formatting (4 variants) | unit | Pure `format!` call |
| 10-13 | TriggerState FromStr parsing valid (4 cases) | unit | Pure string parsing |
| 14 | TriggerState FromStr rejects unknown | unit | Error path, exact variant |
| 15 | TriggerState FromStr rejects empty string | unit | Boundary: empty input |
| 16 | TriggerState FromStr rejects whitespace-only | unit | Boundary: whitespace |
| 17 | TriggerState FromStr rejects prefix | unit | Boundary: partial match |
| 18 | TriggerState FromStr rejects trailing whitespace | unit | Boundary: trailing space |
| 19-22 | TriggerState JSON deserialization (4 variants) | unit | `serde_json::from_str`, no I/O |
| 23 | TriggerState JSON deserialization rejection | unit | Error path, concrete assertion |
| 24 | Display==serde roundtrip | unit | Pure value comparison |
| 25 | ParseTriggerStateError Display | unit | Error formatting |
| 26 | ParseTriggerStateError std::error::Error | unit | Trait object construction |
| 27 | ParseTriggerStateError PartialEq | unit | Equality semantics |
| 28 | ParseTriggerStateError Clone | unit | Clone semantics |
| 29 | TriggerState Copy | unit | Compile-time + size check |
| 30-31 | TriggerState PartialEq + Eq | unit | Equality + symmetry |
| 32 | TriggerState Hash | unit | HashSet membership |
| 33-36 | TriggerId construction happy paths | unit | Pure validation logic |
| 37-45 | TriggerId validation error paths (9 cases) | unit | Every error variant + edge cases |
| 46 | TriggerId preserves input exactly | unit | Identity preservation |
| 47-48 | TriggerId rejects whitespace | unit | No-trim mutation target |
| 49 | TriggerId preserves mixed case | unit | No-lowercase mutation target |
| 50-51 | TriggerId accessors (as_str, Display) | unit | Pure getter |
| 52-53 | TriggerId serde roundtrip (valid) | unit | `serde_json` in-memory |
| 54-57 | TriggerId serde rejection (4 cases) | unit | Error path, concrete assertion |
| 58 | TriggerId Default | unit | Pure constructor |
| 59-60 | TriggerId FromStr (valid + invalid) | unit | Pure string parsing |
| 61-62 | TriggerId From<String> / From<&str> bypass | unit | Infallible trait impls |
| 63-65 | TriggerId AsRef / Deref / Borrow | unit | Trait impl verification |
| 66 | TriggerId Clone | unit | Clone semantics |
| 67 | TriggerId PartialEq reflexive | unit | Equality |
| 68 | TriggerId Eq+Hash | unit | HashSet membership |
| 69-72 | IdError Display through TriggerId::new() | unit | Error message correctness |
| S1 | clippy: no warnings on new code | static | Compile-time |
| S2 | cargo-deny: no new advisories | static | Compile-time |
| S3 | type-check: derive coherence | static | Compile-time |
| S4 | type-check: Copy on TriggerState | static | Compile-time |
| S5 | type-check: Default on TriggerState | static | Compile-time |
| S6 | type-check: serde transparent on TriggerId | static | Compile-time |

---

## 3. BDD Scenarios

### TriggerState — serde serialization (Behaviors 1-4)

`fn trigger_state_serializes_active_to_uppercase()`
```
Given: TriggerState::Active
When: serde_json::to_string(&state)
Then: result == Ok("\"ACTIVE\"".to_string())
```

`fn trigger_state_serializes_paused_to_uppercase()`
```
Given: TriggerState::Paused
When: serde_json::to_string(&state)
Then: result == Ok("\"PAUSED\"".to_string())
```

`fn trigger_state_serializes_disabled_to_uppercase()`
```
Given: TriggerState::Disabled
When: serde_json::to_string(&state)
Then: result == Ok("\"DISABLED\"".to_string())
```

`fn trigger_state_serializes_error_to_uppercase()`
```
Given: TriggerState::Error
When: serde_json::to_string(&state)
Then: result == Ok("\"ERROR\"".to_string())
```

### TriggerState — default (Behavior 5)

`fn trigger_state_default_returns_active()`
```
Given: no prior state
When: TriggerState::default()
Then: result == TriggerState::Active
```

### TriggerState — Display formatting (Behaviors 6-9)

`fn trigger_state_display_formats_active()`
```
Given: TriggerState::Active
When: format!("{}", state)
Then: output == "ACTIVE"
```

`fn trigger_state_display_formats_paused()`
```
Given: TriggerState::Paused
When: format!("{}", state)
Then: output == "PAUSED"
```

`fn trigger_state_display_formats_disabled()`
```
Given: TriggerState::Disabled
When: format!("{}", state)
Then: output == "DISABLED"
```

`fn trigger_state_display_formats_error()`
```
Given: TriggerState::Error
When: format!("{}", state)
Then: output == "ERROR"
```

### TriggerState — FromStr valid parsing (Behaviors 10-13)

`fn trigger_state_parses_lowercase_active()`
```
Given: string "active"
When: "active".parse::<TriggerState>()
Then: result == Ok(TriggerState::Active)
```

`fn trigger_state_parses_uppercase_active()`
```
Given: string "ACTIVE"
When: "ACTIVE".parse::<TriggerState>()
Then: result == Ok(TriggerState::Active)
```

`fn trigger_state_parses_mixed_case_paused()`
```
Given: string "Paused"
When: "Paused".parse::<TriggerState>()
Then: result == Ok(TriggerState::Paused)
```

`fn trigger_state_parses_lowercase_error()`
```
Given: string "error"
When: "error".parse::<TriggerState>()
Then: result == Ok(TriggerState::Error)
```

### TriggerState — FromStr rejection (Behaviors 14-18)

`fn trigger_state_parse_rejects_unknown_string()`
```
Given: string "DESTROYED"
When: "DESTROYED".parse::<TriggerState>()
Then: result == Err(ParseTriggerStateError(String::from("DESTROYED")))
And: result.unwrap_err().0 == "DESTROYED"
```

`fn trigger_state_parse_rejects_empty_string()`
```
Given: string ""
When: "".parse::<TriggerState>()
Then: result == Err(ParseTriggerStateError(String::from("")))
And: result.unwrap_err().0 == ""
```

`fn trigger_state_parse_rejects_whitespace_only()`
```
Given: string "   "
When: "   ".parse::<TriggerState>()
Then: result == Err(ParseTriggerStateError(String::from("   ")))
And: result.unwrap_err().0 == "   "
```

`fn trigger_state_parse_rejects_prefix_of_valid_name()`
```
Given: string "ACTIV"
When: "ACTIV".parse::<TriggerState>()
Then: result == Err(ParseTriggerStateError(String::from("ACTIV")))
And: result.unwrap_err().0 == "ACTIV"
```

`fn trigger_state_parse_rejects_trailing_whitespace()`
```
Given: string "ACTIVE "
When: "ACTIVE ".parse::<TriggerState>()
Then: result == Err(ParseTriggerStateError(String::from("ACTIVE ")))
And: result.unwrap_err().0 == "ACTIVE "
```

### TriggerState — JSON deserialization (Behaviors 19-23)

`fn trigger_state_deserializes_active_from_json()`
```
Given: JSON string "\"ACTIVE\""
When: serde_json::from_str::<TriggerState>("\"ACTIVE\"")
Then: result == Ok(TriggerState::Active)
```

`fn trigger_state_deserializes_paused_from_json()`
```
Given: JSON string "\"PAUSED\""
When: serde_json::from_str::<TriggerState>("\"PAUSED\"")
Then: result == Ok(TriggerState::Paused)
```

`fn trigger_state_deserializes_disabled_from_json()`
```
Given: JSON string "\"DISABLED\""
When: serde_json::from_str::<TriggerState>("\"DISABLED\"")
Then: result == Ok(TriggerState::Disabled)
```

`fn trigger_state_deserializes_error_from_json()`
```
Given: JSON string "\"ERROR\""
When: serde_json::from_str::<TriggerState>("\"ERROR\"")
Then: result == Ok(TriggerState::Error)
```

`fn trigger_state_deserialize_rejects_unknown_value()`
```
Given: JSON string "\"UNKNOWN\""
When: serde_json::from_str::<TriggerState>("\"UNKNOWN\"")
Then: result.is_err() == true
And: result.unwrap_err().to_string().contains("unknown variant") == true
```

### TriggerState — Display==serde roundtrip (Behavior 24)

`fn trigger_state_display_equals_serde_for_all_variants()`
```
Given: all four TriggerState variants [Active, Paused, Disabled, Error]
When: for each variant: format!("{state}") == serde_json::to_string(&state).unwrap().trim_matches('"')
Then: equality holds for all four variants
```

### TriggerState — ParseTriggerStateError (Behaviors 25-28)

`fn parse_trigger_state_error_displays_message()`
```
Given: ParseTriggerStateError(String::from("bad"))
When: format!("{}", err)
Then: output == "unknown TriggerState: bad"
```

`fn parse_trigger_state_error_implements_std_error()`
```
Given: ParseTriggerStateError(String::from("test"))
When: let e: &dyn std::error::Error = &err
Then: e.source() == None
```

`fn parse_trigger_state_error_partial_eq_compares_inner()`
```
Given: err1 = ParseTriggerStateError(String::from("X")), err2 = ParseTriggerStateError(String::from("X")), err3 = ParseTriggerStateError(String::from("Y"))
When: err1 == err2, err1 == err3
Then: err1 == err2 == true
And: err1 == err3 == false
```

`fn parse_trigger_state_error_clone_produces_identical_copy()`
```
Given: err = ParseTriggerStateError(String::from("test"))
When: let cloned = err.clone()
Then: cloned == err
And: cloned.0 == "test"
```

### TriggerState — Copy, PartialEq, Eq, Hash (Behaviors 29-32)

`fn trigger_state_is_copy_and_zero_sized_heap()`
```
Given: TriggerState::Active
When: let copy = state
Then: copy == state
And: std::mem::size_of::<TriggerState>() <= std::mem::size_of::<u8>()
```

`fn trigger_state_partial_eq_reflexive()`
```
Given: TriggerState::Active
When: state == state
Then: true
```

`fn trigger_state_eq_symmetry_for_all_variants()`
```
Given: all four variants
When: for each variant v: v == v
Then: all equalities hold
And: TriggerState::Active != TriggerState::Paused
```

`fn trigger_state_hash_works_in_hashset()`
```
Given: TriggerState::Active, TriggerState::Paused, TriggerState::Active
When: inserted into HashSet
Then: set.len() == 2 (Active deduplicated)
```

### TriggerId — construction happy paths (Behaviors 33-36)

`fn trigger_id_new_returns_ok_when_input_is_3_chars()`
```
Given: string "abc"
When: TriggerId::new("abc")
Then: result == Ok(TriggerId) (value equality via as_str)
And: result.unwrap().as_str() == "abc"
```

`fn trigger_id_new_accepts_exactly_64_chars()`
```
Given: string "a".repeat(64)
When: TriggerId::new(&max_valid)
Then: result.is_ok() == true
And: result.unwrap().as_str().len() == 64
```

`fn trigger_id_new_accepts_dash_and_underscore()`
```
Given: string "a_b-c"
When: TriggerId::new("a_b-c")
Then: result.unwrap().as_str() == "a_b-c"
```

`fn trigger_id_new_accepts_cjk_characters()`
```
Given: string "日本語" (3 CJK chars)
When: TriggerId::new("日本語")
Then: result.unwrap().as_str() == "日本語"
```

### TriggerId — validation error paths (Behaviors 37-45)

`fn trigger_id_new_returns_err_empty_when_input_is_empty()`
```
Given: string ""
When: TriggerId::new("")
Then: result == Err(IdError::Empty)
```

`fn trigger_id_new_returns_err_too_short_when_input_is_2_chars()`
```
Given: string "ab"
When: TriggerId::new("ab")
Then: result == Err(IdError::TooShort(2))
```

`fn trigger_id_new_returns_err_too_short_when_input_is_1_char()`
```
Given: string "a"
When: TriggerId::new("a")
Then: result == Err(IdError::TooShort(1))
```

`fn trigger_id_new_returns_err_too_long_when_input_is_65_chars()`
```
Given: string "a".repeat(65)
When: TriggerId::new(&long)
Then: result == Err(IdError::TooLong(65))
```

`fn trigger_id_new_returns_err_too_long_when_input_is_100_chars()`
```
Given: string "a".repeat(100)
When: TriggerId::new(&long)
Then: result == Err(IdError::TooLong(100))
```

`fn trigger_id_new_returns_err_invalid_characters_when_input_has_at_sign()`
```
Given: string "abc@def"
When: TriggerId::new("abc@def")
Then: result == Err(IdError::InvalidCharacters)
```

`fn trigger_id_new_returns_err_invalid_characters_when_input_has_space()`
```
Given: string "abc def"
When: TriggerId::new("abc def")
Then: result == Err(IdError::InvalidCharacters)
```

`fn trigger_id_new_returns_err_invalid_characters_when_input_has_emoji()`
```
Given: string "abc-\u{1F525}def" (7 chars, valid length)
When: TriggerId::new("abc-\u{1F525}def")
Then: result == Err(IdError::InvalidCharacters)
```

`fn trigger_id_new_returns_err_invalid_characters_when_input_has_null_byte()`
```
Given: string "abc\x00def" (7 chars, valid length)
When: TriggerId::new("abc\x00def")
Then: result == Err(IdError::InvalidCharacters)
```

### TriggerId — preservation and whitespace (Behaviors 46-49)

`fn trigger_id_preserves_input_string_exactly()`
```
Given: string "my-trigger_01"
When: TriggerId::new("my-trigger_01")
Then: result.unwrap().to_string() == "my-trigger_01"
```

`fn trigger_id_new_rejects_leading_whitespace()`
```
Given: string " abc" (4 chars, but starts with space)
When: TriggerId::new(" abc")
Then: result == Err(IdError::InvalidCharacters)
```

`fn trigger_id_new_rejects_trailing_whitespace()`
```
Given: string "abc " (4 chars, but ends with space)
When: TriggerId::new("abc ")
Then: result == Err(IdError::InvalidCharacters)
```

`fn trigger_id_new_preserves_mixed_case()`
```
Given: string "MyTrigger_01" (12 chars, mixed case)
When: TriggerId::new("MyTrigger_01")
Then: result.unwrap().as_str() == "MyTrigger_01"
And: result.unwrap().as_str().contains("M") == true (uppercase preserved)
```

### TriggerId — accessors (Behaviors 50-51)

`fn trigger_id_as_str_returns_original()`
```
Given: TriggerId constructed from "valid-id"
When: id.as_str()
Then: output == "valid-id"
```

`fn trigger_id_display_returns_original_string()`
```
Given: TriggerId constructed from "my-trigger"
When: format!("{}", id)
Then: output == "my-trigger"
```

### TriggerId — serde roundtrip (Behaviors 52-53)

`fn trigger_id_serializes_as_plain_json_string()`
```
Given: TriggerId constructed from "trigger-abc"
When: serde_json::to_string(&id)
Then: result == Ok("\"trigger-abc\"".to_string())
```

`fn trigger_id_deserializes_from_valid_json_string()`
```
Given: JSON string "\"my-trigger\""
When: serde_json::from_str::<TriggerId>("\"my-trigger\"")
Then: result.is_ok() == true
And: result.unwrap().as_str() == "my-trigger"
```

### TriggerId — serde rejection (Behaviors 54-57)

`fn trigger_id_deserialize_rejects_2_char_string()`
```
Given: JSON string "\"ab\""
When: serde_json::from_str::<TriggerId>("\"ab\"")
Then: result.is_err() == true
And: result.unwrap_err().to_string().contains("too short") == true
```

`fn trigger_id_deserialize_rejects_1_char_string()`
```
Given: JSON string "\"x\""
When: serde_json::from_str::<TriggerId>("\"x\"")
Then: result.is_err() == true
And: result.unwrap_err().to_string().contains("too short") == true
```

`fn trigger_id_deserialize_rejects_empty_string()`
```
Given: JSON string "\"\""
When: serde_json::from_str::<TriggerId>("\"\"")
Then: result.is_err() == true
And: result.unwrap_err().to_string().contains("empty") == true
```

`fn trigger_id_deserialize_rejects_65_char_string()`
```
Given: JSON string of 65 'a' characters
When: serde_json::from_str::<TriggerId>(json)
Then: result.is_err() == true
And: result.unwrap_err().to_string().contains("too long") == true
```

### TriggerId — Default (Behavior 58)

`fn trigger_id_default_returns_empty_string()`
```
Given: no prior state
When: TriggerId::default()
Then: id.as_str() == ""
```

### TriggerId — FromStr (Behaviors 59-60)

`fn trigger_id_from_str_parses_valid_string()`
```
Given: string "valid-id"
When: "valid-id".parse::<TriggerId>()
Then: result.is_ok() == true
And: result.unwrap().as_str() == "valid-id"
```

`fn trigger_id_from_str_rejects_short_string()`
```
Given: string "x"
When: "x".parse::<TriggerId>()
Then: result == Err(IdError::TooShort(1))
```

### TriggerId — From<String> and From<&str> bypass validation (Behaviors 61-62)

`fn trigger_id_from_string_bypasses_validation()`
```
Given: String from a 1-char string "x" (too short for new())
When: let id = TriggerId::from(String::from("x"))
Then: id.as_str() == "x" (no validation error; From is infallible)
And: id.as_str().len() == 1
```

`fn trigger_id_from_str_bypasses_validation()`
```
Given: &str "y" (1 char, too short for new())
When: let id = TriggerId::from("y")
Then: id.as_str() == "y" (no validation error; From is infallible)
And: id.as_str().len() == 1
```

### TriggerId — trait impls (Behaviors 63-67)

`fn trigger_id_as_ref_returns_inner_string()`
```
Given: TriggerId constructed from "ref-test"
When: let s: &str = id.as_ref()
Then: s == "ref-test"
```

`fn trigger_id_deref_returns_inner_string()`
```
Given: TriggerId constructed from "deref-test"
When: let s: &str = &*id
Then: s == "deref-test"
```

`fn trigger_id_borrow_returns_inner_string()`
```
Given: TriggerId constructed from "borrow-test"
When: use std::borrow::Borrow; let s: &str = id.borrow()
Then: s == "borrow-test"
```

`fn trigger_id_clone_produces_equal_copy()`
```
Given: TriggerId constructed from "clone-test"
When: let cloned = id.clone()
Then: cloned == id
And: cloned.as_str() == "clone-test"
```

`fn trigger_id_partial_eq_reflexive()`
```
Given: TriggerId constructed from "eq-test"
When: id == id
Then: true
```

### TriggerId — Eq+Hash (Behavior 68)

`fn trigger_id_eq_and_hash_works_in_hashset()`
```
Given: id1 from "same", id2 from "same", id3 from "different"
When: inserted into HashSet
Then: set.len() == 2 (id1 and id2 are equal, deduplicated)
And: set.contains(&TriggerId::new("same").unwrap()) == true
```

### IdError Display through TriggerId::new() path (Behaviors 69-72)

`fn trigger_id_new_returns_err_empty_displays_correct_message()`
```
Given: TriggerId::new("") returns Err
When: let err = TriggerId::new("").unwrap_err()
Then: err matches IdError::Empty
And: format!("{}", err).to_lowercase().contains("empty") == true
```

`fn trigger_id_new_returns_err_too_long_displays_correct_message()`
```
Given: TriggerId::new(&"a".repeat(65)) returns Err
When: let err = TriggerId::new(&"a".repeat(65)).unwrap_err()
Then: err matches IdError::TooLong(65)
And: format!("{}", err).contains("65") == true
```

`fn trigger_id_new_returns_err_too_short_displays_correct_message()`
```
Given: TriggerId::new("ab") returns Err
When: let err = TriggerId::new("ab").unwrap_err()
Then: err matches IdError::TooShort(2)
And: format!("{}", err).to_lowercase().contains("too short") == true
And: format!("{}", err).contains("2") == true
```

`fn trigger_id_new_returns_err_invalid_chars_displays_correct_message()`
```
Given: TriggerId::new("bad@id") returns Err
When: let err = TriggerId::new("bad@id").unwrap_err()
Then: err matches IdError::InvalidCharacters
And: format!("{}", err).to_lowercase().contains("invalid") == true
```

---

## 4. Proptest Invariants

### Proptest: TriggerId::new() length boundary

`proptest_trigger_id_rejects_lengths_outside_3_to_64()`
```
Invariant: For any string s, if s.len() < 3 || s.len() > 64, then TriggerId::new(s) is Err.
           If 3 <= s.len() <= 64 and all chars satisfy is_alphanumeric() || c == '-' || c == '_',
           then TriggerId::new(s) is Ok.

Strategy: Generate strings of length 0..=70 from "[a-zA-Z0-9_-]{len}"
Anti-invariant: Strings of length 0, 1, 2 must always produce Err(TooShort).
               Strings of length 65, 66, ... must always produce Err(TooLong).
```

### Proptest: TriggerId::new() character validation

`proptest_trigger_id_rejects_invalid_chars()`
```
Invariant: For any string s of length 3..=64, if s contains a character outside
           [a-zA-Z0-9_-], then TriggerId::new(s) is Err(InvalidCharacters).

Strategy: Generate strings of length 3..=64 where one random character is replaced
          with a char from "!@#$%^&*() +=[]{}|\\:;\"'<>,./?`~ \t\n\r\x00"
Anti-invariant: Any string containing a non-allowed char must fail, regardless of length.
```

### Proptest: TriggerState serde roundtrip

`proptest_trigger_state_serde_roundtrip_preserves_value()`
```
Invariant: For any TriggerState variant v, serializing then deserializing yields v.

Strategy: proptest::arbitrary::any::<TriggerState>() (requires deriving Arbitrary or
          using a strategy that picks from the 4 variants)
Anti-invariant: None (all variants are valid).
```

### Proptest: TriggerId serde roundtrip

`proptest_trigger_id_serde_roundtrip_preserves_string()`
```
Invariant: For any valid TriggerId, serde_json::to_string then serde_json::from_str
           yields an identical TriggerId with the same as_str().

Strategy: Generate strings matching "[a-zA-Z0-9_-]{3,64}" via proptest::string::string_regex
Anti-invariant: None (strategy only generates valid inputs).
```

### Proptest: TriggerState FromStr case-insensitivity

`proptest_trigger_state_from_str_ignores_case()`
```
Invariant: For any TriggerState variant v, any case-variant of v's name parses to v.

Strategy: For each variant name, generate random-case permutations (e.g. "aCtIvE", "AcTiVe").
          Strategy: pick variant, then for each char randomly upper/lowercase it.
Anti-invariant: None.
```

### Proptest: TriggerId input preservation

`proptest_trigger_id_preserves_input_without_mutation()`
```
Invariant: For any string s where TriggerId::new(s) is Ok, id.as_str() == s (byte-for-byte
           equality, no trimming, no case mutation).

Strategy: Generate strings matching "[a-zA-Z0-9_-]{3,64}"
Anti-invariant: None (valid inputs only). This specifically catches mutations that add
               .trim(), .to_lowercase(), or .to_uppercase() to new().
```

---

## 5. Fuzz Targets

### Fuzz Target: TriggerId::new() with arbitrary bytes

```
Input type: arbitrary String (via arbitrary crate or libfuzzer)
Risk: Panic on OOB, excessive memory on huge strings, logic error in length check,
      missed edge case in character validation (e.g., Unicode edge cases like
      combining characters, zero-width joiners, RTL overrides).
Corpus seeds:
  - ""                          (empty)
  - "a"                         (1 char)
  - "ab"                        (2 chars, boundary)
  - "abc"                       (3 chars, boundary)
  - "a".repeat(64)              (max valid)
  - "a".repeat(65)              (min invalid long)
  - "a".repeat(1000)            (well beyond max)
  - "abc@def"                   (invalid char in middle)
  - "日本語"                     (CJK alphanumeric -- valid per Rust's is_alphanumeric)
  - "abc\x00def"                (null byte)
  - "abc\ndef"                  (newline)
  - "abc\tdef"                  (tab)
  - "-_-"                       (only separators, 3 chars, valid)
  - "a b"                       (space, invalid)
  - "a\u{200B}b"                (zero-width space)
  - "a\u{FEFF}b"                (BOM)
  - " abc"                      (leading space, invalid)
  - "abc "                      (trailing space, invalid)
  - "MyTrigger_01"              (mixed case, valid)
```

**Note:** The existing codebase has no `fuzz/` directory and no `cargo-fuzz` configuration.
This fuzz target is specified for future adoption. The proptest invariant in Section 4
provides equivalent coverage for release 0.1. The fuzz target should be set up when the
project adds a fuzzing infrastructure (add `cargo-fuzz` to workspace, create `fuzz/` dir).

### Fuzz Target: TriggerState from_str with arbitrary bytes

```
Input type: arbitrary String
Risk: Panic on extremely long input, missed case-folding edge case, non-UTF8 strings
      (FromStr takes &str so non-UTF8 is handled by the caller).
Corpus seeds:
  - "ACTIVE"                    (exact uppercase match)
  - "active"                    (exact lowercase)
  - "AcTiVe"                    (mixed case)
  - ""                          (empty)
  - "active\0"                  (null appended)
  - " active"                   (leading space)
  - "active "                   (trailing space)
  - "ACTIVEX"                   (prefix of valid variant)
  - "PAUSE"                     (truncated variant name)
  - "disabled\x00extra"         (embedded null)
  - "   "                       (whitespace only)
```

---

## 6. Kani Harnesses

### Kani Harness: TriggerId validation is complete

```
Property: TriggerId::new(s) returns Ok iff:
  1. s is non-empty
  2. s.len() >= 3
  3. s.len() <= 64
  4. every char c in s satisfies c.is_alphanumeric() || c == '-' || c == '_'

Bound: s.len() <= 128 (Kani needs bounded search; 128 covers the 3-64 range plus overflow)
Rationale: The validation function is the sole gatekeeper for TriggerId construction.
          A missed case would allow invalid IDs into the system. Formal verification
          proves completeness of the four validation checks for all strings up to 128 bytes.

Harness pseudocode:
  let s: String = arbiter::any();
  assume(s.len() <= 128);
  let result = TriggerId::new(s);
  if s.is_empty() {
      assert!(matches!(result, Err(IdError::Empty)));
  } else if s.len() < 3 {
      assert!(matches!(result, Err(IdError::TooShort(n)) if n == s.len()));
  } else if s.len() > 64 {
      assert!(matches!(result, Err(IdError::TooLong(n)) if n == s.len()));
  } else if !s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
      assert!(matches!(result, Err(IdError::InvalidCharacters)));
  } else {
      assert!(result.is_ok());
      assert_eq!(result.unwrap().as_str(), s);
  }
```

### Kani Harness: TriggerState has exactly 4 variants and Default is Active

```
Property: TriggerState::default() == TriggerState::Active (compile-time guarantee).
          All 4 variants serialize to their SCREAMING_SNAKE_CASE names.
Bound: N/A (enum variants are exhaustive by construction in Rust)
Rationale: This is mostly a compile-time guarantee (exhaustive match), but Kani can verify
          that no panics occur in Display/FromStr for any &str input, and that the
          Default variant is Active.

Harness pseudocode:
  assert_eq!(TriggerState::default(), TriggerState::Active);
  // Verify no match arm is missing (exhaustiveness is compile-time, but):
  for variant in [Active, Paused, Disabled, Error] {
      let serialized = serde_json::to_string(&variant).unwrap();
      let deserialized: TriggerState = serde_json::from_str(&serialized).unwrap();
      assert_eq!(variant, deserialized);
  }
```

---

## 7. Mutation Testing Checkpoints

### Critical Mutations to Survive

| Source Mutation | Which Test Catches It | Expected Kill |
|---|---|---|
| `TriggerState::default()` returns `Paused` instead of `Active` | `trigger_state_default_returns_active` | Kill |
| `Display` for `Active` returns `"active"` (lowercase) | `trigger_state_display_formats_active` | Kill |
| `Display` for `Error` returns `"ERROR_STATE"` (wrong name) | `trigger_state_display_formats_error` | Kill |
| `FromStr` match arm `"PAUSED"` changed to `"SUSPENDED"` | `trigger_state_parses_mixed_case_paused` | Kill |
| `FromStr` fails to call `.to_uppercase()` (case-sensitive) | `trigger_state_parses_lowercase_active` | Kill |
| `ParseTriggerStateError` Display format changed | `parse_trigger_state_error_displays_message` | Kill |
| `TriggerId::new()` length check `< 3` changed to `< 2` | `trigger_id_new_returns_err_too_short_when_input_is_2_chars` | Kill |
| `TriggerId::new()` length check `> 64` changed to `> 65` | `trigger_id_new_returns_err_too_long_when_input_is_65_chars` | Kill |
| `TriggerId::new()` length check `<= 64` changed to `< 64` | `trigger_id_new_accepts_exactly_64_chars` | Kill |
| `TriggerId::new()` skips character validation | `trigger_id_new_returns_err_invalid_characters_when_input_has_at_sign` | Kill |
| `TriggerId::new()` empty check removed | `trigger_id_new_returns_err_empty_when_input_is_empty` | Kill |
| `TriggerId::new()` adds `.trim()` to input | `trigger_id_new_rejects_leading_whitespace` | Kill |
| `TriggerId::new()` adds `.to_lowercase()` to input | `trigger_id_new_preserves_mixed_case` | Kill |
| `IdError::Empty` Display omits "empty" | `trigger_id_new_returns_err_empty_displays_correct_message` | Kill |
| `IdError::TooLong` Display omits length | `trigger_id_new_returns_err_too_long_displays_correct_message` | Kill |
| `IdError::TooShort` Display omits length | `trigger_id_new_returns_err_too_short_displays_correct_message` | Kill |
| `IdError::InvalidCharacters` Display omits "invalid" | `trigger_id_new_returns_err_invalid_chars_displays_correct_message` | Kill |
| `serde(transparent)` removed from TriggerId | `trigger_id_serializes_as_plain_json_string` | Kill |
| `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` removed | `trigger_state_serializes_active_to_uppercase` | Kill |
| `TriggerState::Error` variant renamed | `trigger_state_display_formats_error` + serde test | Kill |
| `TriggerId Default` returns non-empty string | `trigger_id_default_returns_empty_string` | Kill |
| `Copy` derive removed from TriggerState | `trigger_state_is_copy_and_zero_sized_heap` | Kill |
| `From<String>` impl removed from TriggerId | `trigger_id_from_string_bypasses_validation` (won't compile) | Kill |
| `FromStr for TriggerId` bypasses validation | `trigger_id_from_str_rejects_short_string` | Kill |
| `TriggerState` FromStr accepts trailing whitespace | `trigger_state_parse_rejects_trailing_whitespace` | Kill |
| `TriggerState` FromStr accepts prefix "ACTIV" | `trigger_state_parse_rejects_prefix_of_valid_name` | Kill |
| `TriggerState` FromStr accepts empty string | `trigger_state_parse_rejects_empty_string` | Kill |

### Threshold

**>= 90% mutation kill rate** as measured by `cargo-mutants`.

---

## 8. Combinatorial Coverage Matrix

### TriggerState

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| serde: Active | `TriggerState::Active` | `"\"ACTIVE\""` | unit |
| serde: Paused | `TriggerState::Paused` | `"\"PAUSED\""` | unit |
| serde: Disabled | `TriggerState::Disabled` | `"\"DISABLED\""` | unit |
| serde: Error | `TriggerState::Error` | `"\"ERROR\""` | unit |
| default | `Default::default()` | `TriggerState::Active` | unit |
| display: Active | `TriggerState::Active` | `"ACTIVE"` | unit |
| display: Paused | `TriggerState::Paused` | `"PAUSED"` | unit |
| display: Disabled | `TriggerState::Disabled` | `"DISABLED"` | unit |
| display: Error | `TriggerState::Error` | `"ERROR"` | unit |
| parse: lowercase | `"active"` | `Ok(Active)` | unit |
| parse: uppercase | `"ACTIVE"` | `Ok(Active)` | unit |
| parse: mixed case | `"Paused"` | `Ok(Paused)` | unit |
| parse: lowercase error | `"error"` | `Ok(Error)` | unit |
| parse: unknown | `"DESTROYED"` | `Err(ParseTriggerStateError("DESTROYED"))` | unit |
| parse: empty string | `""` | `Err(ParseTriggerStateError(""))` | unit |
| parse: whitespace only | `"   "` | `Err(ParseTriggerStateError("   "))` | unit |
| parse: prefix | `"ACTIV"` | `Err(ParseTriggerStateError("ACTIV"))` | unit |
| parse: trailing space | `"ACTIVE "` | `Err(ParseTriggerStateError("ACTIVE "))` | unit |
| deserialize: Active | `"\"ACTIVE\""` | `Ok(Active)` | unit |
| deserialize: Paused | `"\"PAUSED\""` | `Ok(Paused)` | unit |
| deserialize: Disabled | `"\"DISABLED\""` | `Ok(Disabled)` | unit |
| deserialize: Error | `"\"ERROR\""` | `Ok(Error)` | unit |
| deserialize: invalid | `"\"UNKNOWN\""` | `Err(msg contains "unknown variant")` | unit |
| error Display | `ParseTriggerStateError("bad")` | `"unknown TriggerState: bad"` | unit |
| error: std::error::Error | `&dyn Error` | `source() == None` | unit |
| error: PartialEq | two errors | correct eq/ne | unit |
| error: Clone | cloned error | identical | unit |
| Copy check | `TriggerState::Active` | `size_of <= 1` | unit |
| Display==serde | all 4 variants | equality | unit |
| PartialEq reflexive | same variant | true | unit |
| Eq symmetry | Active == Active | true | unit |
| Hash in HashSet | Active, Paused, Active | len == 2 | unit |

### TriggerId

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path: 3 chars | `"abc"` | `Ok(as_str == "abc")` | unit |
| happy path: 64 chars | `"a".repeat(64)` | `Ok(len == 64)` | unit |
| happy path: dash/underscore | `"a_b-c"` | `Ok(as_str == "a_b-c")` | unit |
| happy path: CJK | `"日本語"` (3 chars) | `Ok(as_str == "日本語")` | unit |
| empty | `""` | `Err(IdError::Empty)` | unit |
| too short: 1 char | `"a"` | `Err(IdError::TooShort(1))` | unit |
| too short: 2 chars | `"ab"` | `Err(IdError::TooShort(2))` | unit |
| too long: 65 chars | `"a".repeat(65)` | `Err(IdError::TooLong(65))` | unit |
| too long: 100 chars | `"a".repeat(100)` | `Err(IdError::TooLong(100))` | unit |
| invalid char: @ | `"abc@def"` | `Err(IdError::InvalidCharacters)` | unit |
| invalid char: space | `"abc def"` | `Err(IdError::InvalidCharacters)` | unit |
| invalid char: emoji | `"abc-\u{1F525}def"` | `Err(IdError::InvalidCharacters)` | unit |
| invalid char: null byte | `"abc\x00def"` | `Err(IdError::InvalidCharacters)` | unit |
| leading whitespace | `" abc"` | `Err(IdError::InvalidCharacters)` | unit |
| trailing whitespace | `"abc "` | `Err(IdError::InvalidCharacters)` | unit |
| mixed case preserved | `"MyTrigger_01"` | `Ok(as_str == "MyTrigger_01")` | unit |
| as_str accessor | valid TriggerId | original string | unit |
| Display | valid TriggerId | original string | unit |
| serde serialize | valid TriggerId | `"\"...\""` | unit |
| serde deserialize: valid | `"\"abc\""` | `Ok(as_str == "abc")` | unit |
| serde deserialize: too short (2) | `"\"ab\""` | `Err(msg contains "too short")` | unit |
| serde deserialize: too short (1) | `"\"x\""` | `Err(msg contains "too short")` | unit |
| serde deserialize: empty | `"\"\""` | `Err(msg contains "empty")` | unit |
| serde deserialize: too long | `"\"a...65...\""` | `Err(msg contains "too long")` | unit |
| Default | `TriggerId::default()` | `as_str == ""` | unit |
| FromStr: valid | `"abc"` | `Ok(as_str == "abc")` | unit |
| FromStr: invalid | `"x"` | `Err(IdError::TooShort(1))` | unit |
| From<String>: bypass | `String::from("x")` | `as_str == "x"` | unit |
| From<&str>: bypass | `"y"` | `as_str == "y"` | unit |
| AsRef<str> | valid TriggerId | `as_ref() == "..."` | unit |
| Deref | valid TriggerId | `&*id == "..."` | unit |
| Borrow<str> | valid TriggerId | `borrow() == "..."` | unit |
| Clone | valid TriggerId | `cloned == id` | unit |
| PartialEq reflexive | same value | true | unit |
| Eq+Hash | same/diff strings | set.len() == 2 | unit |

### IdError Display through TriggerId::new() path

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| Empty Display | `TriggerId::new("")` | `Err(Empty)`, msg contains "empty" | unit |
| TooLong Display | `TriggerId::new(&"a".repeat(65))` | `Err(TooLong(65))`, msg contains "65" | unit |
| TooShort Display | `TriggerId::new("ab")` | `Err(TooShort(2))`, msg contains "too short" and "2" | unit |
| InvalidChars Display | `TriggerId::new("bad@id")` | `Err(InvalidCharacters)`, msg contains "invalid" | unit |

### Proptest Invariants

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| length boundary | strings 0..=70 | Err for <3 or >64, Ok otherwise | proptest |
| char validation | strings with injected special chars | Err(InvalidCharacters) | proptest |
| serde roundtrip (state) | any TriggerState | serialize == deserialize | proptest |
| serde roundtrip (id) | valid TriggerId strings | serialize == deserialize | proptest |
| FromStr case-insensitivity | random-case variant names | Ok(correct variant) | proptest |
| input preservation | valid TriggerId strings | as_str() == original byte-for-byte | proptest |

---

## Open Questions

None. All behaviors are fully specified in the contract (contract.md). The implementation
must follow the exact patterns established by `JobState`/`ScheduledJobState` (for
TriggerState) and `JobId`/`TaskId` (for TriggerId, with custom length bounds).

### Implementation Notes for Test Writer

1. **Test location:** Place TriggerState tests in `crates/twerk-core/src/trigger.rs` under
   `#[cfg(test)] mod tests { ... }` (matching `job.rs` pattern). Place TriggerId tests
   in `crates/twerk-core/src/id.rs` under the existing `#[cfg(test)] mod tests { ... }`.

2. **Proptest dependency:** `proptest` is in the workspace `Cargo.toml` but NOT in
   `twerk-core/Cargo.toml` dev-dependencies. The test writer must add
   `proptest.workspace = true` to `[dev-dependencies]` in `crates/twerk-core/Cargo.toml`.

3. **Serde test pattern:** Use `serde_json::to_string` and `serde_json::from_str` for
   all serde tests. The existing codebase uses this pattern (see `domain_types.rs` tests).

4. **No `is_ok()` / `is_err()` without further assertion:** Every test must assert the
   specific value or specific error variant. Use `matches!` for error variant checking
   and `.unwrap_err()` for error Display message assertions.

5. **DO NOT copy banned patterns from existing `id.rs` tests:**
   - `id.rs:179` uses bare `is_ok()` — do NOT copy this pattern.
   - `id.rs:393-394` uses bare `is_err()` — do NOT copy this pattern.
   - `id.rs:216-230` uses a `for` loop in test body for special char sweep — use proptest instead.
   All new tests must use concrete assertions per the discipline in this plan.

6. **IdError::TooShort is a new variant:** The existing `IdError` uses `#[derive(Error)]`
   from `thiserror`. The new variant needs `#[error("ID is too short: {0} characters (minimum 3)")]`.

7. **TriggerId does NOT use `define_id!`:** Per the contract, TriggerId is hand-written
   to enforce the 3-64 length constraint. All trait impls (Display, AsRef, Deref, Borrow,
   FromStr, Default, Serialize, Deserialize, From<String>, From<&str>) must be hand-written
   following the same pattern as the macro output.

8. **Serde deserialization error assertions:** For serde deserialization rejection tests,
   assert on `result.is_err() == true` AND `result.unwrap_err().to_string().contains("...")`.
   Serde produces `serde_json::Error` (a concrete type, not a trait). Assert on the error
   message content to verify the correct validation error propagated through serde.

9. **Use `rstest` for parameterization where appropriate:** The `rstest` crate is already
   in dev-dependencies. Use `#[rstest]` to reduce boilerplate for variant-based tests
   (e.g., Display/serde for all 4 TriggerState variants, all 4 serde deserialize cases).
   Each `#[case]` still counts as a separate logical test.
