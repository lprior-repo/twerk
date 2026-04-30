# Test Plan: twerk-0gr — Fix Broken twerk-cli Test Infra and Handler Defects

## Summary

- **Behaviors identified**: 78
- **Trophy allocation**: 56 unit / 17 integration / 5 e2e / 0 static (static = clippy/cargo-deny, counted separately)
- **Proptest invariants**: 4
- **Fuzz targets**: 2
- **Kani harnesses**: 1
- **Mutation kill target**: ≥90%
- **Dependency additions required**: `percent-encoding` (production), `serial_test` (dev)

---

## 1. Behavior Inventory

### D1 — Test Infra Holzmann Violations (10 behaviors)

| # | Behavior |
|---|----------|
| B1 | claim_14 expands loop into individual assertions per error variant when test body must not contain loops |
| B2 | LOGGING_ENV_LOCK is removed from bdd_behavior_report.rs when tests no longer share mutable LazyLock state |
| B3 | claim_17 asserts Err(CliError::Http(_)) specifically when health check fails to connect |
| B4 | claim_18 asserts Err(CliError::Http(_)) specifically for both trailing-slash and bare endpoint connection failures |
| B5 | claim_19 asserts Err(CliError::UnknownDatastore(_)) specifically when migration receives unknown datastore type |
| B6 | adversarial completeness_check asserts error variants are constructible without let _ = suppression |
| B7 | boundary_check asserts health_check result is examined without let _ = suppression |
| B8 | bdd_behavioral_contract_test completeness_check asserts variants constructible without let _ = suppression |
| B9 | banner.rs internal tests rename with test_ prefix when module is #[cfg(test)] |
| B10 | cli.rs internal tests rename with test_ prefix where missing when module is #[cfg(test)] |

### D2 — Missing Trigger Tests (20 behaviors)

| # | Behavior |
|---|----------|
| B11 | trigger_list returns Err(CliError::ApiError) with server message when server returns 500 with structured JSON |
| B12 | trigger_list returns Err(CliError::HttpStatus) when server returns 500 with non-JSON body |
| B13 | trigger_get returns Err(CliError::ApiError) with server message when server returns 404 with structured JSON |
| B14 | trigger_get returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B15 | trigger_get returns Err(CliError::ApiError) when server returns 400 with structured JSON |
| B16 | trigger_create returns Err(CliError::ApiError) when server returns 400 with structured JSON |
| B17 | trigger_update returns Err(CliError::ApiError) when server returns 409 with structured JSON |
| B18 | trigger_update returns Err(CliError::ApiError) when server returns 404 with structured JSON |
| B19 | trigger_delete returns Err(CliError::ApiError) when server returns 404 with structured JSON |
| B20 | trigger_delete returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B21 | TriggerId of 2 chars returns Err(CliError::ApiError) with code 400 when server rejects below-min-length |
| B22 | TriggerId of 3 chars returns Ok when server has matching trigger (min boundary) |
| B23 | TriggerId of 65 chars returns Err(CliError::ApiError) with code 400 when server rejects above-max-length |
| B24 | TriggerId of 64 chars returns Ok when server has matching trigger (max boundary) |
| B25 | TriggerId with special characters returns Err(CliError::ApiError) with code 400 when server rejects charset violation |
| B26 | trigger_get returns Err(CliError::HttpStatus) when server returns non-JSON non-404 error |
| B27 | trigger_create returns Err(CliError::HttpStatus) when server returns non-JSON non-400 error |
| B28 | trigger_update returns Err(CliError::HttpStatus) when server returns non-JSON error for unrecognized status |
| B29 | trigger_delete returns Err(CliError::HttpStatus) when server returns non-JSON non-404 non-204 error |
| B30 | TriggerErrorResponse parse path removal causes at least one test failure (mutation kill verification) |

### D3 — Handler Error-Body Drop Fixes (28 behaviors)

| # | Behavior |
|---|----------|
| B31 | queue_list returns Err(CliError::ApiError) with server message when server returns 500 with structured JSON |
| B32 | queue_list returns Err(CliError::HttpStatus) when server returns 500 with non-JSON body |
| B33 | queue_get returns Err(CliError::ApiError) with server message when server returns 404 with structured JSON |
| B34 | queue_get returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B35 | queue_get returns Err(CliError::ApiError) when server returns 500 with structured JSON |
| B36 | queue_get returns Err(CliError::HttpStatus) when server returns non-JSON non-404 error |
| B37 | queue_delete returns Err(CliError::ApiError) with server message when server returns 404 with structured JSON |
| B38 | queue_delete returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B39 | queue_delete returns Err(CliError::ApiError) when server returns 500 with structured JSON |
| B40 | queue_delete returns Err(CliError::HttpStatus) when server returns non-JSON non-404 error |
| B41 | task_get returns Err(CliError::ApiError) with server message when server returns 404 with structured JSON |
| B42 | task_get returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B43 | task_get returns Err(CliError::ApiError) when server returns 500 with structured JSON |
| B43.5 | task_get returns error when task_id is empty string |
| B44 | task_get returns Err(CliError::HttpStatus) when server returns non-JSON non-404 error |
| B45 | task_log returns Err(CliError::ApiError) with server message when server returns 404 with structured JSON |
| B46 | task_log returns Err(CliError::NotFound) when server returns 404 with non-JSON body |
| B47 | task_log returns Err(CliError::ApiError) when server returns 500 with structured JSON |
| B48 | task_log returns Err(CliError::HttpStatus) when server returns non-JSON non-404 error |
| B49 | queue_get URL-encodes name when name contains space character |
| B49.5 | queue_get integration test verifies URL encoding in actual HTTP request when name contains space |
| B50 | queue_get URL-encodes name when name contains slash character |
| B51 | queue_get URL does not alter name when name is alphanumeric with hyphens and underscores |
| B52 | queue_delete URL-encodes name when name contains special characters |
| B53 | task_get URL-encodes task_id when task_id contains slash |
| B54 | task_get URL does not alter task_id when task_id is alphanumeric |
| B55 | task_log URL-encodes task_id when task_id contains special characters |
| B55.5 | task_log includes page and size query params when both provided |
| B55.6 | task_log excludes query params when page and size are None |
| B55.7 | task_log includes only page when size is None |
| B55.8 | task_log includes only size when page is None |
| B55.9 | task_log handles zero page and negative size without validation |
| B56 | queue_list returns Ok with body when server returns 200 with valid JSON array |
| B57 | queue_get returns Ok with body when server returns 200 with valid QueueInfo JSON |
| B58 | queue_delete returns Ok with body when server returns 200 |
| B58.1 | queue_list returns Ok with empty array when server returns 200 with [] |
| B58.2 | task_get returns Ok with body when server returns 200 |
| B58.3 | task_log returns Ok with body when server returns 200 |

### Additional — CliError Variant Coverage (10 behaviors)

| # | Behavior |
|---|----------|
| B59 | CliError::HttpStatus display contains status code and reason |
| B60 | CliError::NotFound display contains resource description |
| B61 | CliError::ApiError display contains code and message |
| B62 | CliError::InvalidHostname display contains hostname value |
| B63 | CliError::InvalidEndpoint display contains endpoint value |
| B64 | CliError::Http kind returns ErrorKind::Runtime |
| B65 | CliError::Config kind returns ErrorKind::Validation |
| B66 | CliError::HttpStatus kind returns ErrorKind::Runtime |
| B67 | CliError::NotFound kind returns ErrorKind::Runtime |
| B68 | CliError::ApiError kind returns ErrorKind::Runtime |

---

## 2. Trophy Allocation

| Layer | Count | Percentage | Rationale |
|-------|-------|-----------|-----------|
| Unit | 56 | 72% | Pure Calc-layer: CliError display/kind (10), encode_path_segment (3), BannerMode parsing/renames (9), error variant construction/completeness (19), logging setup (2), boundary checks (4), Holzmann loop expansion (8), mutation kill source scan (1). High unit count because B6 requires 15 per-variant functions and B1 expands a loop into 8 individual assertions. |
| Integration | 17 | 22% | Handler functions against live test servers (axum TcpListener): claim_17/18/19 connection failures (3), URL encoding integration for queue_get/queue_delete/task_get/task_log (4), task_log page/size query params (5), handler happy paths including empty collection (6), trigger/queue/task error-body tests (2 — already counted in URL encoding). |
| E2E | 5 | 6% | CLI binary subprocess tests: 3 JSON-mode error output tests + 2 text-mode output tests (empty collection, non-empty table). All run in CI with embedded mock servers. |
| Static | 0 | 0% | clippy + cargo-deny + `deny(unwrap_used, expect_used, panic)` + `forbid(unsafe_code)` enforced by CI, not counted as separate test functions. |

**Dev-dependencies to add:**

```toml
[dev-dependencies]
serial_test = "3"          # Replace LazyLock<Mutex> for env-var test serialization
percent-encoding = "2"     # Already transitive via reqwest→url; add direct for test-only encode verification
```

**Production dependency to add:**

```toml
[dependencies]
percent-encoding = "2"     # For encode_path_segment in queue.rs and task.rs
```

---

## 3. BDD Scenarios

### D1 — Test Infra Holzmann Fixes

#### B1: claim_14 loop elimination

```
fn claim_14_config_error_display_contains_expected_substring()
fn claim_14_health_failed_display_contains_expected_substring()
fn claim_14_invalid_body_display_contains_expected_substring()
fn claim_14_missing_argument_display_contains_expected_substring()
fn claim_14_migration_error_display_contains_expected_substring()
fn claim_14_unknown_datastore_display_contains_expected_substring()
fn claim_14_logging_error_display_contains_expected_substring()
fn claim_14_engine_error_display_contains_expected_substring()
```

Given: a CliError variant constructed with a known payload
When: .to_string() is called
Then: the display string contains the expected substring
And: no loop construct exists in any test function body

#### B2: LOGGING_ENV_LOCK removal

```
fn claim_3_setup_logging_accepts_valid_level()
fn claim_4_setup_logging_rejects_invalid_level()
```

Given: TWERK_LOGGING_LEVEL env var is set via LoggingEnvGuard RAII
When: setup_logging() is called
Then: valid level "debug" returns Ok(())
And: invalid level "invalid_level_xyz" returns Err(CliError::Logging(msg)) where msg contains "invalid_level_xyz"
And: no LazyLock<Mutex<()>> exists in the test file
And: tests that mutate env vars are annotated with `#[serial_test::serial]`

#### B3: claim_17 specific error variant

```
fn claim_17_health_check_returns_http_error_on_connection_failure()
```

Given: health_check is called with unreachable endpoint "http://localhost:99999"
When: the connection fails
Then: result matches Err(CliError::Http(_))
And: no `is_err()` bare assertion exists

#### B4: claim_18 specific error variant for both endpoints

```
fn claim_18_health_check_trailing_slash_returns_http_error()
fn claim_18_health_check_bare_endpoint_returns_http_error()
```

Given: health_check is called with "http://localhost:99999/" or "http://localhost:99999"
When: the connection fails
Then: result matches Err(CliError::Http(_))

#### B5: claim_19 specific error variant

```
fn claim_19_migration_returns_unknown_datastore_error_for_mysql()
```

Given: run_migration is called with "mysql" datastore type
When: the migration rejects the unknown type
Then: result matches Err(CliError::UnknownDatastore(msg)) where msg contains "mysql"

#### B6: adversarial completeness_check — per-variant assertions (no loop, no let _ =)

```
fn completeness_check_config_variant_is_constructible_and_displayable()
fn completeness_check_http_variant_is_constructible_and_displayable()
fn completeness_check_http_status_variant_is_constructible_and_displayable()
fn completeness_check_health_failed_variant_is_constructible_and_displayable()
fn completeness_check_invalid_body_variant_is_constructible_and_displayable()
fn completeness_check_missing_argument_variant_is_constructible_and_displayable()
fn completeness_check_migration_variant_is_constructible_and_displayable()
fn completeness_check_unknown_datastore_variant_is_constructible_and_displayable()
fn completeness_check_logging_variant_is_constructible_and_displayable()
fn completeness_check_engine_variant_is_constructible_and_displayable()
fn completeness_check_invalid_hostname_variant_is_constructible_and_displayable()
fn completeness_check_invalid_endpoint_variant_is_constructible_and_displayable()
fn completeness_check_not_found_variant_is_constructible_and_displayable()
fn completeness_check_api_error_variant_is_constructible_and_displayable()
fn completeness_check_io_variant_is_constructible_and_displayable()
```

Given: one specific CliError variant
When: it is constructed with minimal valid arguments
Then: `format!("{:?}", variant)` is non-empty
And: `variant.kind()` returns the expected ErrorKind
And: no `let _ =` suppression exists in the function body

**Construction strategy for `CliError::Http`**: Cannot be directly constructed because `reqwest::Error` has no public constructor. Instead, trigger a real connection failure: `let err = reqwest::get("http://unreachable-host.invalid/").await.unwrap_err(); let cli_err = CliError::from(err);` then assert on the variant match and display output.

#### B7: boundary_check without let _ =

```
fn boundary_check_very_long_endpoint_string_returns_error()
```

Given: health_check with extremely long port number
When: connection fails
Then: result matches Err(CliError::Http(_))
And: no `let _ =` suppression exists

#### B8: bdd_behavioral_contract_test completeness_check without let _ =

```
fn then_error_enum_variants_are_constructible_and_debug_is_non_empty()
fn then_commands_enum_variants_are_constructible_and_debug_is_non_empty()
fn then_all_public_constants_are_accessible_and_non_empty()
```

Given: all error/command variants and public constants
When: each is constructed and formatted via Debug/Display
Then: debug string is non-empty for each
And: no `let _ =` suppression exists

#### B9: banner.rs test_ prefix

```
fn test_banner_mode_from_str_returns_expected_variants()
fn test_banner_mode_from_str_case_insensitive()
fn test_banner_mode_from_str_whitespace_defaults_to_console()
fn test_banner_mode_default_is_console()
fn test_banner_constant_not_empty_with_ascii_art()
fn test_banner_constant_contains_branding()
fn test_banner_mode_equality()
fn test_banner_mode_copy_semantics()
fn test_banner_mode_clone_semantics()
```

Given: banner.rs #[cfg(test)] module has 8–9 test functions
When: cargo test discovers tests
Then: all function names start with `test_`
And: all tests pass

#### B10: cli.rs test_ prefix normalization

All test functions in cli.rs `#[cfg(test)] mod tests` that lack `test_` prefix are renamed:
```
test_default_endpoint_is_localhost_http
test_default_datastore_type_is_postgres
test_default_postgres_dsn_contains_localhost
test_version_constant_is_not_empty
test_git_commit_constant_is_not_empty
test_get_git_commit_returns_non_empty_string
test_constants_are_accessible_without_mutation
test_parse_cli_args_returns_execute_none_when_subcommand_missing
test_parse_cli_args_returns_display_version_error_when_version_flag_present
test_parse_cli_args_returns_version_subcommand
test_version_subcommand_skips_startup_ui_in_text_mode
test_health_command_emits_startup_ui_in_text_mode
test_json_mode_skips_startup_ui_for_all_commands
test_parse_cli_args_returns_run_command_for_coordinator_mode
test_parse_cli_args_enables_json_mode_for_health_command
test_get_endpoint_reads_client_endpoint_from_environment_override
test_render_top_level_help_contains_usage
```

---

### D2 — Missing Trigger Tests

#### B11: trigger_list 500 with structured JSON

```
#[tokio::test]
async fn trigger_list_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server returning HTTP 500 with body `{"error":"internal","message":"database unavailable"}`
When: trigger_list is called
Then: result matches Err(CliError::ApiError { code: 500, message }) where message == "database unavailable"

#### B12: trigger_list 500 with non-JSON

```
#[tokio::test]
async fn trigger_list_returns_http_status_when_server_returns_500_with_non_json_body()
```

Given: a mock server returning HTTP 500 with body `"Gateway Timeout"` (plain text)
When: trigger_list is called
Then: result matches Err(CliError::HttpStatus { status: 500, reason: "Internal Server Error" })

#### B13: trigger_get 404 with structured JSON

```
#[tokio::test]
async fn trigger_get_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server returning HTTP 404 with body `{"error":"not_found","message":"trigger 'nonexistent' not found"}`
When: trigger_get is called with id "nonexistent"
Then: result matches Err(CliError::ApiError { code: 404, message }) where message == "trigger 'nonexistent' not found"

#### B14: trigger_get 404 with non-JSON

```
#[tokio::test]
async fn trigger_get_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server returning HTTP 404 with body `"Not Found"` (plain text)
When: trigger_get is called with id "nonexistent"
Then: result matches Err(CliError::NotFound(msg)) where msg == "trigger nonexistent not found"

#### B15: trigger_get 400 with structured JSON

```
#[tokio::test]
async fn trigger_get_returns_api_error_when_server_returns_400_with_structured_json()
```

Given: a mock server returning HTTP 400 with body `{"error":"bad_request","message":"invalid trigger ID format"}`
When: trigger_get is called with id "ab"
Then: result matches Err(CliError::ApiError { code: 400, message }) where message == "invalid trigger ID format"

#### B16: trigger_create 400 with structured JSON

```
#[tokio::test]
async fn trigger_create_returns_api_error_when_server_returns_400_with_structured_json()
```

Given: a mock server returning HTTP 400 with body `{"error":"bad_request","message":"invalid JSON payload"}`
When: trigger_create is called with malformed JSON body
Then: result matches Err(CliError::ApiError { code: 400, message }) where message == "invalid JSON payload"

#### B17: trigger_update 409 with structured JSON

```
#[tokio::test]
async fn trigger_update_returns_api_error_when_server_returns_409_with_structured_json()
```

Given: a mock server returning HTTP 409 with body `{"error":"conflict","message":"version mismatch"}`
When: trigger_update is called
Then: result matches Err(CliError::ApiError { code: 409, message }) where message == "version mismatch"

#### B18: trigger_update 404 with structured JSON

```
#[tokio::test]
async fn trigger_update_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server returning HTTP 404 with body `{"error":"not_found","message":"trigger 'gone' not found"}`
When: trigger_update is called with id "gone"
Then: result matches Err(CliError::ApiError { code: 404, message }) where message == "trigger 'gone' not found"

#### B19: trigger_delete 404 with structured JSON

```
#[tokio::test]
async fn trigger_delete_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server returning HTTP 404 with body `{"error":"not_found","message":"trigger 'gone' not found"}`
When: trigger_delete is called with id "gone"
Then: result matches Err(CliError::ApiError { code: 404, message: "trigger 'gone' not found" })

#### B20: trigger_delete 404 with non-JSON

```
#[tokio::test]
async fn trigger_delete_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server returning HTTP 404 with body `"Not Found"` (plain text)
When: trigger_delete is called with id "gone"
Then: result matches Err(CliError::NotFound(msg)) where msg == "trigger gone not found"

#### B21: TriggerId 2 chars (below min)

```
#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_has_2_chars_below_minimum()
```

Given: a mock server returning HTTP 400 with body `{"error":"validation","message":"trigger id must be at least 3 characters"}`
When: trigger_get is called with id "ab" (2 chars)
Then: result matches Err(CliError::ApiError { code: 400, message: "trigger id must be at least 3 characters" })

#### B22: TriggerId 3 chars (at min)

```
#[tokio::test]
async fn trigger_get_succeeds_when_trigger_id_has_3_chars_at_minimum_boundary()
```

Given: a server with trigger "abc" configured
When: trigger_get is called with id "abc" (3 chars)
Then: result matches Ok(body) where body contains "abc"

#### B23: TriggerId 65 chars (above max)

```
#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_has_65_chars_above_maximum()
```

Given: a mock server returning HTTP 400 with body `{"error":"validation","message":"trigger id must be at most 64 characters"}`
When: trigger_get is called with 65-char id
Then: result matches Err(CliError::ApiError { code: 400, message: "trigger id must be at most 64 characters" })

#### B24: TriggerId 64 chars (at max)

```
#[tokio::test]
async fn trigger_get_succeeds_when_trigger_id_has_64_chars_at_maximum_boundary()
```

Given: a server with a 64-char trigger id configured
When: trigger_get is called with 64-char id matching `[a-zA-Z0-9_-]`
Then: result matches Ok(body) where body contains the 64-char trigger id

#### B25: TriggerId charset violation

```
#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_contains_special_characters()
```

Given: a mock server returning HTTP 400 with body `{"error":"validation","message":"trigger id contains invalid characters"}`
When: trigger_get is called with id "bad trigger!" (contains space and !)
Then: result matches Err(CliError::ApiError { code: 400, message: "trigger id contains invalid characters" })

#### B25.5: TriggerId 0 chars (empty string)

```
#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_is_empty_string()
```

Given: a mock server returning HTTP 400 with body `{"error":"validation","message":"trigger id must be at least 3 characters"}`
When: trigger_get is called with id "" (0 chars)
Then: result matches Err(CliError::ApiError { code: 400, message: "trigger id must be at least 3 characters" })

#### B26: trigger_get non-JSON non-404 error

```
#[tokio::test]
async fn trigger_get_returns_http_status_when_server_returns_non_json_error()
```

Given: a mock server returning HTTP 503 with body `"Service Unavailable"` (plain text)
When: trigger_get is called
Then: result matches Err(CliError::HttpStatus { status: 503, reason: "Service Unavailable" })

#### B27: trigger_create non-JSON non-400 error

```
#[tokio::test]
async fn trigger_create_returns_http_status_when_server_returns_non_json_error()
```

Given: a mock server returning HTTP 500 with body `"Internal Server Error"` (plain text)
When: trigger_create is called
Then: result matches Err(CliError::HttpStatus { status: 500, reason: "Internal Server Error" })

#### B28: trigger_update non-JSON unrecognized status

```
#[tokio::test]
async fn trigger_update_returns_http_status_when_server_returns_unrecognized_status()
```

Given: a mock server returning HTTP 418 with body `"I'm a teapot"` (plain text)
When: trigger_update is called
Then: result matches Err(CliError::HttpStatus { status: 418, reason: "I'm a teapot" })

#### B29: trigger_delete non-JSON non-404 non-204 error

```
#[tokio::test]
async fn trigger_delete_returns_http_status_when_server_returns_unrecognized_status()
```

Given: a mock server returning HTTP 502 with body `"Bad Gateway"` (plain text)
When: trigger_delete is called
Then: result matches Err(CliError::HttpStatus { status: 502, reason: "Bad Gateway" })

#### B30: Mutation kill — TriggerErrorResponse parse removal

```
#[test]
fn mutation_kill_trigger_error_response_parse_path_verified()
```

Given: the source file `src/handlers/trigger.rs` is read via `include_str!`
When: the source content is searched for the parse pattern
Then: assert!(source.contains("from_str::<TriggerErrorResponse>")) — verifies the parse path exists in source
And: this test documents that if the parse path is removed, B11–B20 would fail with `HttpStatus` instead of `ApiError`

---

### D3 — Handler Error-Body Drop Fixes

#### B31: queue_list 500 structured JSON

```
#[tokio::test]
async fn queue_list_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server where GET /queues returns HTTP 500 with body `{"error":"internal","message":"database connection lost"}`
When: queue_list is called
Then: result matches Err(CliError::ApiError { code: 500, message }) where message == "database connection lost"

#### B32: queue_list 500 non-JSON

```
#[tokio::test]
async fn queue_list_returns_http_status_when_server_returns_500_with_non_json_body()
```

Given: a mock server where GET /queues returns HTTP 500 with body `"Gateway Timeout"`
When: queue_list is called
Then: result matches Err(CliError::HttpStatus { status: 500, reason: "Internal Server Error" })

#### B33: queue_get 404 structured JSON

```
#[tokio::test]
async fn queue_get_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server where GET /queues/{name} returns HTTP 404 with body `{"error":"not_found","message":"queue 'nonexistent' does not exist"}`
When: queue_get is called with name "nonexistent"
Then: result matches Err(CliError::ApiError { code: 404, message }) where message == "queue 'nonexistent' does not exist"

#### B34: queue_get 404 non-JSON

```
#[tokio::test]
async fn queue_get_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server where GET /queues/{name} returns HTTP 404 with body `"Not Found"`
When: queue_get is called with name "nonexistent"
Then: result matches Err(CliError::NotFound(msg)) where msg == "queue nonexistent not found"

#### B35: queue_get 500 structured JSON

```
#[tokio::test]
async fn queue_get_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server where GET /queues/{name} returns HTTP 500 with body `{"error":"internal","message":"server error"}`
When: queue_get is called with name "broken"
Then: result matches Err(CliError::ApiError { code: 500, message: "server error" })

#### B35.5: queue_get empty name boundary

```
#[tokio::test]
async fn queue_get_returns_error_when_name_is_empty_string()
```

Given: a mock server returning HTTP 404 with plain text body `"Not Found"` (non-JSON)
When: queue_get is called with name "" (empty string), URL becomes `/queues/`
Then: result matches Err(CliError::NotFound(msg)) where msg == "queue  not found"

#### B36: queue_get non-JSON non-404 error

```
#[tokio::test]
async fn queue_get_returns_http_status_when_server_returns_non_json_non_404_error()
```

Given: a mock server where GET /queues/{name} returns HTTP 503 with body `"Service Unavailable"`
When: queue_get is called with name "broken"
Then: result matches Err(CliError::HttpStatus { status: 503, reason: "Service Unavailable" })

#### B37: queue_delete 404 structured JSON

```
#[tokio::test]
async fn queue_delete_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server where DELETE /queues/{name} returns HTTP 404 with body `{"error":"not_found","message":"queue 'gone' does not exist"}`
When: queue_delete is called with name "gone"
Then: result matches Err(CliError::ApiError { code: 404, message: "queue 'gone' does not exist" })

#### B38: queue_delete 404 non-JSON

```
#[tokio::test]
async fn queue_delete_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server where DELETE /queues/{name} returns HTTP 404 with body `"Not Found"`
When: queue_delete is called with name "gone"
Then: result matches Err(CliError::NotFound(msg)) where msg == "queue gone not found"

#### B39: queue_delete 500 structured JSON

```
#[tokio::test]
async fn queue_delete_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server where DELETE /queues/{name} returns HTTP 500 with body `{"error":"internal","message":"server error"}`
When: queue_delete is called with name "broken"
Then: result matches Err(CliError::ApiError { code: 500, message: "server error" })

#### B39.5: queue_delete empty name boundary

```
#[tokio::test]
async fn queue_delete_returns_error_when_name_is_empty_string()
```

Given: a mock server returning HTTP 404 with plain text body `"Not Found"` (non-JSON)
When: queue_delete is called with name "" (empty string), URL becomes `/queues/`
Then: result matches Err(CliError::NotFound(msg)) where msg == "queue  not found"

#### B40: queue_delete non-JSON non-404 error

```
#[tokio::test]
async fn queue_delete_returns_http_status_when_server_returns_non_json_non_404_error()
```

Given: a mock server where DELETE /queues/{name} returns HTTP 502 with body `"Bad Gateway"`
When: queue_delete is called with name "broken"
Then: result matches Err(CliError::HttpStatus { status: 502, reason: "Bad Gateway" })

#### B41: task_get 404 structured JSON

```
#[tokio::test]
async fn task_get_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server where GET /tasks/{task_id} returns HTTP 404 with body `{"error":"not_found","message":"task 'abc-123' not found"}`
When: task_get is called with task_id "abc-123"
Then: result matches Err(CliError::ApiError { code: 404, message: "task 'abc-123' not found" })

#### B42: task_get 404 non-JSON

```
#[tokio::test]
async fn task_get_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server where GET /tasks/{task_id} returns HTTP 404 with body `"Not Found"`
When: task_get is called with task_id "missing"
Then: result matches Err(CliError::NotFound(msg)) where msg == "task missing not found"

#### B43: task_get 500 structured JSON

```
#[tokio::test]
async fn task_get_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server where GET /tasks/{task_id} returns HTTP 500 with body `{"error":"internal","message":"database connection lost"}`
When: task_get is called with task_id "broken"
Then: result matches Err(CliError::ApiError { code: 500, message: "database connection lost" })

#### B43.5: task_get with empty task_id

```
#[tokio::test]
async fn task_get_returns_error_when_task_id_is_empty_string()
```

Given: a mock server where GET /tasks// returns HTTP 404 with body `"Not Found"`
When: task_get is called with task_id "" (empty string)
Then: result matches Err(CliError::NotFound(msg)) where msg == "task  not found" (empty id in format string)

#### B44: task_get non-JSON non-404 error

```
#[tokio::test]
async fn task_get_returns_http_status_when_server_returns_non_json_non_404_error()
```

Given: a mock server where GET /tasks/{task_id} returns HTTP 503 with body `"Service Unavailable"`
When: task_get is called with task_id "stuck"
Then: result matches Err(CliError::HttpStatus { status: 503, reason: "Service Unavailable" })

#### B45: task_log 404 structured JSON

```
#[tokio::test]
async fn task_log_returns_api_error_when_server_returns_404_with_structured_json()
```

Given: a mock server where GET /tasks/{task_id}/log returns HTTP 404 with body `{"error":"not_found","message":"task 'gone' not found"}`
When: task_log is called with task_id "gone"
Then: result matches Err(CliError::ApiError { code: 404, message: "task 'gone' not found" })

#### B46: task_log 404 non-JSON

```
#[tokio::test]
async fn task_log_returns_not_found_when_server_returns_404_with_non_json_body()
```

Given: a mock server where GET /tasks/{task_id}/log returns HTTP 404 with body `"Not Found"`
When: task_log is called with task_id "missing"
Then: result matches Err(CliError::NotFound(msg)) where msg == "task missing not found"

#### B47: task_log 500 structured JSON

```
#[tokio::test]
async fn task_log_returns_api_error_when_server_returns_500_with_structured_json()
```

Given: a mock server where GET /tasks/{task_id}/log returns HTTP 500 with body `{"error":"internal","message":"server error"}`
When: task_log is called with task_id "broken"
Then: result matches Err(CliError::ApiError { code: 500, message: "server error" })

#### B47.5: task_log with empty task_id

```
#[tokio::test]
async fn task_log_returns_error_when_task_id_is_empty_string()
```

Given: a mock server where GET /tasks//log returns HTTP 404 with body `"Not Found"`
When: task_log is called with task_id "" (empty string)
Then: result matches Err(CliError::NotFound(msg)) where msg == "task  not found" (empty id in format string)

#### B48: task_log non-JSON non-404 error

```
#[tokio::test]
async fn task_log_returns_http_status_when_server_returns_non_json_non_404_error()
```

Given: a mock server where GET /tasks/{task_id}/log returns HTTP 502 with body `"Bad Gateway"`
When: task_log is called with task_id "broken"
Then: result matches Err(CliError::HttpStatus { status: 502, reason: "Bad Gateway" })

#### B49–B55: URL encoding

```
#[test]
fn encode_path_segment_encodes_space_as_percent_20()
```

Given: segment = "my queue"
When: encode_path_segment is called
Then: result == "my%20queue"

```
#[test]
fn encode_path_segment_encodes_slash_as_percent_2f()
```

Given: segment = "abc/def"
When: encode_path_segment is called
Then: result == "abc%2Fdef"

```
#[test]
fn encode_path_segment_preserves_alphanumeric_hyphen_underscore()
```

Given: segment = "normal-queue_1"
When: encode_path_segment is called
Then: result == "normal-queue_1"

```
#[tokio::test]
async fn queue_get_url_encodes_name_with_space_character()
```

Given: a mock server capturing the request URI, returning HTTP 200 with valid QueueInfo JSON
When: queue_get is called with name "my queue"
Then: the request URI path segment contains "my%20queue"

```
#[tokio::test]
async fn queue_delete_url_encodes_name_with_special_characters()
```

Given: a mock server capturing the request URI
When: queue_delete is called with name "my queue"
Then: the request URI path segment contains "my%20queue"

```
#[tokio::test]
async fn task_get_url_encodes_task_id_with_slash()
```

Given: a mock server capturing the request URI
When: task_get is called with task_id "abc/def"
Then: the request URI path segment contains "abc%2Fdef"

```
#[tokio::test]
async fn task_log_url_encodes_task_id_with_special_characters()
```

Given: a mock server capturing the request URI
When: task_log is called with task_id "has space"
Then: the request URI path segment contains "has%20space"

#### B55.5–B55.9: task_log page/size query parameter construction

```
#[tokio::test]
async fn task_log_includes_page_and_size_query_params_when_both_provided()
```

Given: a mock server capturing the request URI, returning HTTP 200 with `{"lines":["line1"]}`
When: task_log is called with task_id "t1", page=Some(5), size=Some(10)
Then: the request URI contains `?page=5&size=10`

```
#[tokio::test]
async fn task_log_excludes_query_params_when_page_and_size_are_none()
```

Given: a mock server capturing the request URI, returning HTTP 200
When: task_log is called with task_id "t1", page=None, size=None
Then: the request URI contains no `?` character

```
#[tokio::test]
async fn task_log_includes_only_page_when_size_is_none()
```

Given: a mock server capturing the request URI, returning HTTP 200
When: task_log is called with task_id "t1", page=Some(2), size=None
Then: the request URI contains `?page=2` and does not contain `size`

```
#[tokio::test]
async fn task_log_includes_only_size_when_page_is_none()
```

Given: a mock server capturing the request URI, returning HTTP 200
When: task_log is called with task_id "t1", page=None, size=Some(50)
Then: the request URI contains `?size=50` and does not contain `page`

```
#[tokio::test]
async fn task_log_handles_zero_page_and_negative_size()
```

Given: a mock server capturing the request URI, returning HTTP 200
When: task_log is called with task_id "t1", page=Some(0), size=Some(-1)
Then: the request URI contains `?page=0&size=-1` (passes through without validation)

```
#[tokio::test]
async fn queue_list_returns_ok_with_body_when_server_returns_200()
```

Given: a mock server where GET /queues returns HTTP 200 with valid JSON array `[{"name":"q1","size":0,"subscribers":0,"unacked":0}]`
When: queue_list is called
Then: result matches Ok(body) where body contains "q1"

```
#[tokio::test]
async fn queue_list_returns_ok_with_empty_array_when_server_returns_200_empty()
```

Given: a mock server where GET /queues returns HTTP 200 with `[]`
When: queue_list is called
Then: result matches Ok(body) where body == "[]"

```
#[tokio::test]
async fn queue_get_returns_ok_with_body_when_server_returns_200()
```

Given: a mock server where GET /queues/q1 returns HTTP 200 with valid QueueInfo JSON
When: queue_get is called with name "q1"
Then: result matches Ok(body) where body contains "q1"

```
#[tokio::test]
async fn queue_delete_returns_ok_with_body_when_server_returns_200()
```

Given: a mock server where DELETE /queues/q1 returns HTTP 200 with body `{"deleted":true,"name":"q1"}`
When: queue_delete is called with name "q1"
Then: result matches Ok(body) where body contains "q1"

```
#[tokio::test]
async fn task_get_returns_ok_with_body_when_server_returns_200()
```

Given: a mock server where GET /tasks/t1 returns HTTP 200 with valid task JSON `{"id":"t1","status":"completed","result":"ok"}`
When: task_get is called with task_id "t1"
Then: result matches Ok(body) where body contains "t1"

```
#[tokio::test]
async fn task_log_returns_ok_with_body_when_server_returns_200()
```

Given: a mock server where GET /tasks/t1/log returns HTTP 200 with body `{"lines":["task started","task completed"]}`
When: task_log is called with task_id "t1"
Then: result matches Ok(body) where body contains "task started"

---

### D3+ — CliError Variant Coverage

#### B59–B68: Error display and kind mapping

```
fn test_cli_error_http_status_display_contains_code_and_reason()
fn test_cli_error_not_found_display_contains_resource()
fn test_cli_error_api_error_display_contains_code_and_message()
fn test_cli_error_invalid_hostname_display_contains_hostname()
fn test_cli_error_invalid_endpoint_display_contains_endpoint()
fn test_cli_error_http_kind_is_runtime()
fn test_cli_error_config_kind_is_validation()
fn test_cli_error_http_status_kind_is_runtime()
fn test_cli_error_not_found_kind_is_runtime()
fn test_cli_error_api_error_kind_is_runtime()
```

These ensure every CliError variant has at least one test asserting its exact display format and kind mapping. Currently missing from existing tests: `HttpStatus`, `NotFound`, `ApiError`, `InvalidHostname`, `InvalidEndpoint` display verification.

---

## 4. Proptest Invariants

### Proptest: encode_path_segment

```
Invariant: For any non-empty UTF-8 string input, encode_path_segment output is a valid percent-encoded string. Decoding it yields the original input.
Strategy: any non-empty String (proptest::string::string_regex(".*").unwrap())
Anti-invariant: empty string input returns empty string (trivially valid)
```

### Proptest: CliError kind/exit_code consistency

```
Invariant: For any CliError variant, kind() returns Validation iff exit_code() == 2, Runtime iff exit_code() == 1.
Strategy: construct each of the 15 variants with arbitrary string payloads
Anti-invariant: no variant should return exit_code 0 (success) or any value other than 1 or 2
```

### Proptest: TriggerErrorResponse deserialization round-trip

```
Invariant: For any valid JSON with string "error" and "message" fields, deserializing then re-serializing preserves the field values exactly.
Strategy: arbitrary (String, String) pairs for (error, message), with optional (String, String) for (path_id, body_id)
Anti-invariant: JSON missing "error" or "message" field fails to deserialize
```

### Proptest: parse_api_error branching logic

```
Invariant: For any (status: u16, body: &str):
  - If body is valid TriggerErrorResponse JSON → parse_api_error returns Some(ApiError { code: status, message: parsed.message })
  - If body is NOT valid TriggerErrorResponse JSON → parse_api_error returns None
Strategy: (any u16, valid TriggerErrorResponse JSON) vs (any u16, arbitrary non-JSON string)
Anti-invariant: empty body string returns None; body with only `{"error":"x"}` (missing "message") returns None

---

## 5. Fuzz Targets

### Fuzz Target: serde_json::from_str for TriggerErrorResponse

```
Input type: arbitrary bytes (&[u8]) interpreted as UTF-8 string
Risk: panic on malformed UTF-8, logic error in field extraction, DoS via deeply nested JSON
Corpus seeds:
  - {"error":"e","message":"m"}
  - {"error":"","message":""}
  - {"error":"x","message":"y","path_id":"z","body_id":"w"}
  - {"error":"x","message":"y","path_id":"z"}
  - [] (empty array)
  - {} (empty object)
  - {"error":123,"message":null} (wrong types)
```

### Fuzz Target: encode_path_segment

```
Input type: arbitrary &str
Risk: panic on edge-case Unicode, assertion failure on round-trip, percent-encoding producing invalid URL
Corpus seeds:
  - "" (empty)
  - "abc" (simple ASCII)
  - "hello world" (space)
  - "/?:#@!$&'()*+,;=" (URL-special chars)
  - "%00" (null byte as literal)
  - "\u{1F600}" (emoji)
  - "a\u{0000}b" (embedded null)
```

---

## 6. Kani Harnesses

### Kani Harness: CliError exit_code exhaustiveness

```
Property: For all possible CliError variants, exit_code() returns exactly 1 or 2, and kind() matches the documented ErrorKind.
Bound: All 15 enum variants (exhaustive)
Rationale: The match in kind() must be exhaustive. Adding a new variant without updating kind() would silently default to a wrong exit code. Kani can prove compile-time exhaustiveness of the match.
```

```rust
// Example harness sketch:
#[kani::proof]
fn verify_all_variants_have_correct_kind() {
    let variants: Vec<CliError> = vec![
        CliError::Config(kani::any()),
        CliError::HealthFailed { status: kani::any() },
        // ... all 15 variants
    ];
    for err in &variants {
        let code = err.exit_code();
        assert!(code == 1 || code == 2);
        match err.kind() {
            ErrorKind::Validation => assert_eq!(code, 2),
            ErrorKind::Runtime => assert_eq!(code, 1),
        }
    }
}
```

---

## 7. Mutation Checkpoints

**Threshold: ≥90% mutation kill rate**

### Critical mutations that MUST be caught:

| Mutation Target | Catching Test(s) | Rationale |
|----------------|------------------|-----------|
| Remove `TriggerErrorResponse` parse in `trigger_list` | B11 (trigger_list 500 structured) | Returns `HttpStatus` instead of `ApiError` |
| Remove `TriggerErrorResponse` parse in `trigger_get` | B13, B15 (trigger_get 404/400 structured) | Falls through to `NotFound`/`HttpStatus` |
| Remove `TriggerErrorResponse` parse in `trigger_create` | B16 (trigger_create 400 structured) | Falls through to `HttpStatus` |
| Remove `TriggerErrorResponse` parse in `trigger_update` | B17, B18 (trigger_update 409/404 structured) | Falls through to `HttpStatus` |
| Remove `TriggerErrorResponse` parse in `trigger_delete` | B19 (trigger_delete 404 structured) | Falls through to `NotFound` |
| Remove body `.text().await` call before status check in `queue_list` | B31, B32 | Returns `HttpStatus` without reading body |
| Remove body `.text().await` call in `queue_get` | B33, B34 | Returns `NotFound` without reading body |
| Remove body `.text().await` call in `queue_delete` | B37, B38 | Returns `NotFound` without reading body |
| Remove body `.text().await` call in `task_get` | B41, B42 | Returns `NotFound` without reading body |
| Remove body `.text().await` call in `task_log` | B45, B46 | Returns `NotFound` without reading body |
| Remove `encode_path_segment` call in `queue_get` | B49.5 (integration: queue_get URL encoding) | URL contains raw space |
| Remove `encode_path_segment` call in `task_get` | B53 (slash encoding) | URL contains raw slash |
| Swap `ErrorKind::Validation` ↔ `ErrorKind::Runtime` in `kind()` | B64–B68 (kind tests) | Wrong exit code |
| Change `status.as_u16()` to hardcoded `0` in `HttpStatus` return | B31, B32 (status code assertions) | Wrong status in error |
| Remove `err_resp.message` field in `ApiError` return | B31, B33, B41 (message assertions) | Wrong message in error |
| Remove `serde_json::from_str` call in `parse_api_error` | All B31–B48 structured JSON tests | `ApiError` never returned |

### Mutation kill verification test (B30):

The dedicated mutation kill test `mutation_kill_trigger_error_response_parse_path_verified` documents that if the `TriggerErrorResponse` parse path is removed from any trigger handler, at least one test from the B11–B20 set will fail by asserting `CliError::ApiError` but receiving `CliError::HttpStatus` or `CliError::NotFound`.

---

## 8. Combinatorial Coverage Matrix

### Unit Tests: encode_path_segment

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| alphanumeric only | "abc-123_XYZ" | "abc-123_XYZ" (unchanged) | unit |
| contains space | "my queue" | "my%20queue" | unit |
| contains slash | "abc/def" | "abc%2Fdef" | unit |
| contains percent | "100%" | "100%25" | unit |
| contains hash | "a#b" | "a%23b" | unit |
| contains question mark | "a?b" | "a%3Fb" | unit |
| empty string | "" | "" | unit |
| single special char | "!" | "%21" | unit |

### Unit Tests: CliError display

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| Config | "missing key" | contains "configuration error" and "missing key" | unit |
| Http | reqwest error | contains "HTTP request failed" | unit |
| HttpStatus | status=500, reason="Internal Server Error" | contains "HTTP error 500" and "Internal Server Error" | unit |
| HealthFailed | status=503 | contains "health check failed" and "503" | unit |
| InvalidBody | "not json" | contains "invalid response body" and "not json" | unit |
| MissingArgument | "mode" | contains "missing required argument" and "mode" | unit |
| Migration | "connection refused" | contains "migration error" and "connection refused" | unit |
| UnknownDatastore | "mysql" | contains "unsupported datastore type" and "mysql" | unit |
| Logging | "invalid level" | contains "logging setup error" and "invalid level" | unit |
| Engine | "failed to start" | contains "engine error" and "failed to start" | unit |
| InvalidHostname | "!!!bad" | contains "invalid hostname" and "!!!bad" | unit |
| InvalidEndpoint | "not a url" | contains "invalid endpoint" and "not a url" | unit |
| NotFound | "resource xyz" | contains "not found" and "resource xyz" | unit |
| ApiError | code=400, message="bad input" | contains "API error 400" and "bad input" | unit |
| Io | io ErrorKind::NotFound | contains "IO error" | unit |

### Unit Tests: CliError kind/exit_code

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| Config | any string | kind=Validation, exit_code=2 | unit |
| Http | any reqwest error | kind=Runtime, exit_code=1 | unit |
| HttpStatus | any status/reason | kind=Runtime, exit_code=1 | unit |
| HealthFailed | any status | kind=Runtime, exit_code=1 | unit |
| InvalidBody | any string | kind=Runtime, exit_code=1 | unit |
| MissingArgument | any string | kind=Validation, exit_code=2 | unit |
| Migration | any string | kind=Runtime, exit_code=1 | unit |
| UnknownDatastore | any string | kind=Validation, exit_code=2 | unit |
| Logging | any string | kind=Runtime, exit_code=1 | unit |
| Engine | any string | kind=Runtime, exit_code=1 | unit |
| InvalidHostname | any string | kind=Validation, exit_code=2 | unit |
| InvalidEndpoint | any string | kind=Validation, exit_code=2 | unit |
| NotFound | any string | kind=Runtime, exit_code=1 | unit |
| ApiError | any code/message | kind=Runtime, exit_code=1 | unit |
| Io | any io error | kind=Runtime, exit_code=1 | unit |

### Integration Tests: Handler Error-Body Preservation

| Scenario | Handler | Server Response | Expected CliError | Test Layer |
|----------|---------|-----------------|-------------------|------------|
| structured JSON 500 | queue_list | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON 500 | queue_list | 500 + plain text | HttpStatus { status: 500, reason: "Internal Server Error" } | integration |
| structured JSON 404 | queue_get | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | queue_get | 404 + plain text | NotFound(name) | integration |
| structured JSON 500 | queue_get | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON non-404 | queue_get | 503 + plain text | HttpStatus { status: 503, reason: "Service Unavailable" } | integration |
| structured JSON 404 | queue_delete | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | queue_delete | 404 + plain text | NotFound(name) | integration |
| structured JSON 500 | queue_delete | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON non-404 | queue_delete | 502 + plain text | HttpStatus { status: 502, reason: "Bad Gateway" } | integration |
| structured JSON 404 | task_get | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | task_get | 404 + plain text | NotFound(task_id) | integration |
| empty task_id | task_get | "" (empty string) | NotFound("") | integration |
| structured JSON 500 | task_get | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON non-404 | task_get | 503 + plain text | HttpStatus { status: 503, reason: "Service Unavailable" } | integration |
| structured JSON 404 | task_log | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | task_log | 404 + plain text | NotFound(task_id) | integration |
| structured JSON 500 | task_log | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON non-404 | task_log | 502 + plain text | HttpStatus { status: 502, reason: "Bad Gateway" } | integration |
| happy path 200 | queue_list | 200 + JSON array | Ok(body) | integration |
| happy path 200 | queue_get | 200 + JSON object | Ok(body) | integration |
| happy path 200 | queue_delete | 200 | Ok(body) | integration |

### Integration Tests: Trigger Negative Status

| Scenario | Handler | Server Response | Expected CliError | Test Layer |
|----------|---------|-----------------|-------------------|------------|
| structured JSON 500 | trigger_list | 500 + JSON | ApiError { code: 500, message } | integration |
| non-JSON 500 | trigger_list | 500 + plain text | HttpStatus { status: 500, reason: "Internal Server Error" } | integration |
| structured JSON 404 | trigger_get | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | trigger_get | 404 + plain text | NotFound(id) | integration |
| structured JSON 400 | trigger_get | 400 + JSON | ApiError { code: 400, message } | integration |
| structured JSON 400 | trigger_create | 400 + JSON | ApiError { code: 400, message } | integration |
| structured JSON 409 | trigger_update | 409 + JSON | ApiError { code: 409, message } | integration |
| structured JSON 404 | trigger_update | 404 + JSON | ApiError { code: 404, message } | integration |
| structured JSON 404 | trigger_delete | 404 + JSON | ApiError { code: 404, message } | integration |
| non-JSON 404 | trigger_delete | 404 + plain text | NotFound(id) | integration |

### Integration Tests: TriggerId Boundaries

| Scenario | Input | Expected Result | Test Layer |
|----------|-------|-----------------|------------|
| 2 chars (below min) | "ab" | Err(ApiError { code: 400 }) | integration |
| 3 chars (at min) | "abc" | Ok(body) | integration |
| 65 chars (above max) | 65-char string | Err(ApiError { code: 400 }) | integration |
| 64 chars (at max) | 64-char string | Ok(body) | integration |
| special chars | "bad trigger!" | Err(ApiError { code: 400 }) | integration |

### E2E Tests: CLI Binary Error Output

| Scenario | CLI Invocation | Expected Output | Test Layer |
|----------|---------------|-----------------|------------|
| queue list 500 | `twerk --json queue list --endpoint http://...` | JSON with error type and server message | e2e |
| task get 404 | `twerk --json task get --id missing` | JSON with error type and not found message | e2e |
| trigger get 400 | `twerk --json trigger get --id "ab"` | JSON with error type and 400 message | e2e |
| queue list text-mode (empty) | `twerk queue list --endpoint http://...` | stdout contains "No queues found." | e2e |
| queue list text-mode (non-empty) | `twerk queue list --endpoint http://...` | stdout contains "NAME" table header | e2e |

---

## Open Questions

1. **Queue/task error envelope shape**: The contract assumes queue and task server endpoints return the same `{ "error", "message" }` JSON envelope as trigger endpoints. If they don't, `parse_api_error` will fail to parse and fall back to `HttpStatus`/`NotFound` — which is the correct degraded behavior. Tests B31–B48 verify both structured JSON and non-JSON paths, so either way the tests are valid.

2. **serial_test vs test restructuring**: The plan specifies `serial_test` for env-var test serialization, but an alternative is to restructure logging tests to avoid shared mutable env state entirely (e.g., use a per-test tokio runtime with isolated env). The test-writer should choose the simpler approach.

3. **TriggerId boundary tests require server cooperation**: B22 and B24 (min/max boundary acceptance) require a live server with a trigger configured at exactly 3 or 64 chars. If `start_test_server()` from `twerk_web::helpers` supports creating such triggers, use it directly. Otherwise, a custom mock server must be built.

4. **E2E test scope**: The 3 E2E tests assume the twerk binary can be built and invoked against a running server. If CI doesn't support server startup in E2E tests, these should be marked `#[ignore]` with a note to run manually.

5. **Missing CliError variants in existing completeness checks**: The existing `bdd_behavior_report.rs` completeness_check only constructs 8 of 15 variants (missing: `Http`, `HttpStatus`, `NotFound`, `ApiError`, `InvalidHostname`, `InvalidEndpoint`, `Io`). The fix for B6 must expand this to all 15 variants. Similarly, `bdd_behavioral_contract_test.rs` constructs 9 of 15 (missing `Http`, `HttpStatus`, `NotFound`, `ApiError`, `InvalidHostname`, `InvalidEndpoint`). The fix for B8 must expand both.

6. **node.rs / metrics.rs / user.rs body drops**: These handlers have the same body-drop pattern but are out of scope for this bead. A follow-up bead should be filed after this work lands.

7. **Max-length and whitespace-only input boundaries**: Handler string parameters (queue name, task_id) are passed through without client-side validation. Max-length (1000+ chars creating multi-kilobyte URLs) and whitespace-only inputs (e.g. `"   "`) are server-validated. The client's job is to pass through and report server errors faithfully. URL encoding handles spaces. No additional client-side boundary tests are needed beyond what's specified.

8. **E2E tests must run in CI**: The E2E tests are structured as integration tests with embedded mock servers (using `start_test_server()` from `twerk_web::helpers`), NOT as `#[ignore]` tests. All 5 E2E tests run in CI.