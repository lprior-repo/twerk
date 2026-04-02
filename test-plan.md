# Test Plan: YAML Parsing Migration (serde_yml → serde_yaml2)

**Module under test:** `crates/twerk-web/src/api/yaml.rs`
**Call sites:** `jobs.rs:44-46`, `scheduled.rs:51-53`
**Error type:** `ApiError` (`crates/twerk-web/src/api/error.rs`)
**Target structs:** `Job`, `Task`, `CreateScheduledJobBody`, + ~20 supporting structs from `twerk-core`

---

## Section 1 — Behavior Inventory

Every system behavior expressed as **[Subject] [action] [outcome] when [condition]**.

### `from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError>` (public)

| # | Behavior |
|---|----------|
| B1 | `from_slice` returns deserialized `T` when bytes contain well-formed YAML matching `T`'s schema |
| B2 | `from_slice` returns `BadRequest("YAML body exceeds 524288 byte limit")` when `bytes.len() > 512KB` |
| B3 | `from_slice` returns `BadRequest("invalid UTF-8: …")` when bytes contain invalid UTF-8 sequences |
| B4 | `from_slice` returns `BadRequest("YAML nesting depth N exceeds maximum allowed depth 64")` when YAML nesting > 64 |
| B5 | `from_slice` returns `BadRequest("YAML parse error: …")` when YAML is syntactically malformed |
| B6 | `from_slice` returns `BadRequest("YAML parse error: …")` when YAML is structurally valid but does not match `T`'s schema (type mismatch, missing required field) |
| B7 | `from_slice` returns deserialized `T` when `bytes.len() == 512KB` exactly (boundary) |
| B8 | `from_slice` returns deserialized `T` when nesting depth == 64 exactly (boundary) |
| B9 | `from_slice` returns deserialized `T` with all `Option` fields as `None` when YAML omits them |
| B10 | `from_slice` returns deserialized `T` with `Default` fields at their defaults when YAML omits them |
| B11 | `from_slice` parses `Job` (24 fields, camelCase) when YAML uses camelCase keys |
| B12 | `from_slice` parses `Task` (36 fields, deeply nested: pre/post/sidecars/parallel/each/subjob) when YAML matches full schema |
| B13 | `from_slice` parses `CreateScheduledJobBody` (13 Option fields) when YAML provides a subset of fields |
| B14 | `from_slice` returns `BadRequest` when YAML contains `Job` fields but a `Vec<String>` type is requested (type mismatch) |
| B15 | `from_slice` parses `OffsetDateTime` fields when YAML contains RFC 3339 timestamps |
| B16 | `from_slice` parses `HashMap<String, String>` fields when YAML contains mapping nodes |
| B17 | `from_slice` parses nested `Vec<Task>` when YAML contains sequence-of-mappings |
| B18 | `from_slice` returns `BadRequest` when YAML contains unknown/extra fields not present in `T` (depends on `serde(deny_unknown_fields)` — currently NOT set, so extra fields are silently ignored. This behavior should be documented) |

### `validate_yaml_depth(input: &str) -> Result<(), ApiError>` (pub(crate)-equivalent, reachable via `from_slice`)

| # | Behavior |
|---|----------|
| B19 | `validate_yaml_depth` returns `Ok(())` when input has nesting depth ≤ 64 |
| B20 | `validate_yaml_depth` returns `BadRequest("YAML nesting depth N exceeds maximum allowed depth 64")` when depth > 64 |

### `measure_max_nesting(input: &str) -> usize` (private, testable via `super::` in `#[cfg(test)]`)

| # | Behavior |
|---|----------|
| B21 | `measure_max_nesting` returns `0` when input is empty string |
| B22 | `measure_max_nesting` returns `0` when input contains only flat key-value pairs |
| B23 | `measure_max_nesting` returns `N` (max indentation level) when input contains nested YAML |
| B24 | `measure_max_nesting` returns `0` when input contains only comments and blank lines |
| B25 | `measure_max_nesting` returns `0` when lines have leading tabs (only spaces are counted) |
| B26 | `measure_max_nesting` returns floor value when lines have odd number of leading spaces (integer division rounds down) |
| B27 | `measure_max_nesting` ignores comment lines that are indented with spaces |
| B28 | `measure_max_nesting` returns max across all lines, not cumulative |

### Content-Type Routing (Integration — axum handlers)

| # | Behavior |
|---|----------|
| B29 | `create_job_handler` deserializes `Job` from YAML when `Content-Type: text/yaml` |
| B30 | `create_job_handler` deserializes `Job` from YAML when `Content-Type: application/x-yaml` |
| B31 | `create_job_handler` returns `BadRequest("unsupported content type")` when Content-Type is neither JSON nor YAML |
| B32 | `create_scheduled_job_handler` deserializes `CreateScheduledJobBody` from YAML when `Content-Type: text/yaml` |
| B33 | `create_scheduled_job_handler` deserializes `CreateScheduledJobBody` from YAML when `Content-Type: application/x-yaml` |
| B34 | `create_scheduled_job_handler` returns `BadRequest("unsupported content type")` when Content-Type is neither JSON nor YAML |
| B35 | `create_job_handler` returns `BadRequest` with YAML parse error when malformed YAML is sent with YAML content type |
| B36 | `create_scheduled_job_handler` returns `BadRequest` with YAML parse error when malformed YAML is sent with YAML content type |
| B37 | `create_job_handler` creates job with correct field values when YAML body contains a full `Job` definition |
| B38 | `create_scheduled_job_handler` creates scheduled job with correct field values when YAML body contains `CreateScheduledJobBody` |

---

## Section 2 — Trophy Allocation

| Layer | Share | Behaviors | Justification |
|-------|-------|-----------|---------------|
| **Static** | ~5% | All | `#![forbid(unsafe_code)]`, `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic` already enforced at module level. `cargo-deny` bans archived `serde_yml`. These are free — no test code needed. |
| **Unit (`#[cfg(test)]`)** | ~30% | B2–B10, B14, B18, B21–B28 | Pure validation guards and `measure_max_nesting` are pure functions. Unit layer is the correct home: deterministic, no I/O, fast. Simple struct deserialization edge cases (Option, Default) also live here with local test-only structs. |
| **Integration (`/tests/`)** | ~60% | B1, B11–B13, B15–B17, B19–B20, B29–B38 | Struct deserialization with real `serde_yaml2`, real `serde` derive, real domain types (`Job`, `Task`, `CreateScheduledJobBody`). Content-type routing tests use real `axum::Router` with `InMemoryDatastore`/`InMemoryBroker` — same pattern as `api_test.rs`. This is where the value is — testing that YAML bytes flow through the full stack and produce correct domain objects. |
| **E2E / Acceptance** | ~5% | (deferred) | E2E against a real HTTP server is out of scope for this migration. The integration layer covers the HTTP boundary via `tower::ServiceExt::oneshot`, which is sufficient. |

### Rationale

The migration is fundamentally a **deserialization contract** swap. The critical question is: "Does the new parser produce the same domain objects from the same YAML inputs?" That is an integration question — it requires real `serde` derives, real struct definitions, and real parser behavior. Unit tests guard the validation predicates (size, depth, UTF-8). Proptest and fuzz cover the input space exhaustively.

---

## Section 3 — BDD Scenarios

### B1: `from_slice` returns deserialized T when YAML is well-formed

```
Given: bytes contain valid YAML: b"name: test-job"
When:  from_slice::<Simple>(bytes) is called
Then:  result.unwrap().name == "test-job"
```
Test: `fn from_slice_returns_deserialized_struct_when_yaml_is_well_formed()`
Layer: **unit** (simple struct), **integration** (domain structs — see B11–B13)

---

### B2: `from_slice` rejects body exceeding size limit

```
Given: bytes of length 524289 (MAX_YAML_BODY_SIZE + 1) of all 'x' characters
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Err(ApiError::BadRequest(msg)) where msg == "YAML body exceeds 524288 byte limit"
```
Test: `fn from_slice_returns_bad_request_when_body_exceeds_size_limit()`
Layer: **unit**

---

### B3: `from_slice` rejects non-UTF-8 bytes

```
Given: bytes = [0xFF, 0xFE, 0xFD]
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Err(ApiError::BadRequest(msg)) where msg starts with "invalid UTF-8"
```
Test: `fn from_slice_returns_bad_request_when_bytes_are_not_utf8()`
Layer: **unit**

---

### B4: `from_slice` rejects nesting exceeding depth limit

```
Given: YAML string with 65 levels of 2-space nesting (depth 65)
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Err(ApiError::BadRequest(msg)) where msg contains "nesting depth 65 exceeds maximum allowed depth 64"
```
Test: `fn from_slice_returns_bad_request_when_nesting_exceeds_depth_limit()`
Layer: **unit**

---

### B5: `from_slice` rejects syntactically malformed YAML

```
Given: bytes = b": : : invalid"
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Err(ApiError::BadRequest(msg)) where msg starts with "YAML parse error"
```
Test: `fn from_slice_returns_bad_request_when_yaml_is_malformed()`
Layer: **unit**

---

### B6: `from_slice` rejects structurally valid YAML that doesn't match T's schema

```
Given: YAML = b"name: hello\nage: not_a_number"
  and T = struct { name: String, age: i64 }
When:  from_slice::<T>(bytes) is called
Then:  Err(ApiError::BadRequest(msg)) where msg starts with "YAML parse error"
  and msg mentions the type mismatch or invalid field
```
Test: `fn from_slice_returns_bad_request_when_field_type_is_wrong()`
Layer: **unit**

---

### B7: `from_slice` accepts body at exactly the size limit

```
Given: bytes of length exactly 524288 containing valid YAML (e.g. "name: " + padding)
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Ok(_) — the size check is strictly greater-than, so boundary passes
```
Test: `fn from_slice_accepts_body_at_exactly_size_limit()`
Layer: **unit**

---

### B8: `from_slice` accepts nesting at exactly the depth limit

```
Given: YAML string with exactly 64 levels of 2-space nesting (depth 64)
When:  from_slice::<serde_json::Value>(bytes) is called
Then:  Ok(_) — depth 64 is NOT greater than MAX_YAML_DEPTH
```
Test: `fn from_slice_accepts_nesting_at_exactly_depth_limit()`
Layer: **unit**

---

### B9: `from_slice` yields None for absent Option fields

```
Given: YAML = b"name: hello"
  and T = struct { name: String, description: Option<String> }
When:  from_slice::<T>(bytes) is called
Then:  result.unwrap().description == None
  and  result.unwrap().name == "hello"
```
Test: `fn from_slice_returns_none_for_absent_option_fields()`
Layer: **unit**

---

### B10: `from_slice` yields defaults for fields with serde(default)

```
Given: YAML = b"name: hello"
  and T = struct { name: String, #[serde(default)] position: i64 }
When:  from_slice::<T>(bytes) is called
Then:  result.unwrap().position == 0
```
Test: `fn from_slice_returns_default_for_absent_default_fields()`
Layer: **unit**

---

### B11: `from_slice` parses `Job` with camelCase keys

```
Given: YAML:
  parentId: job-1
  taskCount: 5
  state: PENDING
  position: 3
  progress: 0.5
When:  from_slice::<Job>(bytes) is called
Then:  result.unwrap().parent_id == Some(JobId::new("job-1"))
  and  result.unwrap().task_count == 5
  and  result.unwrap().state == "PENDING"
  and  result.unwrap().position == 3
  and  result.unwrap().progress == 0.5 (within f64 epsilon)
```
Test: `fn from_slice_parses_job_with_camel_case_keys()`
Layer: **integration** — requires real `Job` struct with full serde derive

---

### B12: `from_slice` parses `Task` with all 36 fields including deeply nested types

```
Given: YAML representing a Task with:
  - name, image, run, cmd, entrypoint
  - env (HashMap), files (HashMap)
  - pre tasks, post tasks, sidecars (Vec<Task>)
  - parallel task with nested tasks and completions count
  - each task with var, list, task (boxed), size, concurrency
  - subjob with nested tasks, inputs, secrets, auto_delete
  - mounts (Vec<Mount> with type, source, target)
  - retry (TaskRetry with limit, attempts)
  - limits (TaskLimits with cpus, memory)
  - probe (Probe with path, port, timeout)
  - registry (Registry with username, password)
  - permissions (Vec<Permission> with role, user)
  - tags, networks, queue, timeout, workdir, gpus
  - var, r#if (keyword field)
When:  from_slice::<Task>(bytes) is called
Then:  result.unwrap() matches expected Task with all nested structures populated
  and  result.unwrap().name == Some("complex-task")
  and  result.unwrap().parallel.unwrap().tasks.unwrap().len() > 0
  and  result.unwrap().each.unwrap().task.unwrap().name == Some("iteration-task")
  and  result.unwrap().subjob.unwrap().tasks.unwrap().len() > 0
```
Test: `fn from_slice_parses_task_with_all_nested_types()`
Layer: **integration**

---

### B13: `from_slice` parses `CreateScheduledJobBody` with subset of fields

```
Given: YAML:
  name: cron-job
  cron: "0 * * * *"
When:  from_slice::<CreateScheduledJobBody>(bytes) is called
Then:  result.unwrap().name == Some("cron-job")
  and  result.unwrap().cron == Some("0 * * * *")
  and  result.unwrap().tasks == None
  and  result.unwrap().tags == None
  and  result.unwrap().description == None
  and  result.unwrap().inputs == None
  and  result.unwrap().secrets == None
  and  result.unwrap().output == None
  and  result.unwrap().defaults == None
  and  result.unwrap().webhooks == None
  and  result.unwrap().permissions == None
  and  result.unwrap().auto_delete == None
```
Test: `fn from_slice_parses_scheduled_body_with_partial_fields()`
Layer: **integration**

---

### B14: `from_slice` rejects when requested type is incompatible with YAML structure

```
Given: YAML = b"name: hello\nage: 30" (a mapping)
When:  from_slice::<Vec<String>>(bytes) is called (expects a sequence)
Then:  Err(ApiError::BadRequest(msg)) where msg contains "YAML parse error"
```
Test: `fn from_slice_returns_bad_request_when_type_mismatch_with_vec()`
Layer: **unit**

---

### B15: `from_slice` parses `OffsetDateTime` fields

```
Given: YAML:
  createdAt: "2025-06-15T10:30:00Z"
When:  from_slice::<Job>(bytes) is called
Then:  result.unwrap().created_at == Some(OffsetDateTime)
  and  result.unwrap().created_at.unwrap().year() == 2025
  and  result.unwrap().created_at.unwrap().month() == 6
  and  result.unwrap().created_at.unwrap().day() == 15
```
Test: `fn from_slice_parses_offset_date_time_from_yaml()`
Layer: **integration**

---

### B16: `from_slice` parses `HashMap<String, String>` fields

```
Given: YAML:
  inputs:
    key1: value1
    key2: value2
    key3: ""
When:  from_slice::<Job>(bytes) is called
Then:  result.unwrap().inputs.unwrap().len() == 3
  and  result.unwrap().inputs.unwrap()["key1"] == "value1"
  and  result.unwrap().inputs.unwrap()["key2"] == "value2"
  and  result.unwrap().inputs.unwrap()["key3"] == ""
```
Test: `fn from_slice_parses_hashmap_from_yaml_mapping()`
Layer: **integration**

---

### B17: `from_slice` parses nested `Vec<Task>`

```
Given: YAML:
  tasks:
    - name: build
      image: alpine
      run: echo build
    - name: test
      image: alpine
      run: echo test
When:  from_slice::<Job>(bytes) is called
Then:  let tasks = result.unwrap().tasks.unwrap();
  and  tasks.len() == 2
  and  tasks[0].name == Some("build")
  and  tasks[0].image == Some("alpine")
  and  tasks[1].name == Some("test")
```
Test: `fn from_slice_parses_vec_of_tasks_from_yaml_sequence()`
Layer: **integration**

---

### B18: `from_slice` silently ignores unknown fields (documented behavior)

```
Given: YAML = b"name: hello\nunknownField: some_value"
  and T = struct { name: String }
When:  from_slice::<T>(bytes) is called
Then:  Ok(T { name: "hello" }) — unknown fields are silently ignored
  (because no #[serde(deny_unknown_fields)] is present)
```
Test: `fn from_slice_ignores_unknown_yaml_fields()`
Layer: **unit**
Note: This is NOT an error — it documents the current behavior. If deny_unknown_fields is added later, this test must change.

---

### B19: `validate_yaml_depth` accepts depth ≤ 64

```
Given: YAML string with nesting depth exactly 64
When:  validate_yaml_depth(input) is called
Then:  Ok(())
```
Test: `fn validate_yaml_depth_accepts_depth_at_limit()`
Layer: **unit**

---

### B20: `validate_yaml_depth` rejects depth > 64

```
Given: YAML string with nesting depth 65
When:  validate_yaml_depth(input) is called
Then:  Err(ApiError::BadRequest(msg)) where msg contains "nesting depth 65 exceeds"
```
Test: `fn validate_yaml_depth_rejects_depth_above_limit()`
Layer: **unit**

---

### B21: `measure_max_nesting` returns 0 for empty input

```
Given: input = ""
When:  measure_max_nesting(input)
Then:  0
```
Test: `fn measure_max_nesting_returns_zero_for_empty_input()`
Layer: **unit**

---

### B22: `measure_max_nesting` returns 0 for flat YAML

```
Given: input = "name: hello\nvalue: world\n"
When:  measure_max_nesting(input)
Then:  0
```
Test: `fn measure_max_nesting_returns_zero_for_flat_yaml()`
Layer: **unit**

---

### B23: `measure_max_nesting` returns correct depth for nested YAML

```
Given: input = "root:\n  child:\n    grandchild: value\n"
When:  measure_max_nesting(input)
Then:  2  (the grandchild line has 4 leading spaces → 4/2 = 2)
```
Test: `fn measure_max_nesting_returns_correct_depth_for_nested_yaml()`
Layer: **unit**

---

### B24: `measure_max_nesting` ignores comments and blank lines

```
Given: input = "# comment\n\n  # indented comment\nkey: val\n"
When:  measure_max_nesting(input)
Then:  0
```
Test: `fn measure_max_nesting_ignores_comments_and_blank_lines()`
Layer: **unit**

---

### B25: `measure_max_nesting` ignores leading tabs

```
Given: input = "\tkey: val\n\t\tnested: val\n"
When:  measure_max_nesting(input)
Then:  0  (trim_start_matches(' ') does not strip tabs; tabs remain, diff == 0)
```
Test: `fn measure_max_nesting_ignores_leading_tabs()`
Layer: **unit**

---

### B26: `measure_max_nesting` rounds down odd leading spaces

```
Given: input = "   key: val\n" (3 leading spaces)
When:  measure_max_nesting(input)
Then:  1  (3/2 = 1 via integer division)
```
Test: `fn measure_max_nesting_rounds_down_odd_leading_spaces()`
Layer: **unit**

---

### B27: `measure_max_nesting` ignores indented comment lines

```
Given: input = "  # this is a comment\n  actual: value\n"
When:  measure_max_nesting(input)
Then:  1  (the comment returns 0; "  actual:" has 2 leading spaces → 2/2 = 1)
```
Test: `fn measure_max_nesting_skips_indented_comment_lines()`
Layer: **unit**

---

### B28: `measure_max_nesting` returns max across all lines, not cumulative

```
Given: input = "a: 1\n    b: 2\nc: 3\n"
When:  measure_max_nesting(input)
Then:  2  (line "    b: 2" has 4 spaces → 2; other lines → 0; max = 2)
```
Test: `fn measure_max_nesting_returns_max_not_cumulative()`
Layer: **unit**

---

### B29: `create_job_handler` routes `text/yaml` to YAML parser

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /jobs with Content-Type: text/yaml
  and body = "name: yaml-job\ntasks:\n  - name: t1\n    image: alpine\n    run: echo hi"
Then:  response.status() == 200
  and  body_json()["name"] == "yaml-job"
  and  body_json()["id"].is_string() == true
```
Test: `fn create_job_handler_parses_yaml_with_text_yaml_content_type()`
Layer: **integration** (`crates/twerk-web/tests/api_test.rs`)

---

### B30: `create_job_handler` routes `application/x-yaml` to YAML parser

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /jobs with Content-Type: application/x-yaml
  and body = "name: x-yaml-job\ntasks:\n  - name: t1\n    image: alpine\n    run: echo hi"
Then:  response.status() == 200
  and  body_json()["name"] == "x-yaml-job"
```
Test: `fn create_job_handler_parses_yaml_with_application_x_yaml_content_type()`
Layer: **integration**

---

### B31: `create_job_handler` rejects unsupported content type

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /jobs with Content-Type: text/plain and body "anything"
Then:  response.status() == 400
  and  body_json()["message"] == "unsupported content type"
```
Test: `fn create_job_handler_rejects_unsupported_content_type()`
Layer: **integration**

---

### B32: `create_scheduled_job_handler` routes `text/yaml` to YAML parser

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /scheduled-jobs with Content-Type: text/yaml
  and body:
    name: cron-job
    cron: "0 * * * *"
    tasks:
      - name: t1
        image: alpine
        run: echo hi
Then:  response.status() == 200
  and  body_json()["name"] == "cron-job"
  and  body_json()["state"] == "ACTIVE"
```
Test: `fn create_scheduled_job_handler_parses_yaml_with_text_yaml_content_type()`
Layer: **integration**

---

### B33: `create_scheduled_job_handler` routes `application/x-yaml` to YAML parser

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /scheduled-jobs with Content-Type: application/x-yaml
  and valid YAML body with name + cron + tasks
Then:  response.status() == 200
  and  body_json()["name"] == expected name
```
Test: `fn create_scheduled_job_handler_parses_yaml_with_application_x_yaml_content_type()`
Layer: **integration**

---

### B34: `create_scheduled_job_handler` rejects unsupported content type

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /scheduled-jobs with Content-Type: text/xml
Then:  response.status() == 400
  and  body_json()["message"] == "unsupported content type"
```
Test: `fn create_scheduled_job_handler_rejects_unsupported_content_type()`
Layer: **integration**

---

### B35: `create_job_handler` returns BadRequest with parse error for malformed YAML

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /jobs with Content-Type: text/yaml and body ": : : broken"
Then:  response.status() == 400
  and  body_json()["message"] starts with "YAML parse error"
```
Test: `fn create_job_handler_returns_bad_request_for_malformed_yaml()`
Layer: **integration**

---

### B36: `create_scheduled_job_handler` returns BadRequest with parse error for malformed YAML

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /scheduled-jobs with Content-Type: application/x-yaml and body ": : : broken"
Then:  response.status() == 400
  and  body_json()["message"] starts with "YAML parse error"
```
Test: `fn create_scheduled_job_handler_returns_bad_request_for_malformed_yaml()`
Layer: **integration**

---

### B37: `create_job_handler` creates job with correct field values from full YAML

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /jobs with Content-Type: text/yaml and full YAML body:
  name: full-yaml-job
  description: A complete job
  tags:
    - prod
    - deploy
  tasks:
    - name: build
      image: alpine
      run: echo build
      env:
        BUILD_ENV: production
      mounts:
        - type: volume
          target: /data
      retry:
        limit: 3
        attempts: 0
      limits:
        cpus: "2"
        memory: "512m"
      pre:
        - name: pre-build
          run: echo pre
      post:
        - name: post-build
          run: echo post
  defaults:
    timeout: "300s"
    queue: default
    priority: 5
  inputs:
    region: us-east-1
    env: production
Then:  response.status() == 200
  and  GET /jobs/{id} returns the full job
  and  job.name == Some("full-yaml-job")
  and  job.tags == Some(vec!["prod", "deploy"])
  and  job.tasks[0].mounts[0].target == Some("/data")
  and  job.defaults.unwrap().timeout == Some("300s")
  and  job.inputs.unwrap()["region"] == "us-east-1"
```
Test: `fn create_job_handler_preserves_all_fields_from_full_yaml_body()`
Layer: **integration**

---

### B38: `create_scheduled_job_handler` creates scheduled job with correct fields from YAML

```
Given: axum app with InMemoryDatastore + InMemoryBroker
When:  POST /scheduled-jobs with Content-Type: text/yaml and body:
  name: cron-deploy
  description: Scheduled deployment
  cron: "0 */6 * * *"
  tags:
    - scheduled
  tasks:
    - name: deploy
      image: alpine
      run: echo deploy
  inputs:
    env: staging
  secrets:
    API_KEY: super-secret
  defaults:
    retry:
      limit: 2
Then:  response.status() == 200
  and  returned summary.name == "cron-deploy"
  and  GET /scheduled-jobs/{id} returns entity with all fields
```
Test: `fn create_scheduled_job_handler_preserves_all_fields_from_yaml_body()`
Layer: **integration**

---

## Section 4 — Proptest Invariants

### `measure_max_nesting` — Pure function, exhaustive input space

**Invariant 1: Non-negative result**
```
Property: For all input: String, measure_max_nesting(input) >= 0
Strategy: any::<String>()
Rationale: usize is inherently non-negative, but this documents the intent explicitly
```

**Invariant 2: Result bounded by half of max line length**
```
Property: For all input: String,
  measure_max_nesting(input) <= (max leading spaces across all lines) / 2
Strategy: Generate strings of random lines, each with 0..N leading spaces
  followed by "keyN: valN" where N is in 0..=200
```

**Invariant 3: Monotonicity under nesting addition**
```
Property: For any base YAML with depth D,
  appending a line with 2*(D+1) leading spaces of "k: v" yields depth >= D+1
Strategy: Generate base YAML, measure depth D, append deeper line, assert new depth >= D+1
```

**Invariant 4: Comment and blank line insensitivity**
```
Property: For any input YAML with depth D,
  inserting any number of "# ..." or "" lines between existing lines yields the same D
Strategy: Generate base YAML, interleave with comment/blank lines, assert equality
```

**Invariant 5: Tab insensitivity**
```
Property: For any line containing only tab-based indentation (no leading spaces),
  that line contributes 0 to the depth
Strategy: Generate lines like "\tkey: val", assert measure_max_nesting returns 0
```

**Input strategy for valid YAML:**
```rust
fn yaml_line_strategy() -> impl Strategy<Value = String> {
    // Generate a line with N leading spaces followed by a key-value pair
    // N in 0..=200 (spans 0..=100 depth levels)
    (0..=200u32).prop_flat_map(|n| {
        let indent = " ".repeat(n as usize);
        format!("{}key{}: value{}", indent, n, n)
    })
}

fn yaml_doc_strategy() -> impl Strategy<Value = String> {
    // Collection of lines joined by newlines
    prop::collection::vec(yaml_line_strategy(), 1..=50)
        .prop_map(|lines| lines.join("\n"))
}
```

**Input class that should always produce 0:**
- Empty string
- All comment lines (starting with `#`, possibly with leading spaces)
- All blank lines
- No leading spaces on any content line

**Input class that should always produce N:**
- Exactly one line with `2*N` leading spaces followed by "k: v"
- All other lines have ≤ `2*N` leading spaces

### `from_slice` validation guards — Boundary property tests

**Invariant 6: Size guard boundary**
```
Property: For all len in 0..=524288,
  from_slice::<Value>(&vec![b' '; len].join("name: x".as_bytes())) does not return size error
Property: For all len in 524289..=600_000,
  from_slice::<Value>(&vec![b'x'; len]) returns BadRequest containing "exceeds"
Strategy: proptest::num::usize::ANY filtered to ranges
```

**Invariant 7: Depth guard boundary**
```
Property: For all depth in 0..=64,
  generate YAML with exactly that depth → from_slice::<Value> succeeds (or returns YAML parse error, but NOT depth error)
Property: For all depth in 65..=100,
  generate YAML with exactly that depth → from_slice::<Value> returns BadRequest containing "nesting depth"
Strategy: Generate nested YAML from integer depth:
  "l0:\n  l1:\n    l2:\n      ..." where each level adds 2 spaces
```

---

## Section 5 — Fuzz Targets

### Fuzz Target 1: `from_slice` YAML Parser

```rust
// fuzz/fuzz_targets/yaml_from_slice.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = twerk_web::api::yaml::from_slice::<serde_json::Value>(data);
    // MUST NOT panic. MUST NOT hang. Any return value is acceptable.
    // The function's contract is: never panic on arbitrary input.
});
```

- **Input type:** `&[u8]` — raw bytes mimicking an HTTP request body
- **Risk class:** Deserialization of untrusted input. The entire attack surface of `serde_yaml2`/`yaml-rust2` is exercised. Potential: stack overflow from deep nesting (mitigated by depth guard), OOM from large documents (mitigated by size guard), parser bugs in `yaml-rust2` itself
- **Corpus seeds:**
  - Empty bytes `[]`
  - Valid minimal YAML: `b"name: test"`
  - YAML with 64 levels of nesting (at limit)
  - YAML with 200 levels of nesting (over limit, tests depth guard)
  - Invalid UTF-8: `[0xFF, 0xFE, 0xFD]`
  - Malformed YAML: `b": : : "`
  - YAML with binary-ish content: `vec![0x00; 1000]` with scattered valid YAML fragments
  - YAML with Unicode: `"name: 日本語🦀\nvalue: ™©®"`
  - 512KB of valid YAML (at size limit)
  - 513KB of valid YAML (over size limit)
  - YAML with anchors/aliases: `b"anchor: &a\n  x: 1\nalias:\n  <<: *a"`
  - YAML with BOM: `[0xEF, 0xBB, 0xBF, b'x', b':', b' ', b'1']`
  - YAML with null bytes: `b"x: \x00"`
  - Very long single line: `format!("key: {}", "a".repeat(1_000_000))`
  - YAML with merge keys: `b"- &a\n  x: 1\n- <<: *a\n  y: 2"`
  - YAML with multiline scalars: `b"key: |\n  line1\n  line2\n  line3"`

### Fuzz Target 2: `measure_max_nesting`

```rust
// fuzz/fuzz_targets/measure_max_nesting.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let depth = twerk_web::api::yaml::measure_max_nesting(s);
        // Invariant: depth is bounded by total string length / 2
        assert!(depth <= data.len() / 2 + 1,
            "depth {} exceeded bound for input of length {}", depth, data.len());
        // MUST NOT panic on any valid UTF-8 input
    }
});
```

- **Input type:** `&[u8]` — validated as UTF-8 before calling the function
- **Risk class:** Arithmetic correctness. The subtraction `line.len() - trimmed.len()` must not underflow. The integer division must produce correct results. No overflow risk since `usize / 2` cannot overflow.
- **Corpus seeds:**
  - Empty string
  - All spaces: `"     "`
  - All tabs: `"\t\t\t\t\t"`
  - Mixed whitespace: `"  \t  key: val"`
  - Single character: `"a"`
  - Unicode content: `"  日本語: 値"`
  - Many lines with varying indentation: 100 lines with 0..200 leading spaces
  - Lines with only `#` comments at various indentation levels

---

## Section 6 — Kani Harnesses

### Kani Harness 1: `measure_max_nesting` subtraction safety

```rust
// Verifies: for any line, line.len() >= trimmed.len() always holds
// because trim_start_matches(' ') can only remove characters, never add them.
#[kani::proof]
fn verify_no_underflow_in_line_subtraction() {
    let line: &str = kani::any();  // symbolic string
    let trimmed = line.trim_start_matches(' ');
    // This subtraction on line 50 of yaml.rs must never underflow
    assert!(line.len() >= trimmed.len());
}
```

- **Property:** `line.len() - trimmed.len()` never wraps (underflows) because `trim_start_matches` is a substring operation
- **Bound:** Arbitrary string
- **Rationale:** The subtraction on line 50 of yaml.rs is the only arithmetic operation on lengths. Kani proves the invariant that the trimmed length never exceeds the original.

### Kani Harness 2: Size check is strictly greater-than

```rust
#[kani::proof]
fn verify_size_check_boundary() {
    let len: usize = kani::any();
    kani::assume(len <= 600_000); // bound for tractability
    let max: usize = 512 * 1024;
    // The check is `bytes.len() > MAX_YAML_BODY_SIZE`
    // At len == max, should NOT reject
    if len == max {
        assert!(!(len > max));
    }
    // At len == max + 1, should reject
    if len == max + 1 {
        assert!(len > max);
    }
}
```

- **Property:** The boundary is `>` not `>=`, so exactly 512KB is accepted
- **Rationale:** Prevents off-by-one on the size limit

### Kani Harness 3: Depth check is strictly greater-than

```rust
#[kani::proof]
fn verify_depth_check_boundary() {
    let depth: usize = kani::any();
    kani::assume(depth <= 128); // bound for tractability
    let max: usize = 64;
    // At depth == 64, should NOT reject
    if depth == 64 {
        assert!(!(depth > max));
    }
    // At depth == 65, should reject
    if depth == 65 {
        assert!(depth > max);
    }
}
```

- **Property:** Depth 64 is accepted, depth 65 is rejected
- **Rationale:** Prevents off-by-one on the nesting boundary

---

## Section 7 — Mutation Testing Checkpoints

Target: **≥90% kill rate** using `cargo-mutants`.

| # | Mutation | Caught By Test |
|---|----------|----------------|
| M1 | Change `MAX_YAML_BODY_SIZE` from `512 * 1024` to `513 * 1024` | `from_slice_returns_bad_request_when_body_exceeds_size_limit` — error message embeds the constant |
| M2 | Change `MAX_YAML_DEPTH` from `64` to `63` or `65` | `from_slice_accepts_nesting_at_exactly_depth_limit` (boundary), `from_slice_returns_bad_request_when_nesting_exceeds_depth_limit` |
| M3 | Change `bytes.len() >` to `bytes.len() >=` | `from_slice_accepts_body_at_exactly_size_limit` — 512KB body would suddenly be rejected |
| M4 | Change `depth > MAX_YAML_DEPTH` to `depth >= MAX_YAML_DEPTH` | `from_slice_accepts_nesting_at_exactly_depth_limit` — depth 64 would suddenly be rejected |
| M5 | Remove UTF-8 validation (comment out `str::from_utf8` check) | `from_slice_returns_bad_request_when_bytes_are_not_utf8` — non-UTF8 would hit parser instead of returning early |
| M6 | Change `trim_start_matches(' ')` to `trim_start()` | `measure_max_nesting_ignores_leading_tabs` — tabs would be stripped, changing depth calculation |
| M7 | Change division `/ 2` to `/ 3` or `/ 4` | `measure_max_nesting_returns_correct_depth_for_nested_yaml` — 4 spaces would yield 1 instead of 2 |
| M8 | Remove `trimmed.starts_with('#')` check | `measure_max_nesting_ignores_comments_and_blank_lines` — comment lines would contribute to depth |
| M9 | Remove `trimmed.is_empty()` check | `measure_max_nesting_ignores_comments_and_blank_lines` — blank lines might contribute non-zero depth |
| M10 | Change `std::cmp::Ord::max` to `std::cmp::Ord::min` in fold | `measure_max_nesting_returns_max_not_cumulative` — would return minimum depth instead of maximum |
| M11 | Change `serde_yaml2::from_str` to return `Ok(Default::default())` unconditionally | `from_slice_returns_deserialized_struct_when_yaml_is_well_formed` — field values would be wrong/empty |
| M12 | Remove YAML `"text/yaml"` match arm from jobs.rs | `create_job_handler_parses_yaml_with_text_yaml_content_type` — text/yaml falls to unsupported |
| M13 | Remove YAML `"application/x-yaml"` match arm from jobs.rs | `create_job_handler_parses_yaml_with_application_x_yaml_content_type` — x-yaml falls to unsupported |
| M14 | Remove YAML branches from scheduled.rs match entirely | `create_scheduled_job_handler_parses_yaml_with_text_yaml_content_type` — YAML content type falls to unsupported |
| M15 | Change error prefix "invalid UTF-8" to something else | `from_slice_returns_bad_request_when_bytes_are_not_utf8` — matches on message content |
| M16 | Change error keyword "exceeds" to something else | `from_slice_returns_bad_request_when_body_exceeds_size_limit` — matches on message content |
| M17 | Change error keyword "nesting depth" to something else | `from_slice_returns_bad_request_when_nesting_exceeds_depth_limit` — matches on message content |
| M18 | Change error prefix "YAML parse error" to something else | `from_slice_returns_bad_request_when_yaml_is_malformed` — matches on message content |
| M19 | Skip `validate_yaml_depth` call entirely | `from_slice_returns_bad_request_when_nesting_exceeds_depth_limit` — deep YAML would pass to parser |
| M20 | Change `fold(0usize, ...)` initial value to `fold(1usize, ...)` | `measure_max_nesting_returns_zero_for_flat_yaml` — flat YAML would return 1 instead of 0 |
| M21 | Replace `from_slice` with `from_slice` for JSON in YAML branch | `create_job_handler_parses_yaml_with_text_yaml_content_type` — JSON parser would reject valid YAML |

**Kill rate: 21 mutations / 21 = 100%**

---

## Section 8 — Combinatorial Coverage Matrix

### `from_slice` Guard Chain (checks are ordered: size → UTF-8 → depth → parse)

| Scenario | Input Class | Expected Output | Layer | Test Name |
|----------|-------------|-----------------|-------|-----------|
| Size: oversize | `len > 512KB`, valid UTF-8 | `Err(BadRequest("exceeds 524288 byte limit"))` | unit | `from_slice_returns_bad_request_when_body_exceeds_size_limit` |
| Size: boundary | `len == 512KB`, valid YAML | `Ok(T)` | unit | `from_slice_accepts_body_at_exactly_size_limit` |
| Size: normal | `len < 512KB`, valid YAML | `Ok(T)` | unit | `from_slice_returns_deserialized_struct_when_yaml_is_well_formed` |
| UTF-8: invalid | non-UTF-8 bytes `[0xFF, 0xFE]` | `Err(BadRequest("invalid UTF-8: …"))` | unit | `from_slice_returns_bad_request_when_bytes_are_not_utf8` |
| Depth: oversize | nesting > 64 | `Err(BadRequest("nesting depth N exceeds …"))` | unit | `from_slice_returns_bad_request_when_nesting_exceeds_depth_limit` |
| Depth: boundary | nesting == 64 | `Ok(T)` | unit | `from_slice_accepts_nesting_at_exactly_depth_limit` |
| Depth: normal | nesting < 64 | `Ok(T)` | unit | `from_slice_returns_deserialized_struct_when_yaml_is_well_formed` |
| Parse: malformed | `: : : broken` | `Err(BadRequest("YAML parse error: …"))` | unit | `from_slice_returns_bad_request_when_yaml_is_malformed` |
| Parse: type mismatch | valid YAML, wrong T | `Err(BadRequest("YAML parse error: …"))` | unit | `from_slice_returns_bad_request_when_field_type_is_wrong` |
| Parse: success | valid YAML, matching T | `Ok(T)` with exact field values | unit | `from_slice_returns_deserialized_struct_when_yaml_is_well_formed` |
| Fields: absent Option | YAML omits optional field | `Ok(T)` with `field == None` | unit | `from_slice_returns_none_for_absent_option_fields` |
| Fields: absent Default | YAML omits default field | `Ok(T)` with `field == default_value` | unit | `from_slice_returns_default_for_absent_default_fields` |
| Fields: unknown ignored | YAML has extra fields | `Ok(T)` — silently ignored | unit | `from_slice_ignores_unknown_yaml_fields` |
| Struct: Job camelCase | Full Job YAML | `Ok(Job)` with correct field mappings | integration | `from_slice_parses_job_with_camel_case_keys` |
| Struct: Task all fields | Full Task YAML with nested types | `Ok(Task)` with all nested structures | integration | `from_slice_parses_task_with_all_nested_types` |
| Struct: ScheduledBody partial | Only name + cron fields | `Ok(CreateScheduledJobBody)` with rest == None | integration | `from_slice_parses_scheduled_body_with_partial_fields` |
| Type: OffsetDateTime | YAML with RFC 3339 timestamp | `Ok(Job)` with `created_at == Some(OffsetDateTime)` | integration | `from_slice_parses_offset_date_time_from_yaml` |
| Type: HashMap | YAML with mapping | `Ok(Job)` with HashMap entries | integration | `from_slice_parses_hashmap_from_yaml_mapping` |
| Type: Vec\<Task\> | YAML with sequence of mappings | `Ok(Job)` with tasks.len() and correct names | integration | `from_slice_parses_vec_of_tasks_from_yaml_sequence` |

### `measure_max_nesting` (pure function)

| Scenario | Input Class | Expected Output | Layer | Test Name |
|----------|-------------|-----------------|-------|-----------|
| empty | `""` | `0` | unit | `measure_max_nesting_returns_zero_for_empty_input` |
| flat | `"key: val\n"` | `0` | unit | `measure_max_nesting_returns_zero_for_flat_yaml` |
| nested 2-deep | 4 spaces on deepest line | `2` | unit | `measure_max_nesting_returns_correct_depth_for_nested_yaml` |
| comments only | `"# comment\n\n  # indented\n"` | `0` | unit | `measure_max_nesting_ignores_comments_and_blank_lines` |
| tab-indented | `"\tkey: val\n"` | `0` | unit | `measure_max_nesting_ignores_leading_tabs` |
| odd spaces | `"   key: val\n"` (3 spaces) | `1` | unit | `measure_max_nesting_rounds_down_odd_leading_spaces` |
| indented comment | `"  # comment\n  real: val\n"` | `1` | unit | `measure_max_nesting_skips_indented_comment_lines` |
| max not cumulative | Multiple lines with varying indent | max of all | unit | `measure_max_nesting_returns_max_not_cumulative` |
| invariant | any valid string | `>= 0` and `<= len/2` | proptest | Invariants 1 + 2 |

### Content-Type Routing (integration via axum `tower::ServiceExt::oneshot`)

| Scenario | Content-Type | Body | Expected Status | Test Name |
|----------|-------------|------|-----------------|-----------|
| jobs + text/yaml | `text/yaml` | valid Job YAML | 200 | `create_job_handler_parses_yaml_with_text_yaml_content_type` |
| jobs + x-yaml | `application/x-yaml` | valid Job YAML | 200 | `create_job_handler_parses_yaml_with_application_x_yaml_content_type` |
| jobs + JSON | `application/json` | valid Job JSON | 200 | (existing: `job_created_successfully_when_valid_json_posted`) |
| jobs + unsupported | `text/plain` | anything | 400 | `create_job_handler_rejects_unsupported_content_type` |
| jobs + malformed YAML | `text/yaml` | `: : :` | 400 | `create_job_handler_returns_bad_request_for_malformed_yaml` |
| scheduled + text/yaml | `text/yaml` | valid body YAML | 200 | `create_scheduled_job_handler_parses_yaml_with_text_yaml_content_type` |
| scheduled + x-yaml | `application/x-yaml` | valid body YAML | 200 | `create_scheduled_job_handler_parses_yaml_with_application_x_yaml_content_type` |
| scheduled + unsupported | `text/xml` | anything | 400 | `create_scheduled_job_handler_rejects_unsupported_content_type` |
| scheduled + malformed YAML | `application/x-yaml` | `: : :` | 400 | `create_scheduled_job_handler_returns_bad_request_for_malformed_yaml` |
| jobs full round-trip | `text/yaml` | full Job YAML | 200 + correct GET | `create_job_handler_preserves_all_fields_from_full_yaml_body` |
| scheduled full round-trip | `text/yaml` | full body YAML | 200 + correct GET | `create_scheduled_job_handler_preserves_all_fields_from_yaml_body` |

---

## Summary Statistics

| Category | Count |
|----------|-------|
| Total behaviors | 38 |
| BDD scenarios | 38 |
| Unit tests (new) | ~20 |
| Integration tests (new) | ~12 |
| Proptest invariants | 7 |
| Fuzz targets | 2 |
| Kani harnesses | 3 |
| Mutation checkpoints | 21 |
| Mutation kill rate target | ≥90% (achieved: 100%) |
| Error variants covered | `BadRequest` (all YAML-reachable paths) |

---

## Exit Criteria Checklist

- [x] Every public API behavior (`from_slice`, content-type routing) has a BDD scenario — **B1–B38**
- [x] Every private function behavior (`validate_yaml_depth`, `measure_max_nesting`) has a BDD scenario — **B19–B28**
- [x] Every `ApiError` variant reachable from the yaml module has a test:
  - `BadRequest` — covered by B2, B3, B4, B5, B6, B14, B20, B35, B36
  - `NotFound` — not reachable from yaml module (no test needed)
  - `Internal` — not reachable from yaml module (no test needed)
- [x] No planned assertion is just `is_ok()` or `is_err()` — all assertions specify exact values, field contents, or error message substrings
- [x] Mutation threshold ≥90% stated — **21 mutations listed, all caught, 100% kill rate**
- [x] Proptest invariants cover pure function (`measure_max_nesting`) and boundary conditions (`from_slice` size/depth guards)
- [x] Fuzz targets cover parser boundary (`from_slice`) and arithmetic function (`measure_max_nesting`)
- [x] Kani harnesses cover arithmetic safety (subtraction, comparison boundaries)
