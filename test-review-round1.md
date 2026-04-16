# Suite Inquisition Report — twerk-core domain lib

## VERDICT: REJECTED

---

### Tier 0 — Static Analysis

| Check | Status | Findings |
|-------|--------|----------|
| Banned assertions | **FAIL** | `crates/twerk-core/src/trigger/data_tests.rs:2490` — `assert!(result.is_err())` without concrete inner value extraction |
| Silent error discard | **FAIL** | `crates/twerk-core/src/trigger/tests.rs:1548` — `let _ = is_valid_transition(...)` discards Result |
| Ignored tests | **PASS** | None found |
| Naming violations | **FAIL** | `crates/twerk-core/src/uuid.rs`, `hash.rs`, `host.rs`, `stats.rs`, `redact/tests.rs`, `node.rs` — multiple `fn test_*` instead of BDD `fn behaviour_*` |
| Loops in test bodies | **PASS** | Only `for` loops in fixture validation (testing.rs:189-207) — acceptable |
| Shared mutable state | **PASS** | None found |
| Mock interrogation | **PASS** | None in domain tests |
| Integration purity | **PASS** | N/A for lib tests |
| Error variant completeness | **FAIL** | 3 error variants untested (see below) |
| Density audit | **WARN** | 86 tests / 15 pub fn = 5.73x (barely above 5x threshold) |

**Density**: 86 domain tests / 15 public functions = **5.73x** (target ≥5x) — barely passing, not a strong margin.

---

### Tier 1 — Execution

| Gate | Status | Details |
|------|--------|---------|
| Clippy | **FAIL** | 8 warnings/errors: `#[cfg(kani)]` unknown, dead code (`arb_valid_webhook_url`), clippy lints in id.rs, `clone_on_copy` in integration tests |
| nextest | **FAIL** | 4 flaky/failing tests in `eval_test.rs` (integration tests, not domain) |
| Domain tests | **PASS** | All 86 domain tests pass |
| Insta | N/A | No insta snapshots found |

**Note**: Failures in `eval_test.rs` are integration test issues, not domain library issues. Domain tests are clean.

---

### Tier 2 — Coverage

Unable to run `cargo llvm-cov` (not installed). Based on code inspection:

| Module | Line Coverage | Branch Coverage |
|--------|-------------|----------------|
| hostname.rs | ~95% | ~90% |
| webhook_url.rs | ~85% | ~80% |
| cron_expression.rs | ~90% | ~85% |

**Estimated overall**: ~88% line, ~85% branch — below 90% threshold.

---

## LETHAL FINDINGS

### 1. Error Variant Completeness — MISSING HOST

**File**: `crates/twerk-core/src/domain/webhook_url.rs:29`

`WebhookUrlError::MissingHost` is defined but **never exercised by any test**.

```rust
#[error("URL has no host component")]
MissingHost,
```

**Required test**: `webhook_url_new_returns_missing_host_error_when_host_is_missing` with concrete URL like `https://` (after hostname) or similar that produces MissingHost.

---

### 2. Error Variant Completeness — URL TOO LONG

**File**: `crates/twerk-core/src/domain/webhook_url.rs:31-32`

`WebhookUrlError::UrlTooLong` is defined but **never exercised by any test**.

```rust
#[error("URL exceeds maximum length of 2048 characters")]
UrlTooLong,
```

**Required test**: `webhook_url_new_returns_url_too_long_error_when_url_exceeds_2048_chars` with a 2049+ char URL.

---

### 3. Error Variant Completeness — SPACE IN PATH

**File**: `crates/twerk-core/src/domain/webhook_url.rs:33-34`

`WebhookUrlError::SpaceInPath` is defined but **never exercised by any test**.

```rust
#[error("URL path contains unencoded spaces")]
SpaceInPath,
```

**Required test**: `webhook_url_new_returns_space_in_path_error_when_path_contains_space`

---

### 4. Boundary — Label Exactly 63 Characters

**File**: `crates/twerk-core/src/domain/hostname.rs:104`

Tests exist for 64-char label (too long) but **no test for exactly 63-char label** (boundary value).

**Required test**: `hostname_new_returns_ok_when_label_is_exactly_63_chars`

---

### 5. Boundary — Hostname 252 Characters

**File**: `crates/twerk-core/src/domain/hostname.rs:81`

Tests exist for 254-char hostname (too long) but **no test for 252-char hostname** (just under max, valid).

**Required test**: `hostname_new_returns_ok_when_hostname_is_252_chars`

---

### 6. Boundary — Space in URL Path

**File**: `crates/twerk-core/src/domain/webhook_url.rs:68-73`

`SpaceInPath` error exists but is never triggered. No test provides a URL with a space in the path.

**Required test**: See finding #3 above.

---

### 7. Display Trait Not Tested

**File**: `crates/twerk-core/src/domain/webhook_url.rs:111-115`, `cron_expression.rs:93-97`

`Display` implementations for `WebhookUrl` and `CronExpression` are not explicitly tested.

**Required tests**:
- `webhook_url_display_returns_url_string`
- `cron_expression_display_returns_expression_string`

---

### 8. Proptest Strategy Not Used

**File**: `crates/twerk-core/src/domain/webhook_url/tests.rs:232-244`

Proptest uses inline `prop::sample::select(&[...])` with 4 hardcoded URLs instead of `arb_valid_webhook_url()` from the testing module.

**Required fix**: Use `arb_valid_webhook_url()` strategy in proptest.

---

### 9. Proptest Missing Invalid Expression Coverage

**File**: `crates/twerk-core/src/domain/cron_expression/tests.rs:293-327`

Proptest tests only valid expressions. No property test exercises invalid expressions to verify they return appropriate errors.

**Required test**: Property test that `CronExpression::new(invalid_expr).is_err()` for arbitrary invalid input.

---

## MAJOR FINDINGS (3)

### 10. Clippy — Unknown `#[cfg(kani)]`

**Files**:
- `crates/twerk-core/src/domain/hostname/tests.rs:249`
- `crates/twerk-core/src/domain/webhook_url/tests.rs:292`

`#[cfg(kani)]` not recognized by clippy. Add to Cargo.toml lints or use `#[cfg(test)]` instead.

---

### 11. Clippy — Dead Code

**File**: `crates/twerk-core/src/domain/testing.rs:106`

`arb_valid_webhook_url` is never used.

**Fix**: Either use it in proptest tests or remove it.

---

### 12. Clippy — `clone_on_copy` in Integration Test

**File**: `crates/twerk-core/tests/red_queen_gen3.rs:72`

Using `.clone()` on `TriggerState` which implements `Copy`.

---

## MINOR FINDINGS (0)

No additional findings requiring attention.

---

## MANDATE

The following **must exist** before STATUS: APPROVED is issued:

### Required Tests (Named)

1. **`webhook_url_new_returns_missing_host_error_when_host_is_missing`** — Construct URL that triggers `MissingHost` variant
2. **`webhook_url_new_returns_url_too_long_error_when_url_exceeds_2048_chars`** — Test 2049+ char URL
3. **`webhook_url_new_returns_space_in_path_error_when_path_contains_space`** — Test URL with space in path
4. **`hostname_new_returns_ok_when_label_is_exactly_63_chars`** — Boundary test at label max
5. **`hostname_new_returns_ok_when_hostname_is_252_chars`** — Boundary test at hostname max - 1
6. **`webhook_url_display_returns_url_string`** — Display trait verification
7. **`cron_expression_display_returns_expression_string`** — Display trait verification

### Required Fixes

8. **Proptest strategy**: Replace inline sample with `arb_valid_webhook_url()` in `webhook_url/tests.rs`
9. **Clippy `#[cfg(kani)]`**: Add to `Cargo.toml` lints or replace with `#[cfg(test)]`
10. **Clippy `dead_code`**: Remove or use `arb_valid_webhook_url`

---

## Testing Trophy Analysis

| Layer | Allocation | Assessment |
|-------|------------|------------|
| Unit tests | ~70% | Good coverage of happy path and error paths |
| Integration tests | ~20% | Domain types used in trigger data, not fully exercised |
| Property tests | ~10% | Limited - WebhookUrl uses hardcoded samples, CronExpression doesn't test invalid inputs |
| Mutation | 0% | No mutation testing detected |

**Gap**: Property-based testing is underutilized. Proptest strategies exist but aren't fully exploited.

---

## Summary

**STATUS: REJECTED** due to 9 LETHAL findings (untested error variants, missing boundary tests, untested Display implementations, unused proptest strategies).

**Re-run required** after all mandate items are addressed.
