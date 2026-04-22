# Design-by-Contract Specification: twerk-0gr

**Bead**: twerk-0gr
**Crate**: twerk-cli
**Scope**: Fix broken test infra + handler error-body drops + add missing trigger tests

---

## 1. Context

### Feature Description

Three related bug-fix streams in `crates/twerk-cli`:

1. **Test infra Holzmann violations** — `bdd_behavior_report.rs` and `bdd_behavioral_contract_test.rs` contain patterns banned by the project's Holzmann test rules. Internal `#[cfg(test)]` modules in source files violate the `test_` naming convention.

2. **Handler error-body drops** — `queue.rs` and `task.rs` check HTTP status codes *before* reading the response body. When the server returns an error (400/404/500), the response body — which contains the server's structured error message — is dropped. Users see generic `HttpStatus` or `NotFound` errors instead of the server's actual error detail. The correct pattern exists in `trigger.rs`: read `.text()` first, then branch on status, attempt `TriggerErrorResponse` parse, fall back to `HttpStatus`.

3. **Missing trigger tests** — No coverage for negative HTTP statuses (400/404/409/500), TriggerId boundary validation (min 3 chars, max 64 chars, charset `[a-zA-Z0-9_-]`), or mutation kill verification.

### Domain Terms

| Term | Definition |
|------|-----------|
| **Error-body drop** | Handler checks `response.status()` then returns `CliError::HttpStatus` without calling `response.text().await`, discarding the server's error payload |
| **TriggerErrorResponse** | Server-side error envelope: `{ "error": String, "message": String, "path_id": Option<String>, "body_id": Option<String> }` |
| **TriggerId** | Server-side validated identifier: 3–64 chars, charset `[a-zA-Z0-9_-]`. No CLI-side validation; server rejects with 400 |
| **Holzmann rules** | Project-enforced test discipline: Rule 2 (no loops in test bodies), Rule 7 (no `LazyLock<Mutex>` in tests), banned `is_err()` assertions, banned `let _ =` silent suppressions |
| **test_ naming** | All functions inside `#[cfg(test)] mod` blocks in source files must be prefixed `test_` |

### Assumptions

- The `CliError` enum variants are stable; no variants are being added or removed.
- The `TriggerErrorResponse` struct in `trigger.rs` is the canonical error-deserialization shape. Queue and task handlers will reuse it or an equivalent struct.
- Server-side error responses for queue/task endpoints use the same JSON envelope as trigger endpoints (`{ "error": ..., "message": ... }`).
- `reqwest` workspace dependency is available and provides the HTTP client.
- `percent-encoding` is available transitively via `reqwest → url` and may need to be added as a direct dependency for path-segment encoding.
- The `start_test_server()` helper from `twerk_web::helpers` is available for integration tests.

### Open Questions

1. **Queue/task error envelope shape** — Do queue and task server endpoints return the same `{ "error", "message" }` envelope as trigger endpoints? If not, the `TriggerErrorResponse` struct should be generalized or a separate struct introduced. **Resolution assumption**: same envelope shape; reuse `TriggerErrorResponse` by moving it to a shared location or importing from `trigger.rs`.
2. **URL encoding scope** — Should all handler path-segment IDs (trigger, queue, task, node, metrics, user) be URL-encoded, or only queue/task (the bead scope)? **Bead scope is queue/task only**; node/metrics/user handlers are out of scope but should be noted as follow-up.

---

## 2. Preconditions

### P1: Test Infra Fixes

| ID | Condition | File |
|----|-----------|------|
| P1.1 | `bdd_behavior_report.rs` contains a `for` loop in `claim_14_cli_error_display_messages` (line 187) iterating over error/display pairs — Holzmann Rule 2 violation | `tests/bdd_behavior_report.rs:187` |
| P1.2 | `bdd_behavior_report.rs` uses `LazyLock<Mutex<()>>` at line 33 for `LOGGING_ENV_LOCK` — Holzmann Rule 7 violation | `tests/bdd_behavior_report.rs:33` |
| P1.3 | `bdd_behavior_report.rs` uses `result.is_err()` assertions in claims 17, 18, 19 — banned assertion pattern | `tests/bdd_behavior_report.rs:216,229,235` |
| P1.4 | `bdd_behavior_report.rs` uses `let _ =` in adversarial module completeness_check (lines 285–292) and boundary_check (line 299) | `tests/bdd_behavior_report.rs:285-292,299` |
| P1.5 | `bdd_behavioral_contract_test.rs` uses `let _ =` in `bdd_setup_logging` (line 257), `bdd_completeness_check` (lines 268–284, 290–304) | `tests/bdd_behavioral_contract_test.rs` |
| P1.6 | `banner.rs` internal `#[cfg(test)]` module has 8 tests without `test_` prefix | `src/banner.rs:66-133` |
| P1.7 | `cli.rs` internal `#[cfg(test)]` module has mixed naming (some `test_` prefixed, some not) | `src/cli.rs` |

### P2: Handler Error-Body Drop Fixes

| ID | Condition | Site |
|----|-----------|------|
| P2.1 | `queue_list` checks `!status.is_success()` at line 24, returns `HttpStatus` without reading body | `handlers/queue.rs:22-28` |
| P2.2 | `queue_get` checks `NOT_FOUND` at line 69, returns `NotFound` without reading body | `handlers/queue.rs:69-71` |
| P2.3 | `queue_get` checks `!is_success()` at line 73, returns `HttpStatus` without reading body | `handlers/queue.rs:73-77` |
| P2.4 | `queue_delete` checks `NOT_FOUND` at line 108, returns `NotFound` without reading body | `handlers/queue.rs:108-109` |
| P2.5 | `queue_delete` checks `!is_success()` at line 112, returns `HttpStatus` without reading body | `handlers/queue.rs:112-116` |
| P2.6 | `task_get` checks `NOT_FOUND` at line 65, returns `NotFound` without reading body | `handlers/task.rs:65-67` |
| P2.7 | `task_get` checks `!is_success()` at line 69, returns `HttpStatus` without reading body | `handlers/task.rs:69-73` |
| P2.8 | `task_log` checks `NOT_FOUND` at line 141, returns `NotFound` without reading body | `handlers/task.rs:141-143` |
| P2.9 | `task_log` checks `!is_success()` at line 145, returns `HttpStatus` without reading body | `handlers/task.rs:145-149` |
| P2.10 | `queue.rs` interpolates `name` directly into URL via `format!()` — no percent-encoding | `handlers/queue.rs:63,101` |
| P2.11 | `task.rs` interpolates `task_id` directly into URL via `format!()` — no percent-encoding | `handlers/task.rs:59,124` |

### P3: Missing Trigger Tests

| ID | Condition |
|----|-----------|
| P3.1 | No test exists for trigger handlers receiving 400 Bad Request |
| P3.2 | No test exists for trigger handlers receiving 404 Not Found |
| P3.3 | No test exists for trigger handlers receiving 409 Conflict |
| P3.4 | No test exists for trigger handlers receiving 500 Internal Server Error |
| P3.5 | No test exists for TriggerId boundary: 2-char input (below min=3) |
| P3.6 | No test exists for TriggerId boundary: 65-char input (above max=64) |
| P3.7 | No test exists for TriggerId charset: special chars (`/`, `?`, spaces, etc.) |
| P3.8 | No mutation kill test verifies that removing `TriggerErrorResponse` parse causes test failure |

---

## 3. Postconditions

### Post1: Test Infra Fixes

| ID | Condition |
|----|-----------|
| Post1.1 | `claim_14` in `bdd_behavior_report.rs` expands the loop into individual `assert!` calls per error variant — no loop in test body |
| Post1.2 | `LOGGING_ENV_LOCK` is removed from `bdd_behavior_report.rs`; env-var tests use `std::sync::OnceLock` or per-test serialization via `serial_test` or equivalent single-thread strategy |
| Post1.3 | All `is_err()` assertions replaced with `match` on specific `CliError` variant |
| Post1.4 | All `let _ =` replaced with explicit `assert!` on the constructed value (e.g., `assert!(!format!("{:?}", expr).is_empty())`) or `drop()` with an accompanying assert |
| Post1.5 | All 8 tests in `banner.rs` `#[cfg(test)]` module renamed with `test_` prefix |
| Post1.6 | Mixed-name tests in `cli.rs` `#[cfg(test)]` module renamed with `test_` prefix where missing |
| Post1.7 | All existing tests pass after renaming (no behavioral change) |

### Post2: Handler Error-Body Drop Fixes

| ID | Condition |
|----|-----------|
| Post2.1 | `queue_list` reads `response.text().await` before checking status; on non-success, attempts `TriggerErrorResponse` parse, falls back to `HttpStatus` |
| Post2.2 | `queue_get` reads body first; on 404, attempts `TriggerErrorResponse` parse then falls back to `NotFound`; on other non-success, attempts parse then falls back to `HttpStatus` |
| Post2.3 | `queue_delete` reads body first; same pattern as Post2.2 |
| Post2.4 | `task_get` reads body first; same pattern as Post2.2 |
| Post2.5 | `task_log` reads body first; same pattern as Post2.2 |
| Post2.6 | All path-segment IDs in `queue.rs` and `task.rs` are percent-encoded via `percent_encoding::utf8_percent_encode(&segment, percent_encoding::NON_ALPHANUMERIC)` or equivalent |
| Post2.7 | When server returns structured JSON error, the CLI produces `CliError::ApiError { code, message }` with the server's message preserved |
| Post2.8 | When server returns non-JSON error body, the CLI falls back to `CliError::HttpStatus { status, reason }` |
| Post2.9 | All handler function signatures remain unchanged — only internal implementation changes |

### Post3: Missing Trigger Tests

| ID | Condition |
|----|-----------|
| Post3.1 | Tests exist for each trigger handler function (`trigger_list`, `trigger_get`, `trigger_create`, `trigger_update`, `trigger_delete`) receiving 400/404/409/500 responses from a mock server |
| Post3.2 | Tests verify that `CliError::ApiError` is returned with the server's error message when the server returns structured JSON |
| Post3.3 | Tests verify that `CliError::HttpStatus` is returned when server returns non-JSON error body |
| Post3.4 | TriggerId boundary test: 2-char ID returns error (server rejects) |
| Post3.5 | TriggerId boundary test: 65-char ID returns error (server rejects) |
| Post3.6 | TriggerId charset test: special chars in ID result in appropriate error |
| Post3.7 | TriggerId boundary test: 3-char ID (min valid) is accepted |
| Post3.8 | TriggerId boundary test: 64-char ID (max valid) is accepted |
| Post3.9 | Mutation kill test: test fails if `TriggerErrorResponse` parse path is removed |

---

## 4. Invariants

These must hold before, during, and after all changes.

| ID | Invariant |
|----|-----------|
| INV1 | All 15 `CliError` variants remain constructible with their current field types |
| INV2 | `CliError::kind()` mapping is unchanged for all variants |
| INV3 | `CliError::exit_code()` returns 1 for Runtime, 2 for Validation — unchanged |
| INV4 | `Error` trait impl (via `thiserror`) produces the same display strings for all variants |
| INV5 | `From<DsnError>` and `From<EndpointError>` conversions remain unchanged |
| INV6 | All existing passing tests continue to pass after changes |
| INV7 | Handler function signatures (`pub async fn ... -> Result<String, CliError>`) are unchanged |
| INV8 | Crate-level lints remain: `deny(unwrap_used, expect_used, panic)`, `forbid(unsafe_code)` |
| INV9 | No new `unwrap()`, `expect()`, or `panic!()` introduced |
| INV10 | No `unsafe` code introduced |
| INV11 | `json_mode` parameter behavior is preserved: `true` → print raw JSON, `false` → formatted table/output |
| INV12 | HTTP method per handler is unchanged (GET for list/get, DELETE for delete, POST for create, PUT for update) |
| INV13 | Successful response body is returned as `Ok(String)` — unchanged |

---

## 5. Error Taxonomy

Complete list of `CliError` variants with fields and error kind:

| Variant | Fields | ErrorKind | Exit Code | Display Format |
|---------|--------|-----------|-----------|----------------|
| `Config` | `String` | Validation | 2 | `"configuration error: {0}"` |
| `Http` | `reqwest::Error` (via `#[from]`) | Runtime | 1 | `"HTTP request failed: {0}"` |
| `HttpStatus` | `{ status: u16, reason: String }` | Runtime | 1 | `"HTTP error {status}: {reason}"` |
| `HealthFailed` | `{ status: u16 }` | Runtime | 1 | `"health check failed with status: {status}"` |
| `InvalidBody` | `String` | Runtime | 1 | `"invalid response body: {0}"` |
| `MissingArgument` | `String` | Validation | 2 | `"missing required argument: {0}"` |
| `Migration` | `String` | Runtime | 1 | `"migration error: {0}"` |
| `UnknownDatastore` | `String` | Validation | 2 | `"unsupported datastore type: {0}"` |
| `Logging` | `String` | Runtime | 1 | `"logging setup error: {0}"` |
| `Engine` | `String` | Runtime | 1 | `"engine error: {0}"` |
| `InvalidHostname` | `String` | Validation | 2 | `"invalid hostname: {0}"` |
| `InvalidEndpoint` | `String` | Validation | 2 | `"invalid endpoint: {0}"` |
| `NotFound` | `String` | Runtime | 1 | `"not found: {0}"` |
| `ApiError` | `{ code: u16, message: String }` | Runtime | 1 | `"API error {code}: {message}"` |
| `Io` | `std::io::Error` (via `#[from]`) | Runtime | 1 | `"IO error: {0}"` |

### Handler Error Mapping (Post-Fix)

After the fix, the error-body-drop sites will map server responses as follows:

```
Server Response → Handler Error Mapping:

1. Structured JSON body parseable as TriggerErrorResponse:
   → CliError::ApiError { code: status.as_u16(), message: err_resp.message }

2. Non-JSON or unparseable body, status == 404:
   → CliError::NotFound("{resource_type} {id} not found")

3. Non-JSON or unparseable body, status != 404:
   → CliError::HttpStatus { status: status.as_u16(), reason: canonical_reason }

4. Network failure:
   → CliError::Http(reqwest::Error)

5. Body read failure:
   → CliError::InvalidBody(error_message)
```

---

## 6. Contract Signatures

### 6.1 Modified Functions (signatures unchanged, implementation changed)

```rust
// handlers/queue.rs — body now read before status check, IDs percent-encoded

pub async fn queue_list(endpoint: &str, json_mode: bool) -> Result<String, CliError>;

pub async fn queue_get(endpoint: &str, name: &str, json_mode: bool) -> Result<String, CliError>;

pub async fn queue_delete(endpoint: &str, name: &str, json_mode: bool) -> Result<String, CliError>;
```

```rust
// handlers/task.rs — body now read before status check, IDs percent-encoded

pub async fn task_get(endpoint: &str, task_id: &str, json_mode: bool) -> Result<String, CliError>;

pub async fn task_log(
    endpoint: &str,
    task_id: &str,
    page: Option<i64>,
    size: Option<i64>,
    json_mode: bool,
) -> Result<String, CliError>;
```

### 6.2 Added Helper (private, shared error-response parsing)

```rust
// handlers/queue.rs or handlers/task.rs or a shared module — extracts ApiError from body

fn parse_api_error(status: u16, body: &str) -> Option<CliError>;
```

Returns `Some(CliError::ApiError { code, message })` if body parses as `TriggerErrorResponse`, `None` otherwise.

### 6.3 URL Encoding Helper (private)

```rust
fn encode_path_segment(segment: &str) -> String;
```

Returns `percent_encoding::utf8_percent_encode(segment, percent_encoding::NON_ALPHANUMERIC).to_string()`.

### 6.4 Renamed Tests (internal #[cfg(test)] modules)

#### `banner.rs` — 8 renames

| Old Name | New Name |
|----------|----------|
| `banner_mode_from_str_returns_expected_variant_for_supported_and_unknown_values` | `test_banner_mode_from_str_returns_expected_variants` |
| `banner_mode_from_str_is_case_insensitive_for_known_values` | `test_banner_mode_from_str_case_insensitive` |
| `banner_mode_from_str_treats_whitespace_wrapped_values_as_console_default` | `test_banner_mode_from_str_whitespace_defaults_to_console` |
| `banner_mode_default_returns_console` | `test_banner_mode_default_is_console` |
| `banner_constant_is_not_empty_and_contains_ascii_art_shape_markers` | `test_banner_constant_not_empty_with_ascii_art` |
| `banner_constant_contains_expected_branding_pattern` | `test_banner_constant_contains_branding` |
| `banner_mode_equality_and_inequality_are_consistent` | `test_banner_mode_equality` |
| `banner_mode_copy_semantics_preserve_value` | `test_banner_mode_copy_semantics` |

(`banner_mode_clone_semantics_preserve_value` → `test_banner_mode_clone_semantics`)

#### `cli.rs` — mixed naming renames

Descriptive snake_case tests without `test_` prefix renamed to `test_` prefix. Exact names determined by reading the `#[cfg(test)]` module in `cli.rs`.

---

## 7. Scope Boundaries

### In Scope

- `handlers/queue.rs` — error-body read pattern + URL encoding
- `handlers/task.rs` — error-body read pattern + URL encoding
- `tests/bdd_behavior_report.rs` — Holzmann Rule fixes
- `tests/bdd_behavioral_contract_test.rs` — `let _ =` fixes
- `src/banner.rs` — test naming fixes
- `src/cli.rs` — test naming fixes
- New test file or additions to `tests/trigger_contract_regression_test.rs` — negative status tests, TriggerId boundaries, mutation kill

### Out of Scope (Non-Goals)

- `handlers/node.rs`, `handlers/metrics.rs`, `handlers/user.rs` — body drops exist but are not in bead scope; file as follow-up bead
- `handlers/trigger.rs` — already follows correct pattern; no changes needed (only new tests)
- `error.rs` — no variant changes
- `cli.rs` dispatch logic — no changes
- `commands.rs` — no changes
- `run.rs`, `health.rs`, `migrate.rs` — no changes
- `tests/e2e_cli_test.rs` — no changes
- Adding new `CliError` variants
- Adding client-side TriggerId validation (server validates; client passes through)
- Any changes to `twerk-web`, `twerk-core`, `twerk-app`, `twerk-common`, `twerk-infrastructure`

---

## 8. Given-When-Then Scenarios

### 8.1 Test Infra: Loop Elimination (Rule 2)

**Scenario**: Expand `claim_14` loop into individual assertions

```
Given bdd_behavior_report.rs claim_14 iterates a Vec of (CliError, &str) pairs
When the loop is expanded into individual test functions or individual assert blocks
Then each error variant has its own assertion without a loop construct
And all 8 error display messages are still verified
```

### 8.2 Test Infra: LazyLock<Mutex> Removal (Rule 7)

**Scenario**: Remove `LOGGING_ENV_LOCK` from test infra

```
Given LOGGING_ENV_LOCK is a LazyLock<Mutex<()>> serializing env-var tests
When it is removed
Then tests that modify TWERK_LOGGING_LEVEL use an alternative serialization strategy
  (e.g., serial_test crate, or tests run without shared-state dependency)
And LoggingEnvGuard RAII pattern is preserved for env-var cleanup
And all logging tests still pass in single-threaded and multi-threaded test runners
```

### 8.3 Test Infra: `is_err()` Replacement

**Scenario**: Replace `is_err()` with specific variant matching

```
Given claim_17 uses assert!(result.is_err()) for health check connection failure
When replaced with match on Err(CliError::Http(_)) (specific variant)
Then the test still fails if setup_logging returns Ok
And the test fails if a different CliError variant is returned

Given claim_18 uses assert!(result.is_err()) for trailing slash test
When replaced with match on Err(CliError::Http(_))
Then the test still passes for connection failures

Given claim_19 uses assert!(result.is_err()) for unknown datastore rejection
When replaced with match on Err(CliError::UnknownDatastore(_))
Then the test still verifies the specific error variant
```

### 8.4 Test Infra: `let _ =` Replacement

**Scenario**: Replace `let _ =` with explicit assertions

```
Given bdd_behavior_report.rs adversarial::completeness_check uses let _ = CliError::Config(...)
When replaced with assert!(!format!("{:?}", CliError::Config(...)).is_empty())
Then the test still proves constructibility
And the compiler verifies the variant shape at test compile time

Given bdd_behavioral_contract_test.rs bdd_completeness_check uses let _ = for variant construction
When replaced with similar assert patterns
Then all variants are still verified constructible
```

### 8.5 Test Naming: banner.rs `test_` Prefix

**Scenario**: Add `test_` prefix to all 8 banner tests

```
Given banner.rs #[cfg(test)] mod tests has 8 functions without test_ prefix
When each function is renamed to test_<descriptive_name>
Then cargo test discovers all 8 tests
And all 8 tests pass
```

### 8.6 Handler: queue_list Error Body Preserved

**Scenario**: Server returns 500 with structured error

```
Given a queue_list request to an endpoint whose /queues returns HTTP 500
  And the response body is {"error":"internal","message":"database connection lost"}
When queue_list processes the response
Then the body is read via .text().await before status checking
And the result is Err(CliError::ApiError { code: 500, message: "database connection lost" })
And the server's error message is visible to the user
```

**Scenario**: Server returns 500 with non-JSON body

```
Given a queue_list request where /queues returns HTTP 500 with body "Gateway Timeout"
When queue_list processes the response
Then the body is read first
And TriggerErrorResponse parse fails (not JSON)
And the result is Err(CliError::HttpStatus { status: 500, reason: "Internal Server Error" })
```

### 8.7 Handler: queue_get Error Body Preserved

**Scenario**: Server returns 404 with structured error

```
Given a queue_get request for queue "nonexistent"
  And the server returns HTTP 404 with body {"error":"not_found","message":"queue 'nonexistent' does not exist"}
When queue_get processes the response
Then the body is read first
And TriggerErrorResponse parse succeeds
And the result is Err(CliError::ApiError { code: 404, message: "queue 'nonexistent' does not exist" })
```

**Scenario**: Server returns 404 with non-JSON body

```
Given a queue_get request where server returns HTTP 404 with body "Not Found"
When queue_get processes the response
Then TriggerErrorResponse parse fails
And the result is Err(CliError::NotFound("queue nonexistent not found"))
```

### 8.8 Handler: queue_delete Error Body Preserved

**Scenario**: Server returns 404 with structured error

```
Given a queue_delete request for queue "gone"
  And the server returns HTTP 404 with body {"error":"not_found","message":"queue 'gone' does not exist"}
When queue_delete processes the response
Then the body is read first
And the result is Err(CliError::ApiError { code: 404, message: "queue 'gone' does not exist" })
```

### 8.9 Handler: task_get Error Body Preserved

**Scenario**: Server returns 404 with structured error

```
Given a task_get request for task_id "abc-123"
  And the server returns HTTP 404 with body {"error":"not_found","message":"task 'abc-123' not found"}
When task_get processes the response
Then the body is read first
And the result is Err(CliError::ApiError { code: 404, message: "task 'abc-123' not found" })
```

### 8.10 Handler: task_log Error Body Preserved

**Scenario**: Server returns 404 with structured error

```
Given a task_log request for task_id "missing"
  And the server returns HTTP 404 with body {"error":"not_found","message":"task 'missing' not found"}
When task_log processes the response
Then the body is read first
And the result is Err(CliError::ApiError { code: 404, message: "task 'missing' not found" })
```

### 8.11 Handler: URL Encoding of Path Segments

**Scenario**: Queue name contains special characters

```
Given a queue_get request for queue name "my queue"
When the URL is constructed
Then the name is percent-encoded: "/queues/my%20queue"
And the server receives a valid URL (not a malformed path)

Given a task_get request for task_id "abc/def"
When the URL is constructed
Then the task_id is percent-encoded: "/tasks/abc%2Fdef"
```

**Scenario**: IDs with no special characters are unchanged

```
Given a queue_get request for name "normal-queue_1"
When the URL is constructed
Then the name is percent-encoded (no-op for alphanum + hyphen + underscore): "/queues/normal-queue_1"
```

### 8.12 Trigger Test: Negative HTTP Status 400

**Scenario**: trigger_create receives 400 Bad Request

```
Given a mock server that returns HTTP 400 with body {"error":"bad_request","message":"invalid JSON payload"}
When trigger_create is called with malformed JSON
Then the result is Err(CliError::ApiError { code: 400, message: "invalid JSON payload" })
```

### 8.13 Trigger Test: Negative HTTP Status 404

**Scenario**: trigger_get receives 404 Not Found

```
Given a mock server that returns HTTP 404 with body {"error":"not_found","message":"trigger 'nonexistent' not found"}
When trigger_get is called with id "nonexistent"
Then the result is Err(CliError::ApiError { code: 404, message: "trigger 'nonexistent' not found" })
```

### 8.14 Trigger Test: Negative HTTP Status 409

**Scenario**: trigger_update receives 409 Conflict

```
Given a mock server that returns HTTP 409 with body {"error":"conflict","message":"version mismatch"}
When trigger_update is called
Then the result is Err(CliError::ApiError { code: 409, message: "version mismatch" })
```

### 8.15 Trigger Test: Negative HTTP Status 500

**Scenario**: trigger_list receives 500 Internal Server Error

```
Given a mock server that returns HTTP 500 with body {"error":"internal","message":"database unavailable"}
When trigger_list is called
Then the result is Err(CliError::ApiError { code: 500, message: "database unavailable" })
```

### 8.16 Trigger Test: TriggerId Boundary Below Minimum

**Scenario**: 2-character trigger ID (below min=3)

```
Given a trigger_get request with id "ab" (2 chars, below TRIGGER_ID_MIN_LEN=3)
When the request reaches the server
Then the server returns HTTP 400
And the CLI returns Err(CliError::ApiError { code: 400, message: ... })
```

### 8.17 Trigger Test: TriggerId Boundary At Minimum

**Scenario**: 3-character trigger ID (at min=3)

```
Given a trigger_get request with id "abc" (3 chars, exactly TRIGGER_ID_MIN_LEN)
When the request reaches a server with trigger "abc" configured
Then the server returns HTTP 200 with the trigger data
And the CLI returns Ok(body)
```

### 8.18 Trigger Test: TriggerId Boundary Above Maximum

**Scenario**: 65-character trigger ID (above max=64)

```
Given a trigger_get request with id of 65 characters
When the request reaches the server
Then the server returns HTTP 400
And the CLI returns Err(CliError::ApiError { code: 400, message: ... })
```

### 8.19 Trigger Test: TriggerId Boundary At Maximum

**Scenario**: 64-character trigger ID (at max=64)

```
Given a trigger_get request with id of exactly 64 characters matching [a-zA-Z0-9_-]
When the request reaches a server with that trigger configured
Then the server returns HTTP 200
And the CLI returns Ok(body)
```

### 8.20 Trigger Test: TriggerId Charset Violation

**Scenario**: Trigger ID contains invalid characters

```
Given a trigger_get request with id "bad trigger!" (contains space and exclamation mark)
When the request reaches the server
Then the server returns HTTP 400
And the CLI returns Err(CliError::ApiError { code: 400, message: ... })
```

### 8.21 Trigger Test: Mutation Kill Verification

**Scenario**: Removing TriggerErrorResponse parse path causes test failure

```
Given existing trigger negative-status tests verify ApiError extraction from server responses
When the TriggerErrorResponse parse path is commented out or removed in trigger.rs
Then at least one test in the trigger test suite fails
  (proving tests catch the regression, not just pass trivially)
```

---

## 9. Implementation Order

1. **Test naming fixes** (banner.rs, cli.rs) — mechanical renames, low risk, establishes clean baseline
2. **Handler error-body drops** (queue.rs, task.rs) — follow trigger.rs pattern, add URL encoding
3. **Holzmann violations in test files** (bdd_behavior_report.rs, bdd_behavioral_contract_test.rs) — fix after handler fixes since some tests may need updating
4. **New trigger tests** (trigger_contract_regression_test.rs or new file) — add after handlers are fixed, tests the complete fixed behavior
5. **Verify** — `cargo test`, `cargo clippy`, `cargo build` pass cleanly

---

## 10. Dependency Changes

| Change | Rationale |
|--------|-----------|
| Add `percent-encoding` to `crates/twerk-cli/Cargo.toml` | URL-encode path segments in queue/task handlers. Already in workspace Cargo.lock via `reqwest → url → percent-encoding`. |
| Optionally add `serial_test` to `[dev-dependencies]` | Replace `LazyLock<Mutex>` for env-var test serialization (alternative: restructure tests to not share mutable env state) |
