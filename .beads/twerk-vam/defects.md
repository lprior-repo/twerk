# Black Hat Review: twerk-vam Defects Report

**Review Date**: 2026-04-14  
**Files Reviewed**:
- `/home/lewis/src/twerk-vam/crates/twerk-core/src/domain/webhook_url.rs`
- `/home/lewis/src/twerk-vam/crates/twerk-core/src/domain/hostname.rs`
- `/home/lewis/src/twerk-vam/crates/twerk-core/src/domain/cron_expression.rs`

---

## PHASE 1: Contract & Bead Parity — ❌ REJECTED

### CRITICAL: Contract-Implementation Mismatch (WebhookUrl)

**Issue #1: Wrong Error Type for URL Parse Errors**

| Aspect | Contract (Line 62-65) | Implementation (Line 28-29) |
|--------|----------------------|----------------------------|
| Error variant | `UrlParseError(ParseError)` | `UrlParseError(String)` |
| Inner type | `url::ParseError` (opaque crate type) | `String` |

**Contract specifies**: The error taxonomy explicitly shows `url::ParseError` as the inner type, not `String`. This matters because callers may want to inspect the specific parse error variant.

**Impact**: Lines 58-59 convert the url crate's `ParseError` to `String`, losing type information. This violates Parse, Don't Validate - the domain should preserve the parse error for debugging.

---

**Issue #2: Undocumented Error Variants (RQ-002, RQ-003)**

The following error variants exist in the implementation but are **NOT defined in the contract**:

| Error Variant | Location | Contract Reference |
|--------------|----------|-------------------|
| `UrlTooLong` | webhook_url.rs:34-35 | NOT IN CONTRACT |
| `SpaceInPath` | webhook_url.rs:36-37 | NOT IN CONTRACT |

**Contract Error Taxonomy (Lines 62-67)** defines exactly 3 variants:
- `UrlParseError(ParseError)`
- `InvalidScheme(Scheme)`
- `MissingHost`

The RQ-002 and RQ-003 fixes added new error variants without updating `contract.md`. This is a **contract drift violation**.

---

### Hostname Contract Parity: ✅ PASS

All preconditions, postconditions, invariants, and error variants match the contract exactly.

---

### CronExpression Contract Parity: ✅ PASS (with note)

The `to_uppercase()` normalization at line 57 correctly addresses RQ-001. The contract does not specify case handling, but the implementation correctly normalizes for the cron parser while preserving the original input.

---

## PHASE 2: Farley Engineering Rigor — ❌ REJECTED

### CRITICAL: Function Length Violation

| Function | Lines | Limit | Excess |
|----------|-------|-------|--------|
| `Hostname::new()` | 49-128 (79 lines) | 25 | +54 lines (316% over) |
| `CronExpression::new()` | 41-74 (33 lines) | 25 | +8 lines (32% over) |
| `WebhookUrl::new()` | 49-79 (30 lines) | 25 | +5 lines (20% over) |

**Hostname::new() is 3x the allowed size**. This function performs 5 distinct validation passes that should be extracted into helper functions or a state machine.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6) — ⚠️ CONDITIONAL PASS

### Panic Vector Violations (hostname.rs)

| Line | Code | Issue |
|------|------|-------|
| 64 | `s.chars().nth(colon_pos).unwrap()` | `unwrap()` in domain validation |
| 90 | `label.chars().next().unwrap()` | `unwrap()` in domain validation |
| 99 | `label.chars().last().unwrap()` | `unwrap()` in domain validation |

**Analysis**: While preconditions guarantee these values exist (e.g., `colon_pos` from `find(':')` implies the char exists), using `unwrap()` in domain validation code is forbidden by The Panic Vector. The contract says "make illegal states unrepresentable" - but `unwrap()` asserts rather than prevents.

**Required Fix**: Use `expect()` with explicit message, or restructure to avoid iterator unwrap.

---

### WebhookUrl Deserialization Redundancy

The implementation stores both `inner: String` AND `parsed: url::Url` (lines 20-23). This is a space-time tradeoff that violates YAGNI. The contract only requires `as_str()` and `as_url()` - the implementation preemptively cached parsing.

**Verdict**: Acceptable for performance, but the `parsed` field adds complexity to serialization (requires custom `Serialize`/`Deserialize` impl vs `#[serde(transparent)]`).

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin) — ✅ PASS

### CUPID Analysis

| Property | WebhookUrl | Hostname | CronExpression |
|----------|-----------|----------|---------------|
| Composable | ✅ | ✅ | ✅ |
| Unix-philosophy | ✅ | ✅ | ✅ |
| Predictable | ✅ | ✅ | ✅ |
| Idiomatic | ✅ | ✅ | ✅ |
| Domain-based | ✅ | ✅ | ✅ |

All three types follow DDD principles correctly:
- Transparent wrappers with validation at construction
- Explicit error types (not `Option`-based)
- Immutable data structures
- No `mut` in domain logic

---

## PHASE 5: The Bitter Truth (Velocity & Legibility) — ✅ PASS

### Sniff Test: ✅ PASS

Code is readable, boring, and obvious. No clever combinators or gratuitous abstraction. Error messages are clear and actionable.

**WebhookUrl error messages** (lines 28-37):
```
"URL parse error: {0}"
"invalid scheme: {0} (must be http or https)"
"URL has no host component"
"URL exceeds maximum length of 2048 characters"
"URL path contains unencoded spaces"
```

These are properly descriptive. However, `UrlTooLong` and `SpaceInPath` should be documented in contract.md.

---

## SUMMARY OF DEFECTS

| Phase | Severity | Issue | File | Lines |
|-------|----------|-------|------|-------|
| 1 | CRITICAL | Wrong error type: `UrlParseError(String)` instead of `UrlParseError(ParseError)` | webhook_url.rs | 28-29, 58-59 |
| 1 | MAJOR | `UrlTooLong` not in contract | webhook_url.rs | 34-35 |
| 1 | MAJOR | `SpaceInPath` not in contract | webhook_url.rs | 36-37 |
| 2 | CRITICAL | `Hostname::new()` is 79 lines (limit: 25) | hostname.rs | 49-128 |
| 2 | MAJOR | `CronExpression::new()` is 33 lines (limit: 25) | cron_expression.rs | 41-74 |
| 3 | MINOR | `unwrap()` in domain validation | hostname.rs | 64, 90, 99 |

---

## VERDICT

```
STATUS: REJECTED
```

### Required Actions for Re-Approval:

1. **[CRITICAL - Phase 1]**: Update `contract.md` error taxonomy to include `UrlTooLong` and `SpaceInPath`, OR remove these error variants from the implementation.

2. **[CRITICAL - Phase 1]**: Either:
   - Change `UrlParseError(String)` to `UrlParseError(ParseError)` and import `url::ParseError`, OR
   - Update contract.md to specify `String` as the inner type

3. **[CRITICAL - Phase 2]**: Refactor `Hostname::new()` into helper functions. Suggested structure:
   - `fn validate_length(s: &str) -> Result<(), HostnameError>`
   - `fn validate_no_colon(s: &str) -> Result<(), HostnameError>`
   - `fn validate_labels(s: &str) -> Result<(), HostnameError>`

4. **[MINOR - Phase 3]**: Replace `unwrap()` with explicit `expect()` or restructure to avoid iterator unwrapping in domain code.

---

*Reviewed by: Black Hat Reviewer*
*Skill: black-hat-reviewer v1.0*
