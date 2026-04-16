# Test Review Round 4 — twerk-web

## VERDICT: REJECTED

---

### Tier 0 — Static
[PASS] Banned assertions (is_ok/is_err)
[PASS] Silent error discard (let _, .ok())
[PASS] Ignored tests
[PASS] Sleep in tests
[PASS] Naming violations
[FAIL] Holzmann Rule 2 (loop in test body)
[PASS] Shared mutable state
[PASS] Mock interrogation
[PASS] Error variant completeness
[FAIL] Density: 111 tests / 56 functions = 1.98x (target ≥5x)

### Tier 1 — Execution
[FAIL] Clippy: 11 warnings/errors
[FAIL] nextest: 6 tests FAILED consistently (InvalidUuid panic), 0 flaky
[PASS] Ordering probe: consistent
[N/A] Insta: not in project

### Tier 2 — Coverage
[SKIP] Line coverage: blocked by compilation failures

### Tier 3 — Mutation
[FAIL] Kill rate: CANNOT RUN — tests fail in unmutated tree

---

## LETHAL FINDINGS

### 1. Loop in Test Body (Holzmann Rule 2)
**File:** `crates/twerk-web/src/api/yaml/tests.rs:699`

```rust
for name in files {
    let file = examples_dir.join(name);
    let content = fs::read_to_string(&file)
        .unwrap_or_else(|_| panic!("Failed to read {}", file.display()));
    let result: Result<serde_json::Value, _> = from_slice(content.as_bytes());
    assert!(result.is_ok(), ...);
}
```

This is in test function `parse_all_example_yaml_files`. The loop iterates over example YAML files and asserts each parses successfully. Per Holzmann Rule 2, test bodies should not contain loops — tests should be linear, atomic assertions.

**Required fix:** Convert to individual test cases via `rstest` or proptest, or use `try_from_slice` with iteration handled by the framework.

---

### 2. Test Density Far Below Target
**Density:** 111 tests / 56 public functions = **1.98x** (required ≥5x)

Public functions requiring test coverage:
- `api/yaml.rs`: `from_slice`, `to_string`
- `api/redact.rs`: `redact_job`, `redact_job_summary`, `redact_task`, `redact_task_log_parts`
- `api/domain/auth.rs`: `Username::new`, `Password::new`
- `api/domain/pagination.rs`: `Page::new`, `PageSize::new`
- `api/domain/api.rs`: `ServerAddress::new`, `ApiFeatureFlags`
- `api/trigger_api/domain.rs`: `TriggerId::parse`, `validate_trigger_update`, `apply_trigger_update`
- `api/trigger_api/datastore.rs`: `InMemoryDatastore::upsert/get_trigger_by_id/update_trigger`
- `middleware/hooks.rs`: `apply_job_middleware`, `apply_task_middleware`, `on_read_job`, etc.

111 tests against 56 functions is insufficient. The YAML parser tests (90+) inflate the count while domain logic is under-tested.

**Required fix:** Add ~170 more targeted tests for domain logic, redact functions, and middleware.

---

### 3. Clippy Violations Block Compilation
**Files:** Multiple

```
error: docs for function returning `Result` missing `# Errors` section
  --> crates/twerk-web/src/api/yaml.rs:38:1
error: variables can be used directly in the `format!` string
  --> crates/twerk-web/src/api/yaml.rs:47:44
error: used `unwrap()` on a `Result` value
  --> crates/twerk-web/src/api/domain/api.rs:212:20
error: used `unwrap()` on a `Result` value
  --> crates/twerk-web/src/api/domain/auth.rs:164:17
error: used `unwrap()` on a `Result` value
  --> crates/twerk-web/src/api/domain/auth.rs:186:17
error: used `unwrap()` on a `Result` value
  --> crates/twerk-web/src/api/domain/pagination.rs:166:20
error: used `unwrap()` on a `Result` value
  --> crates/twerk-web/src/api/domain/pagination.rs:177:20
error: module has the same name as its containing module
  --> crates/twerk-web/src/api/yaml/tests.rs:9:1
```

Total: 11 clippy errors. Tests will not compile in CI with `-D warnings`.

**Required fixes:**
- Add `# Errors` section to doc comments on `from_slice`, `to_string`
- Change `format!("YAML parse error: {}", e)` to `format!("YAML parse error: {e}")`
- Replace `unwrap()` with `expect()` in test code, or use `?` operator
- Rename `yaml/tests.rs` module to avoid name collision with `yaml.rs`

---

### 4. Integration Tests Panicking on InvalidUuid
**Files:** `crates/twerk-web/tests/api_endpoints_test.rs:68`, `crates/twerk-web/tests/api_endpoints_test.rs:94`, `crates/twerk-web/tests/comprehensive_api_test.rs`

```rust
let job_id = JobId::new("test-job-for-task").unwrap();
//                                     ^^^^^^^^^^^^^^^^^^^^^^^ InvalidUuid
```

`JobId::new` requires a valid UUID format (e.g., `00000000-0000-0000-0000-000000000001`), but tests pass hyphenated strings like `"test-job-for-task"`.

Affected tests:
- `cancel_job_returns_ok_when_job_is_running` (line 68)
- `get_task_log_respects_pagination` (line 94)
- `get_task_log_returns_empty_when_no_logs` (line 94)
- `get_task_log_returns_logs_when_exist` (line 94)
- `get_task_returns_task_when_exists` (line 94)
- `jobs_cancel_returns_ok_for_running_job` (comprehensive_api_test.rs:219)
- `jobs_restart_returns_ok_for_failed_job` (comprehensive_api_test.rs:248)
- `jobs_get_returns_job` (comprehensive_api_test.rs:172)
- `jobs_list_returns_job_list` (comprehensive_api_test.rs:142)

**Required fix:** Use valid UUIDs: `JobId::new("00000000-0000-0000-0000-000000000001").unwrap()`

---

### 5. Mutation Testing Blocked
`cargo mutants` cannot run because tests fail in unmutated tree (due to finding #4 above). Kill rate is unknown — this is a **LETHAL gate failure**.

---

## MAJOR FINDINGS (1)

### 1. ApiError::BadRequest Lacks Variant-Specific Test
**File:** `crates/twerk-web/src/api/error.rs`

`ApiError` has 3 variants: `BadRequest`, `NotFound`, `Internal`. The tests cover:
- `NotFound` via `from_datastore_error_maps_*` (6 tests)
- `Internal` via `from_datastore_error_maps_unknown_to_internal` and `from_anyhow_error_maps_to_internal`

**Missing:** Direct test for `ApiError::BadRequest` behavior. While `into_response_bad_request_preserves_message` tests the `IntoResponse` impl, there is no test that creates `ApiError::BadRequest(...)` directly and verifies its behavior.

**Required test:** A test like `api_error_bad_request_variant_preserves_message` that directly constructs `ApiError::bad_request("...")` and verifies it.

---

## MINOR FINDINGS (0)
No additional findings below MAJOR threshold.

---

## MANDATE

The following must exist before resubmission:

1. **[CRITICAL]** `crates/twerk-web/tests/api_endpoints_test.rs:68,94` — Fix `JobId::new` calls to use valid UUID format (e.g., `"00000000-0000-0000-0000-000000000001"`)
2. **[CRITICAL]** `crates/twerk-web/tests/comprehensive_api_test.rs:142,172,219,248` — Same UUID fix
3. **[CRITICAL]** `crates/twerk-web/src/api/yaml.rs:38,50` — Add `# Errors` section to doc comments
4. **[CRITICAL]** `crates/twerk-web/src/api/yaml.rs:47,52` — Use inline format args: `{e}` instead of `{} e`
5. **[CRITICAL]** `crates/twerk-web/src/api/yaml/tests.rs` — Fix `module_inception` by renaming the `tests` module
6. **[CRITICAL]** `crates/twerk-web/src/api/domain/api.rs:212`, `auth.rs:164,186`, `pagination.rs:166,177` — Replace `unwrap()` with proper error handling in tests
7. **[CRITICAL]** `crates/twerk-web/src/api/yaml/tests.rs:699` — Eliminate `for name in files` loop in `parse_all_example_yaml_files` test
8. **[HIGH]** Add ~170 additional tests to reach 5x density target
9. **[MEDIUM]** Add direct test for `ApiError::BadRequest` variant

**STATUS: REJECTED** — Suite Inquisition halted at Tier 1. Fix all LETHAL findings and re-run from Tier 0.
