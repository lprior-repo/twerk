# Test Plan: twerk-web API / OpenAPI Contract Parity

## Summary
- Behaviors identified: 28
- Trophy allocation: 8 unit / 16 integration / 2 e2e / 2 static
- Proptest invariants: 6
- Fuzz targets: 7
- Kani harnesses: 4
- Mutation threshold: **≥90% kill rate**

## Scope and current risk readout

Focused files reviewed:
- `crates/twerk-web/src/api/**/*`
- `docs/openapi.json`
- `crates/twerk-web/openapi.json`
- `crates/twerk-web/tests/*.rs`
- `qa/*.yaml`
- `.github/workflows/*.yml`

Highest-risk findings to lock down first:
1. **OpenAPI CI is ineffective**: workflow generates `openapi.json` at repo root, but repo tracks `crates/twerk-web/openapi.json` and `docs/openapi.json`; root file is untracked, so drift can pass CI.
2. **Live `/openapi.json` parity is barely tested**: current coverage is essentially `200` + `body["info"].is_object()`.
3. **Spec/runtime mismatches exist now**:
   - `GET /jobs` runtime returns paginated page object, spec says array of `Job`.
   - `POST /jobs` accepts YAML + JSON and detached mode returns summary-like payload, spec documents JSON string request body and `Job` response only.
   - `PUT/POST /jobs/{id}/cancel` and `PUT /jobs/{id}/restart` return `{ "status": "OK" }`, spec says body `Job`.
   - Queue endpoints have almost no response schema in OpenAPI despite `QueueInfo` schema existing.
4. **Queue contract is unstable in tests**: some tests expect `404` for nonexistent queues, others still expect `200` for arbitrary names.
5. **Many current tests are smoke-only** (`is_array` / `is_object`) and would survive major contract breakage.

---

## 1. Behavior Inventory

1. Health returns `200` with `status=UP` and exact server version when datastore and broker are healthy.
2. Health returns `503` with `status=DOWN` and exact server version when datastore or broker health check fails.
3. Live `/openapi.json` returns a valid OpenAPI document generated from `ApiDoc`.
4. Live `/openapi.json` stays byte/JSON-value equivalent to `crates/twerk-web/openapi.json` when code and artifacts are in sync.
5. `docs/openapi.json` stays equivalent to `crates/twerk-web/openapi.json` when published docs are in sync with runtime artifacts.
6. OpenAPI CI fails when generated spec differs from either checked-in artifact.
7. Job creation accepts valid JSON and returns the exact detached/blocking contract for the selected wait mode.
8. Job creation accepts valid YAML and returns the same semantic contract as JSON input.
9. Job creation rejects unsupported content type with `400` and exact message `unsupported content type`.
10. Job creation rejects malformed JSON/YAML with `400` and a specific parse/validation message.
11. Job creation rejects invalid job definitions with `400` and exact validation message(s).
12. Job list returns a paginated page object with exact pagination metadata and redacted summaries when query params are valid or coercible.
13. Job list clamps/normalizes invalid pagination inputs to documented defaults when page/size are absent, non-numeric, zero, or out of range.
14. Job fetch returns the exact stored job, actual task states, and redacted secrets when ID exists.
15. Job fetch returns `404` with exact `{"message":"job not found"}` when ID does not exist.
16. Job cancel returns `200` with exact success payload when job is cancellable.
17. Job cancel rejects terminal-state jobs with `400` and exact message `job cannot be cancelled in its current state`.
18. Job restart returns `200` with exact success payload when job state is `FAILED` or `CANCELLED`.
19. Job restart rejects non-restartable jobs with `400` and exact message `job cannot be restarted`.
20. Queue list returns an exact array of `QueueInfo` objects for existing queues.
21. Queue fetch returns exact `QueueInfo { name, size, subscribers, unacked }` when queue exists.
22. Queue fetch returns `404` with exact `{"message":"queue <name> not found"}` when queue does not exist.
23. Queue delete returns `200` when queue exists and subsequent fetch returns `404`.
24. Queue delete returns `404` with exact `{"message":"queue <name> not found"}` when queue does not exist.
25. Scheduled job creation accepts valid JSON/YAML and returns exact summary payload with generated ID and redacted secrets.
26. Scheduled job creation rejects missing cron/tasks, invalid cron, invalid job body, and unsupported content type with exact `400` payloads.
27. Scheduled job lifecycle fetch/list/pause/resume/delete enforces exact state transitions and exact `400/404` errors for invalid states or missing IDs.
28. Trigger CRUD enforces exact create/get/update/delete/version-conflict/id-mismatch/content-type/body-size/malformed-json contracts.

---

## 2. Trophy Allocation

| Priority | Behavior group | Layer | Why this layer | Tool/runner |
|---|---|---|---|---|
| P0 | OpenAPI artifact sync (`3-6`) | Static + integration | CI must fail before merge; also verify live endpoint emits same doc | GitHub Actions + `cargo test -p twerk-web` |
| P0 | Contract parity for jobs (`7-19`) | Integration | Public HTTP contract; real router + in-memory deps exposes current spec drift | axum router tests |
| P0 | Queue contract stabilization (`20-24`) | Integration | Behavior depends on broker implementation; must lock API-visible semantics | axum router tests + broker contract tests |
| P0 | Trigger negative-path contract (`28`) | Integration | Richest error surface; already contract-heavy, must keep exact status/error payloads | axum router tests |
| P1 | Scheduled job lifecycle (`25-27`) | Integration | State transitions and publish side effects are API contracts | axum router tests |
| P1 | Pagination coercion (`12-13`) | Unit + integration | Pure normalization logic plus end-to-end HTTP proof | `#[test]`, `proptest`, axum router |
| P1 | OpenAPI schema shape sanity for queues/jobs/scheduled | Integration | Validate artifact content against known contract hotspots | parse JSON + exact path assertions |
| P2 | Health / metrics / nodes / users | Integration | Lower drift risk than job/queue/spec endpoints, still public API | axum router tests |
| P2 | QA YAML scenario execution | E2E | Ensures published YAML workflows still exercise contract from outside | run selected `qa/*.yaml` |

Target ratio for this plan: integration-heavy by design because the risk is contract drift at HTTP and artifact boundaries.

---

## 3. Prioritized Test Matrix

| Pri | Area | Scenario | Layer | Must assert exactly |
|---|---|---|---|---|
| P0 | OpenAPI CI | generated spec differs from checked-in artifacts | static | workflow exits non-zero and names which artifact drifted |
| P0 | Live OpenAPI parity | `GET /openapi.json` equals `ApiDoc::openapi()` normalized JSON | integration | whole JSON value equality |
| P0 | Artifact sync | `docs/openapi.json` equals `crates/twerk-web/openapi.json` | static | whole JSON value equality |
| P0 | Jobs parity | `/jobs` spec response schema matches actual paginated object | integration | `items`, `page`, `size`, `total_items`, `total_pages` exact values |
| P0 | Jobs parity | `POST /jobs` detached response shape matches contract | integration | exact returned keys and values, not just `id` exists |
| P0 | Jobs parity | cancel/restart response body matches documented contract | integration | exact JSON `{"status":"OK"}` or spec updated accordingly |
| P0 | Queue stabilization | nonexistent queue GET returns canonical status/payload across broker modes | integration | exact status + exact message |
| P0 | Queue stabilization | existing queue GET returns exact `QueueInfo` fields | integration | `name`, `size`, `subscribers`, `unacked` exact ints |
| P0 | Queue stabilization | delete existing queue removes it | integration | delete `200`, then GET `404` exact payload |
| P0 | Trigger contract | stale version update returns `409` + `error=VersionConflict` | integration | status 409, exact `error`, exact `message` |
| P1 | Scheduled jobs | create/pause/resume/delete lifecycle | integration | exact state before/after and exact success/error payloads |
| P1 | Jobs negative | unsupported content-type / malformed JSON / invalid job | integration | exact `400` payload message |
| P1 | Users negative | invalid username/password/missing fields | integration | exact `400` payload message |
| P1 | Health/OpenAPI QA | external smoke upgraded to contract check | e2e | exact `info.title`, `info.version`, critical path existence |
| P2 | Metrics | stable object keys | integration | exact presence/type of `jobs`, `tasks`, `nodes` sub-objects |

---

## 4. Minimal set to add or strengthen first for test-reviewer approval

### First wave: must land before anything else

1. **Replace broken OpenAPI sync workflow**
   - Generate to a temp file.
   - Compare temp file against:
     - `crates/twerk-web/openapi.json`
     - `docs/openapi.json`
   - Fail on any diff.
   - Also fail if the two checked-in artifacts differ from each other.

2. **Add live `/openapi.json` parity integration test**
   - Boot router.
   - GET `/openapi.json`.
   - Parse JSON.
   - Compare normalized JSON value to `ApiDoc::openapi()` and both checked-in artifacts.

3. **Add contract hotspot test for documented schema mismatches**
   - Assert OpenAPI paths for:
     - `/jobs`
     - `/jobs/{id}/cancel`
     - `/jobs/{id}/restart`
     - `/queues`
     - `/queues/{name}`
     - `/scheduled-jobs`
   - Compare documented response/request schemas to actual runtime payloads from router.
   - This should intentionally fail until spec/runtime are reconciled.

4. **Stabilize queue contract with broker-backed integration tests**
   - Existing queue returns exact `QueueInfo`.
   - Nonexistent queue returns exact `404` payload.
   - Delete existing queue then GET returns `404`.
   - Run same assertions for at least in-memory broker and the RabbitMQ code path that does not have management metadata, or explicitly disable unsupported behavior and test that decision.

5. **Strengthen critical API happy/negative tests**
   - `POST /jobs` JSON happy path
   - `POST /jobs` YAML happy path
   - `GET /jobs` pagination path
   - `PUT /jobs/{id}/cancel` happy + terminal-state negative
   - `PUT /jobs/{id}/restart` happy + invalid-state negative
   - `POST /scheduled-jobs` happy + invalid cron negative
   - `PUT /api/v1/triggers/{id}` stale version negative

### Second wave: should follow immediately

6. Add exact-assertion coverage for `/users`, `/metrics`, `/nodes`.
7. Promote selected `qa/*.yaml` checks into CI-facing assertions for OpenAPI and queues.
8. Add mutation gate (`cargo-mutants`) focused on API contract modules.

---

## 5. BDD Scenarios

### Behavior: health returns UP when dependencies are healthy
Given: an app state with healthy datastore and broker
When: `GET /health`
Then: status is `200` and body equals `{"status":"UP","version":"<crate-version>"}`

Test function name: `fn health_returns_up_with_exact_version_when_dependencies_are_healthy()`

### Behavior: health returns DOWN when a dependency is unhealthy
Given: datastore or broker health check fails
When: `GET /health`
Then: status is `503` and body equals `{"status":"DOWN","version":"<crate-version>"}`

Test function name: `fn health_returns_down_with_exact_version_when_dependency_health_check_fails()`

### Behavior: live openapi endpoint returns generated spec
Given: router created from current code
When: `GET /openapi.json`
Then: status is `200` and parsed JSON exactly equals `ApiDoc::openapi()` serialized JSON value

Test function name: `fn openapi_endpoint_returns_exact_generated_spec_when_router_is_running()`

### Behavior: checked-in artifacts stay in sync
Given: checked-in OpenAPI artifacts
When: both files are parsed as JSON
Then: `docs/openapi.json == crates/twerk-web/openapi.json`

Test function name: `fn checked_in_openapi_artifacts_are_exactly_equal_when_docs_are_in_sync()`

### Behavior: job creation accepts valid JSON
Given: valid JSON job body and `Content-Type: application/json`
When: `POST /jobs`
Then: status is `200`; body contains exact job name, exact generated/non-empty id string, and detached response contract chosen by product decision

Error variant:
Given: malformed JSON body
When: `POST /jobs`
Then: status `400` and body equals `{"message":"<serde parse message>"}`

Test function name: `fn create_job_returns_exact_contract_when_valid_json_is_posted()`

### Behavior: job creation accepts valid YAML
Given: valid YAML job body and YAML content type
When: `POST /jobs`
Then: status is `200` and returned semantic fields equal the JSON path result for same job definition

Error variant:
Given: unsupported content type
When: `POST /jobs`
Then: status `400` and body equals `{"message":"unsupported content type"}`

Test function name: `fn create_job_accepts_yaml_and_returns_same_semantic_contract_as_json()`

### Behavior: job creation rejects invalid job definitions
Given: body missing required tasks or invalid output/defaults combination
When: `POST /jobs`
Then: status `400` and body equals exact validation message string emitted by `validate_job`

Test function name: `fn create_job_returns_exact_validation_message_when_job_definition_is_invalid()`

### Behavior: job list returns paginated page object
Given: datastore containing known jobs
When: `GET /jobs?page=1&size=2`
Then: status `200`; `items.len()==2`; `page==1`; `size==2`; `total_items==N`; `total_pages==ceil(N/2)`; first item names equal inserted order/contracted order

Error variant:
Given: non-numeric page/size
When: `GET /jobs?page=abc&size=xyz`
Then: status `200`; body equals default pagination contract (`page=1`, default size, matching totals)

Test function name: `fn list_jobs_returns_exact_page_metadata_when_pagination_query_is_applied()`

### Behavior: job fetch returns exact job and redacted fields
Given: stored job with secrets and updated task states
When: `GET /jobs/{id}`
Then: status `200`; body `id`, `name`, `state`, and `tasks[*].state` match datastore; secret-bearing fields are redacted per hook policy

Error variant:
Given: nonexistent job id
When: `GET /jobs/{id}`
Then: status `404` and body equals `{"message":"job not found"}`

Test function name: `fn get_job_returns_exact_job_and_redacted_tasks_when_id_exists()`

### Behavior: job cancel succeeds for cancellable jobs
Given: job in `PENDING` or `RUNNING`
When: `PUT /jobs/{id}/cancel`
Then: status `200` and body equals `{"status":"OK"}`

Error variant:
Given: job in `COMPLETED`, `FAILED`, or `CANCELLED`
When: same action
Then: status `400` and body equals `{"message":"job cannot be cancelled in its current state"}`

Test function name: `fn cancel_job_returns_status_ok_when_job_is_cancellable()`

### Behavior: job restart succeeds only for failed or cancelled jobs
Given: job in `FAILED` or `CANCELLED`
When: `PUT /jobs/{id}/restart`
Then: status `200` and body equals `{"status":"OK"}`

Error variant:
Given: job in `PENDING`, `RUNNING`, or `COMPLETED`
When: same action
Then: status `400` and body equals `{"message":"job cannot be restarted"}`

Test function name: `fn restart_job_returns_status_ok_when_job_is_restartable()`

### Behavior: queue list returns exact queue infos
Given: broker with known queues and subscriber counts
When: `GET /queues`
Then: status `200` and body equals exact ordered/unordered array of `QueueInfo` objects with `name`, `size`, `subscribers`, `unacked`

Test function name: `fn list_queues_returns_exact_queueinfo_objects_when_queues_exist()`

### Behavior: queue fetch returns exact queue info
Given: existing queue `x-jobs`
When: `GET /queues/x-jobs`
Then: status `200` and body equals `{"name":"x-jobs","size":<n>,"subscribers":<n>,"unacked":<n>}`

Error variant:
Given: nonexistent queue
When: same action
Then: status `404` and body equals `{"message":"queue no-such-queue not found"}`

Test function name: `fn get_queue_returns_exact_queueinfo_when_queue_exists()`

### Behavior: queue delete removes existing queue
Given: existing queue
When: `DELETE /queues/{name}`
Then: status `200`; subsequent `GET /queues/{name}` returns exact `404` payload

Error variant:
Given: nonexistent queue
When: same action
Then: status `404` and body equals `{"message":"queue no-such-queue not found"}`

Test function name: `fn delete_queue_returns_404_after_successful_deletion_when_queue_is_fetched_again()`

### Behavior: scheduled job creation accepts valid input
Given: valid JSON or YAML scheduled job body
When: `POST /scheduled-jobs`
Then: status `200`; body contains exact `id`, `name`, `cron`, summary state, and no unredacted secrets

Error variant:
Given: missing cron
When: same action
Then: status `400` and body equals `{"message":"cron is required"}`

Additional error variant:
Given: missing tasks
Then: status `400` and body equals `{"message":"tasks is required"}`

Additional error variant:
Given: invalid cron
Then: status `400` and body equals exact cron validation message

Additional error variant:
Given: unsupported content type
Then: status `400` and body equals `{"message":"unsupported content type"}`

Test function name: `fn create_scheduled_job_returns_exact_summary_when_input_is_valid()`

### Behavior: scheduled job list returns paginated summaries
Given: datastore containing scheduled jobs
When: `GET /scheduled-jobs?page=1&size=2`
Then: status `200`; body contains exact page metadata and exact summary items

Test function name: `fn list_scheduled_jobs_returns_exact_page_metadata_when_jobs_exist()`

### Behavior: scheduled job fetch returns exact job
Given: existing scheduled job
When: `GET /scheduled-jobs/{id}`
Then: status `200`; body fields equal stored values except redacted secrets

Error variant:
Given: nonexistent id
When: same action
Then: status `404` and body equals `{"message":"scheduled job not found"}`

Test function name: `fn get_scheduled_job_returns_exact_redacted_job_when_id_exists()`

### Behavior: scheduled job pause enforces active-only transition
Given: active scheduled job
When: `PUT /scheduled-jobs/{id}/pause`
Then: status `200` and body equals `{"status":"OK"}`; subsequent fetch shows `state="PAUSED"`

Error variant:
Given: paused scheduled job
When: same action
Then: status `400` and body equals `{"message":"scheduled job is not active"}`

Test function name: `fn pause_scheduled_job_returns_status_ok_when_job_is_active()`

### Behavior: scheduled job resume enforces paused-only transition
Given: paused scheduled job
When: `PUT /scheduled-jobs/{id}/resume`
Then: status `200` and body equals `{"status":"OK"}`; subsequent fetch shows `state="ACTIVE"`

Error variant:
Given: active scheduled job
When: same action
Then: status `400` and body equals `{"message":"scheduled job is not paused"}`

Test function name: `fn resume_scheduled_job_returns_status_ok_when_job_is_paused()`

### Behavior: scheduled job delete removes job
Given: existing scheduled job
When: `DELETE /scheduled-jobs/{id}`
Then: status `200` and body equals `{"status":"OK"}`; subsequent fetch returns `404`

Test function name: `fn delete_scheduled_job_returns_404_after_successful_delete_when_fetched_again()`

### Behavior: trigger list returns exact trigger views
Given: trigger datastore with known triggers
When: `GET /api/v1/triggers`
Then: status `200` and body equals exact array of trigger views

Test function name: `fn list_triggers_returns_exact_trigger_views_when_triggers_exist()`

### Behavior: trigger create returns exact created view
Given: valid JSON trigger body
When: `POST /api/v1/triggers`
Then: status `201`; body contains exact `name`, `enabled`, `event`, `action`, `metadata`, `version=1`, non-empty `id`

Error variant:
Given: body exceeds `MAX_BODY_BYTES`
When: same action
Then: status `400`; body equals `{"error":"ValidationFailed","message":"request body exceeds max size"}`

Additional error variant:
Given: unsupported content type
Then: status `400`; body equals `{"error":"UnsupportedContentType","message":"<content-type>"}`

Additional error variant:
Given: malformed JSON
Then: status `400`; body equals `{"error":"MalformedJson","message":"malformed json"}` or exact constant-backed message

Test function name: `fn create_trigger_returns_exact_created_view_when_request_is_valid()`

### Behavior: trigger fetch validates id and existence
Given: existing trigger id
When: `GET /api/v1/triggers/{id}`
Then: status `200` and body equals stored trigger view

Error variant:
Given: invalid id format
When: same action
Then: status `400`; body contains exact `error="InvalidIdFormat"`

Additional error variant:
Given: unknown id
Then: status `404`; body contains exact `error="TriggerNotFound"`

Test function name: `fn get_trigger_returns_exact_view_when_id_exists()`

### Behavior: trigger update enforces version and id contract
Given: existing trigger and matching request version/id
When: `PUT /api/v1/triggers/{id}`
Then: status `200`; body reflects updated fields and version increment

Error variant:
Given: stale version
When: same action
Then: status `409`; body equals `{"error":"VersionConflict","message":"<exact message>"}`

Additional error variant:
Given: path id and body id mismatch
Then: status `400`; body equals `{"error":"IdMismatch","message":"id mismatch","path_id":"...","body_id":"..."}`

Test function name: `fn update_trigger_returns_conflict_when_stale_version_is_supplied()`

### Behavior: trigger delete removes trigger
Given: existing trigger
When: `DELETE /api/v1/triggers/{id}`
Then: status `204` and empty body; subsequent fetch returns `404` exact error payload

Test function name: `fn delete_trigger_returns_no_content_and_removes_trigger_when_id_exists()`

### Behavior: user creation validates username and password
Given: valid username/password body
When: `POST /users`
Then: status `200` and datastore contains created user with bcrypt-hashed password, not plaintext

Error variant:
Given: missing username
When: same action
Then: status `400` and body equals `{"message":"username is required"}`

Additional error variant:
Given: missing password
Then: status `400` and body equals `{"message":"password is required"}`

Additional error variant:
Given: invalid username or short password
Then: status `400` and body equals exact `invalid username: ...` or `invalid password: ...` string

Test function name: `fn create_user_returns_exact_validation_message_when_credentials_are_invalid()`

---

## 6. Proptest Invariants

### Proptest: `parse_page`
- Invariant: any `Some(v)` with `v >= 1` returns `v`; any `None` or `v < 1` returns `1`
- Strategy: `Option<i64>` over full range
- Anti-invariant: none; function is total

### Proptest: `parse_size`
- Invariant: result is always in `1..=max`; valid in-range input is preserved; invalid/None becomes `default` then clamped
- Strategy: `(Option<i64>, default in 1..=100, max in 1..=200 where default<=max)`
- Anti-invariant: `max == 0` should be impossible by caller contract; unit test should document/prevent invalid harness setup

### Proptest: `PaginationQuery::from_raw`
- Invariant: numeric strings parse to exact `i64`; non-numeric strings become `None`; `q` is preserved exactly
- Strategy: arbitrary numeric/non-numeric strings and optional search string
- Anti-invariant: malformed integers never surface raw parse panics/errors

### Proptest: `WaitMode` deserialization
- Invariant: `true`, `"true"`, `"1"`, `"yes"`, `"blocking"` => `Blocking`; all other strings => `Detached`
- Strategy: booleans + arbitrary ASCII strings with mixed case
- Anti-invariant: deserialization never panics on unknown string inputs

### Proptest: `parse_content_type`
- Invariant: `application/json` with any case-normalizable parameter suffix (`; charset=utf-8`) is accepted after normalization; any other media type is rejected with `UnsupportedContentType(exact_type)`
- Strategy: valid/invalid content-type strings with optional parameters/whitespace/casing
- Anti-invariant: empty/malformed headers are rejected, not accepted silently as JSON

### Proptest: queue contract normalization (`QueueInfo` response assertions)
- Invariant: whenever queue exists, serialized response contains exact original name and non-negative integer counters
- Strategy: generate queue names and non-negative counts through broker test fixture
- Anti-invariant: nonexistent queue never serializes as zeroed `200` response if canonical contract is `404`

---

## 7. Fuzz Targets

### Fuzz Target: `yaml::from_slice` for job submission
- Input type: bytes
- Risk: parser panic, pathological YAML expansion, wrong error classification
- Corpus seeds: empty body, minimal valid job, 100-task example YAML, invalid indentation, anchors/aliases, huge scalar, non-UTF8 bytes

### Fuzz Target: `serde_json::from_slice::<Job>` via `POST /jobs`
- Input type: bytes
- Risk: malformed JSON handling, panic, inconsistent error payload
- Corpus seeds: `{}`, truncated JSON, deeply nested objects, wrong scalar types, oversized arrays

### Fuzz Target: `parse_create_body` for scheduled jobs
- Input type: bytes + content type selector
- Risk: YAML/JSON branch divergence and unsupported-content handling gaps
- Corpus seeds: valid JSON body, valid YAML body, missing cron, missing tasks, invalid cron, random bytes

### Fuzz Target: `decode_trigger_update_request`
- Input type: bytes
- Risk: malformed JSON, type confusion (`metadata` non-object, booleans/strings/nulls), panic-free field extraction
- Corpus seeds: empty object, missing required fields, wrong field types, nested metadata, huge metadata map

### Fuzz Target: `prepare_update`
- Input type: path id string + header value + body bytes
- Risk: inconsistent ordering of content-type/id/version validation
- Corpus seeds: valid request, stale version, body id mismatch, bad content type, bad id format

### Fuzz Target: live `/openapi.json` artifact parser
- Input type: checked-in JSON artifacts + live endpoint body
- Risk: malformed published spec or non-JSON response after refactor
- Corpus seeds: current artifacts, intentionally truncated artifact, invalid UTF-8 wrapper

### Fuzz Target: QA YAML files as executable job definitions
- Input type: bytes from `qa/*.yaml`
- Risk: published QA documents stop being valid jobs
- Corpus seeds: all files under `qa/`

---

## 8. Kani Harnesses

### Kani Harness: pagination bounds are total
- Property: `parse_size(input, default, max)` always returns value in `1..=max` for bounded valid harness inputs
- Bound: small bounded integers (e.g. `-3..=10`, `max 1..=10`)
- Rationale: tiny pure function with high mutation risk and broad API impact

### Kani Harness: scheduled state transition guards are complete
- Property: `validate_pause` accepts only `Active`; `validate_resume` accepts only `Paused`
- Bound: all `ScheduledJobState` enum variants
- Rationale: full state-machine exhaustiveness proof is cheap and valuable

### Kani Harness: trigger body size gate is exact
- Property: body lengths `<= MAX_BODY_BYTES` do not fail size gate; `> MAX_BODY_BYTES` always fail with `ValidationFailed(BODY_TOO_LARGE_MSG)`
- Bound: `0..=MAX_BODY_BYTES+1`
- Rationale: off-by-one bug here would be invisible to smoke tests

### Kani Harness: queue counters remain non-negative in serialized contract
- Property: `QueueInfo { size, subscribers, unacked }` serialized/deserialized contract never produces negative counts from supported broker paths
- Bound: broker fixture values in small integer domain
- Rationale: API consumers treat these as counts, not signed arbitrary integers

---

## 9. Mutation Testing Checkpoints

Critical mutations to survive:
- Change `/jobs` list response assertion from page object to array → caught by `list_jobs_returns_exact_page_metadata_when_pagination_query_is_applied`
- Remove YAML branch from `create_job_handler` → caught by `create_job_accepts_yaml_and_returns_same_semantic_contract_as_json`
- Change unsupported content type branch to return `200`/`500` → caught by exact `400` assertions on jobs and scheduled jobs
- Remove terminal-state guard in cancel → caught by `cancel_job_returns_exact_validation_message_when_job_is_terminal`
- Remove restart-state guard → caught by `restart_job_returns_exact_validation_message_when_job_is_not_restartable`
- Return zeroed `QueueInfo` for nonexistent queue instead of `404` → caught by `get_queue_returns_exact_404_payload_when_queue_does_not_exist`
- Make delete queue a no-op → caught by `delete_queue_returns_404_after_successful_deletion_when_queue_is_fetched_again`
- Downgrade trigger stale version from `409` to `400/200` → caught by `update_trigger_returns_conflict_when_stale_version_is_supplied`
- Strip `path_id`/`body_id` from id-mismatch payload → caught by exact id-mismatch test
- Change `/openapi.json` test to only assert `info` exists → mutant survives today; kill with full JSON equality test

Threshold: **minimum 90% mutation kill rate** on `crates/twerk-web/src/api/**` and OpenAPI artifact parity helpers.

---

## 10. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| health happy | healthy ds + broker | `200` + exact `{"status":"UP","version":"..."}` | integration |
| health down | failing ds or broker | `503` + exact `{"status":"DOWN","version":"..."}` | integration |
| openapi happy | live router | exact JSON equality with `ApiDoc::openapi()` | integration |
| artifact sync | checked-in files | exact JSON equality | static |
| jobs create json | valid JSON | `200` + exact contract payload | integration |
| jobs create yaml | valid YAML | `200` + exact semantic payload | integration |
| jobs content-type error | unsupported media type | `400` + exact `unsupported content type` message | integration |
| jobs malformed body | invalid JSON/YAML | `400` + exact parse message | integration |
| jobs validation error | invalid job | `400` + exact validation message | integration |
| jobs list happy | page=1 size=2 | exact page metadata + exact items | integration |
| jobs list invalid page | page=abc size=0 | defaults/clamps exactly applied | unit + integration |
| jobs get happy | existing id | exact job/task payload | integration |
| jobs get missing | absent id | `404` + exact message | integration |
| cancel happy | pending/running | `200` + `{"status":"OK"}` | integration |
| cancel invalid state | completed/failed/cancelled | `400` + exact message | integration |
| restart happy | failed/cancelled | `200` + `{"status":"OK"}` | integration |
| restart invalid state | pending/running/completed | `400` + exact message | integration |
| queue list empty | no queues | `200` + `[]` | integration |
| queue list populated | existing queues | exact `QueueInfo[]` | integration |
| queue get happy | existing queue | exact `QueueInfo` | integration |
| queue get missing | absent queue | `404` + exact message | integration |
| queue delete happy | existing queue | `200`, then follow-up GET exact `404` | integration |
| queue delete missing | absent queue | `404` + exact message | integration |
| scheduled create happy | valid JSON/YAML | `200` + exact summary | integration |
| scheduled create missing cron | invalid body | `400` + `cron is required` | integration |
| scheduled create invalid cron | invalid body | `400` + exact cron message | integration |
| scheduled pause happy | active job | `200` then fetch `state="PAUSED"` | integration |
| scheduled pause invalid | paused job | `400` + exact message | integration |
| scheduled resume happy | paused job | `200` then fetch `state="ACTIVE"` | integration |
| scheduled resume invalid | active job | `400` + exact message | integration |
| trigger create happy | valid JSON | `201` + exact trigger view | integration |
| trigger bad content type | invalid header | `400` + exact error payload | integration |
| trigger malformed json | invalid bytes | `400` + exact error payload | integration |
| trigger stale version | version=0/stale version | `409` + exact error payload | integration |
| trigger id mismatch | path/body mismatch | `400` + exact payload incl. ids | integration |
| user create happy | valid username/password | `200` + persisted bcrypt hash | integration |
| user invalid password | short | `400` + exact message | integration |
| invariant | any pagination input | bounds/clamp properties hold | proptest |

---

## 11. Explicit assertions to prefer over weak smoke assertions

Replace weak assertions like `body.is_object()` / `body.is_array()` with these:

- **OpenAPI**
  - Prefer: whole parsed JSON equality with generated spec/artifacts.
  - Also assert exact `info.title == "Twerk API"`, exact `info.version == env!("CARGO_PKG_VERSION")`, and required paths exist with exact response codes.

- **Queue list**
  - Prefer: `assert_eq!(body, json!([{"name":"x-jobs","size":1,"subscribers":2,"unacked":0}]))`
  - Or compare deserialized `Vec<QueueInfo>` to exact expected structs.

- **Queue get**
  - Prefer exact field assertions:
    - `name == "x-jobs"`
    - `size == 1`
    - `subscribers == 2`
    - `unacked == 0`

- **Job list**
  - Prefer exact page object assertions:
    - `page == 1`
    - `size == 2`
    - `total_items == 3`
    - `total_pages == 2`
    - `items[0].name == "page-job-a"`

- **Cancel/restart**
  - Prefer exact payload: `{"status":"OK"}`
  - Negative: exact `{"message":"job cannot be restarted"}` or `{"message":"job cannot be cancelled in its current state"}`

- **Scheduled lifecycle**
  - Prefer follow-up GET assertions on exact state transitions (`ACTIVE -> PAUSED -> ACTIVE`) rather than only `200` status.

- **Triggers**
  - Prefer exact error envelope:
    - `status == 409`
    - `body["error"] == "VersionConflict"`
    - `body["message"] == "stale version supplied"` (or exact canonical text)

- **Users**
  - Prefer datastore assertion that created user exists and `password_hash != input_password`.

Never accept:
- `assert!(body.is_object())`
- `assert!(body.is_array())`
- `assert!(response.status().is_success())`
- `assert!(result.is_ok())`
- `assert!(body["id"].is_string())` without asserting other contract fields

---

## 12. Static / workflow plan

### Workflow: `openapi-sync.yml`
Must do:
1. generate spec to temp path
2. compare temp vs `crates/twerk-web/openapi.json`
3. compare temp vs `docs/openapi.json`
4. compare checked-in artifacts to each other
5. print unified diff on failure

### Workflow: API contract checks
Add a new workflow for `twerk-web` that runs at least:
- contract parity tests
- live `/openapi.json` parity tests
- queue contract tests
- selected trigger/scheduled/job negative-path tests

### Workflow: mutation gate
Optional first, required before approval hardening:
- run `cargo-mutants` on `crates/twerk-web/src/api` subset
- enforce `>= 90%` kill rate

---

## Open Questions

1. Canonical product decision needed: should queue GET/DELETE on nonexistent queue be `404` for all broker implementations? The current API tests disagree.
2. Canonical product decision needed: should detached `POST /jobs` return `JobSummary` or full `Job`? Runtime and spec currently disagree.
3. Canonical product decision needed: should cancel/restart return `{status:"OK"}` or the updated `Job` resource? Runtime and spec currently disagree.
4. Canonical product decision needed: should `/openapi.json` itself be described inside the OpenAPI document, or only tested as the transport for the spec artifact?
5. If RabbitMQ `management_url` is absent, should queue endpoints degrade to empty/zero values, or should they fail as unsupported? Contract tests must match the chosen answer.
