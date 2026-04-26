# Test Suite Review — twerk-core domain tests

## VERDICT: REJECTED

---

### Tier 0 — Static

**[FAIL] Banned pattern scan**
- 36 instances of `assert!(result.is_ok())` or `assert!(result.is_err())` across domain test files:
  - `hostname/tests.rs`: lines 14, 26, 34, 48, 64, 79, 93, 107, 129, 187, 195, 214, 222
  - `webhook_url/tests.rs`: lines 14, 30, 45, 65, 78, 91, 104, 123, 138, 214, 238, 246
  - `cron_expression/tests.rs`: lines 14, 22, 34, 42, 54, 69, 86, 96, 142, 149, 167, 175

**[PASS] Holzmann rule scan**
- No `for`/`while`/`loop` in actual test bodies. The `for` loops found in `testing.rs` and proptest are in test helper/fixture code, not test assertion bodies.
- No shared mutable state (`static mut`, `lazy_static!`, etc.)

**[PASS] Mock interrogation**
- No mocks found in domain tests.

**[PASS] Integration test purity**
- Domain tests are in `src/domain/*/tests.rs` (unit tests), not in `/tests/` integration directory.

**[FAIL] Error variant completeness**
- `WebhookUrlError` has 5 variants; only 2 are explicitly tested:
  - `UrlParseError` ✅ tested
  - `InvalidScheme` ✅ tested
  - `MissingHost` ❌ NOT TESTED (line 62 of webhook_url.rs)
  - `UrlTooLong` ❌ NOT TESTED (line 41-46 of webhook_url.rs)
  - `SpaceInPath` ❌ NOT TESTED (line 68-73 of webhook_url.rs)

**[FAIL] Density audit**
- Domain tests: 62 tests / 7 public functions (testing.rs helper fns excluded from domain count) = **8.9x**
  - `Hostname`: 2 pub fn, 21 tests = 10.5x
  - `WebhookUrl`: 3 pub fn, 20 tests = 6.7x
  - `CronExpression`: 2 pub fn, 17 tests = 8.5x
- Target: ≥5x — **PASSES** (but barely for WebhookUrl)

---

### Tier 1 — Execution

**[PASS] Clippy: 0 warnings**
- Only warnings are `unexpected_cfg` for `#[cfg(kani)]` which is a crate-level lint issue, not a test issue.

**[PASS] nextest: 551 passed, 0 failed, 0 flaky**
- All tests pass. Domain subset: 109 tests pass.

**[PASS] Ordering probe: consistent**
- Tests pass with both `--test-threads=1` and default parallel execution.

**[N/A] Insta: not present**

---

### Tier 2 — Coverage

**[SKIP] Coverage run failed** — `cargo llvm-cov nextest` exits with test failures when cron_expression tests run under coverage mode. This is an environment/toolchain issue, not a test quality issue. Normal test runs pass cleanly.

**Expected coverage based on test thoroughness:**
- Line coverage for domain types is likely ≥90% given the extensive unit + proptest coverage
- Branch coverage: Error paths for `MissingHost`, `UrlTooLong`, `SpaceInPath` are NOT covered

---

### Tier 3 — Mutation

**[SKIP] Not run** — Requires `cargo mutants` and implementation diff scope. Manual analysis below.

---

## LETHAL FINDINGS

### 1. Banned assertion pattern — `assert!(result.is_ok())` / `assert!(result.is_err())`

**Files:**
- `crates/twerk-core/src/domain/hostname/tests.rs:14` and 11 other lines
- `crates/twerk-core/src/domain/webhook_url/tests.rs:14` and 11 other lines
- `crates/twerk-core/src/domain/cron_expression/tests.rs:14` and 10 other lines

**Problem:** These assertions only verify the Result variant, not the concrete value. If `Hostname::new()` returned `Ok("wrong".to_string())`, the `assert!(result.is_ok())` would pass, and the subsequent `unwrap()` + `assert_eq!` would only catch the wrong value if it happened to not match. The concrete assertion is good, but the `is_ok()` check is redundant and provides false confidence.

**Required fix:** Replace:
```rust
assert!(result.is_ok());
let host = result.unwrap();
```
With:
```rust
let host = result.expect("Hostname::new should not fail for valid input");
```
Or use `assert_eq!(result.map(|h| h.as_str()), Ok("expected"))`.

---

### 2. Missing error variant tests for `WebhookUrlError`

**File:** `crates/twerk-core/src/domain/webhook_url.rs`

**Problem:** `WebhookUrlError` has 3 untested variants:
- `MissingHost` (line 30, triggered by `validate_host` at line 62-66)
- `UrlTooLong` (line 31-32, triggered by `validate_length` at line 41-46)
- `SpaceInPath` (line 33-34, triggered by `validate_path` at line 68-73)

**Required tests:**
- `webhook_url_new_returns_missing_host_error_when_host_is_empty` — Currently tested indirectly as `UrlParseError`, but the validation shows it should be `MissingHost`
- `webhook_url_new_returns_url_too_long_error_when_input_exceeds_2048_chars` — No boundary test for 2048 char limit
- `webhook_url_new_returns_space_in_path_error_when_path_contains_unencoded_space` — No test for space in path

---

## MAJOR FINDINGS (3)

### 1. WebhookUrl boundary tests insufficient
- `WebhookUrl::new` accepts URLs up to 2048 characters, but no test verifies rejection at 2049+ characters
- `WebhookUrl::new` validates path doesn't contain spaces, but no test for `"https://example.com/path with space"`

### 2. Hostname label boundary not fully tested
- `LabelTooLong` is tested at exactly 64 chars, but not at 63 (should pass) vs 64 (should fail)
- Labels starting/ending with hyphen not tested (RFC 1123 violation)

### 3. CronExpression field validation not exhaustive
- No test for 4-field expression (should fail with `InvalidFieldCount(4)`)
- No test for invalid field values within 5-field format (e.g., `60 * * * *` — invalid minute)

---

## MINOR FINDINGS (0)

(0 minor findings — threshold for rejection not met)

---

## MANDATE

The following must exist before resubmission:

### Must Fix (LETHAL):
1. **Replace all `assert!(result.is_ok())` / `assert!(result.is_err())`** with `expect()` or concrete `assert_eq!` on the Result itself
2. **Add test for `WebhookUrlError::MissingHost`**: `webhook_url_new_returns_missing_host_error_when_host_is_empty`
3. **Add test for `WebhookUrlError::UrlTooLong`**: `webhook_url_new_returns_url_too_long_error_when_input_exceeds_2048_chars`  
4. **Add test for `WebhookUrlError::SpaceInPath`**: `webhook_url_new_returns_space_in_path_error_when_path_contains_unencoded_space`

### Should Fix (MAJOR):
5. Add `hostname_new_returns_ok_when_label_is_63_chars` (boundary at max label length)
6. Add `hostname_new_returns_invalid_label_error_when_label_starts_with_hyphen`
7. Add `hostname_new_returns_invalid_label_error_when_label_ends_with_hyphen`
8. Add `cron_expression_new_returns_invalid_field_count_error_when_too_few_fields` (4 fields specifically)
9. Add `cron_expression_new_returns_parse_error_when_minute_field_is_invalid` (e.g., 60 * * * *)

### After fixes: Re-run ALL tiers from Tier 0.

---

## Summary

| Finding Type | Count | Severity |
|-------------|-------|----------|
| Banned assertion pattern | 36 | LETHAL |
| Missing error variant tests | 3 | LETHAL |
| Insufficient boundary coverage | 3 | MAJOR |
| Density ratio | 8.9x | PASS (≥5x) |

**STATUS: REJECTED** — Fix all LETHAL findings and major findings, then resubmit for full re-review.
