---
bead_id: twerk-r4l
bead_title: "action: Implement trigger update endpoint (PUT /api/v1/triggers/{id})"
phase: state-1.5-test-plan
updated_at: 2026-04-13T13:30:00Z
plan_revision: 2
replaces: test-plan.md (rejected)
---

# Test Plan: Trigger Update Endpoint (`PUT /api/v1/triggers/{id}`)

## Summary
- Behaviors identified: 33
- Trophy allocation: **16 unit / 15 integration / 2 e2e** (+ 5 static gates)
- Unit floor check: 16 planned unit scenarios ≥ required minimum 15 (5 × 3 public functions)
- Proptest invariants: 10 (covering all pure multi-input functions)
- Fuzz targets: 6
- Kani harness specs: 6
- Mutation testing threshold: **≥90% kill rate** (`cargo mutants`)

Message-contract decision (to prevent wildcard assertions): test suite MUST assert exact canonical messages below:
- `ValidationFailed("name must be non-empty after trim")`
- `ValidationFailed("event must be non-empty after trim")`
- `ValidationFailed("action must be non-empty after trim")`
- `ValidationFailed("metadata key must be non-empty ASCII")`
- `ValidationFailed("updated_at cannot move backwards")`
- `MalformedJson("malformed JSON body")`
- `Serialization("failed to serialize response")`
- `Persistence("internal persistence failure")`

If implementation uses different text, normalize to these constants before returning API errors.

## 1) Behavior Inventory

1. Handler returns `400 InvalidIdFormat(path_id)` when path id cannot parse.
2. Handler returns `400 UnsupportedContentType(actual)` when content type is not `application/json`.
3. Handler returns `400 MalformedJson("malformed JSON body")` when body is not deserializable JSON.
4. Handler returns `400 ValidationFailed("name must be non-empty after trim")` when name is blank-after-trim.
5. Handler returns `400 ValidationFailed("event must be non-empty after trim")` when event is blank-after-trim.
6. Handler returns `400 ValidationFailed("action must be non-empty after trim")` when action is blank-after-trim.
7. Handler returns `400 IdMismatch { path_id, body_id }` when optional body id differs from path id.
8. Handler returns `400 ValidationFailed("metadata key must be non-empty ASCII")` when any metadata key is empty or non-ASCII.
9. Handler returns `404 TriggerNotFound(id)` when target id does not exist.
10. Handler returns `409 VersionConflict(msg)` when optimistic concurrency is enabled and stale version is supplied.
11. Handler returns `500 Persistence("internal persistence failure")` when datastore read/update fails.
12. Handler returns `500 Serialization("failed to serialize response")` when response encoding fails.
13. Handler updates exactly one trigger on success.
14. Handler preserves immutable `id` on success.
15. Handler preserves immutable `created_at` on success.
16. Handler sets `updated_at` to server `now_utc` and `updated_at >= prior updated_at`.
17. Handler response `TriggerView` exactly matches committed trigger state.
18. Handler applies PUT idempotently for mutable fields (same request twice => same persisted mutable state).
19. Handler commits atomically: failed update leaves persisted trigger unchanged.
20. Handler maps all client contract errors to HTTP 400.
21. Handler maps `TriggerNotFound` to 404.
22. Handler maps `VersionConflict` to 409 when feature enabled.
23. Handler maps `Persistence` and `Serialization` to 500.
24. `validate_trigger_update` returns `Ok(TriggerId(path_id))` for valid input.
25. `validate_trigger_update` returns exact field-specific validation errors for invalid required fields.
26. `validate_trigger_update` returns `IdMismatch` for body/path id mismatch.
27. `validate_trigger_update` enforces metadata key ASCII/non-empty invariant.
28. `apply_trigger_update` returns trigger with mutable fields replaced from request.
29. `apply_trigger_update` preserves immutable fields (`id`, `created_at`).
30. `apply_trigger_update` accepts equality boundary `now_utc == current.updated_at` and sets updated_at exactly.
31. `apply_trigger_update` rejects backward time (`now_utc < current.updated_at`) with exact validation error.
32. `apply_trigger_update` rejects required-field invariant violations with exact validation errors.
33. Datastore update closure is atomic: closure `Err` causes no persisted mutation.

## 2) Testing Trophy Allocation

| Behavior IDs | Layer | Count | Rationale |
|---|---:|---:|---|
| 24–32 (+ boundaries below) | Unit (`#[cfg(test)]`) | 16 | Pure deterministic logic in `validate_trigger_update` + `apply_trigger_update`; best for exhaustive boundary/error enumeration. |
| 1–23, 33 | Integration (`/tests`) | 15 | Real router + request decoding + datastore abstraction + HTTP/status mapping + atomic persistence behavior. |
| End-user HTTP workflow (happy + failure) | E2E | 2 | Black-box assurance from client perspective. |
| fmt/clippy/check/deny/warnings-as-errors | Static | 5 gates | Compile/lint/security guarantees at lowest cost. |

Distribution is integration-heavy, unit-second, minimal e2e, aligned with Testing Trophy.

## 3) BDD Scenarios (Given-When-Then)

### Public function: `update_trigger_handler`

### Behavior: rejects invalid path id
Test function: `fn update_trigger_handler_returns_400_invalid_id_format_when_path_id_is_unparseable()`
Given: Router is mounted; datastore contains trigger `trg_abc`; request body is valid JSON; path id is `"not-valid"`.
When: PUT `/api/v1/triggers/not-valid` with `Content-Type: application/json`.
Then: status is `400` and body equals exact JSON `{ "error": "InvalidIdFormat", "message": "not-valid" }`.

### Behavior: rejects unsupported content type
Test function: `fn update_trigger_handler_returns_400_unsupported_content_type_when_content_type_is_text_plain()`
Given: Existing trigger `trg_abc`; body bytes are valid JSON.
When: PUT with `Content-Type: text/plain`.
Then: status `400`; body exactly `{ "error": "UnsupportedContentType", "message": "text/plain" }`.

### Behavior: rejects malformed JSON with exact payload
Test function: `fn update_trigger_handler_returns_400_malformed_json_when_body_is_truncated_json()`
Given: Existing trigger `trg_abc`; content type `application/json`; body bytes `{\"name\":`.
When: Request is executed.
Then: status `400`; body exactly `{ "error": "MalformedJson", "message": "malformed JSON body" }`.

### Behavior: rejects empty JSON object as validation failure
Test function: `fn update_trigger_handler_returns_400_validation_failed_when_body_is_empty_object()`
Given: Existing trigger `trg_abc`; content type json; body `{}`.
When: Request is executed.
Then: status `400`; body error `ValidationFailed` with exact first failing message constant.

### Behavior: rejects body id mismatch
Test function: `fn update_trigger_handler_returns_400_id_mismatch_when_body_id_differs_from_path_id()`
Given: Path id `trg_path`; body id `trg_body`; all other fields valid.
When: PUT request is executed.
Then: status `400`; body exactly `{ "error": "IdMismatch", "path_id": "trg_path", "body_id": "trg_body" }`.

### Behavior: returns not found
Test function: `fn update_trigger_handler_returns_404_trigger_not_found_when_trigger_missing()`
Given: Datastore has no trigger `trg_missing`; request is valid.
When: PUT `/api/v1/triggers/trg_missing`.
Then: status `404`; body exactly `{ "error": "TriggerNotFound", "message": "trg_missing" }`.

### Behavior: returns version conflict (feature enabled)
Test function: `fn update_trigger_handler_returns_409_version_conflict_when_stale_version_supplied()`
Given: Concurrency guard enabled; trigger exists at version 3; request carries stale version 2.
When: PUT executed.
Then: status `409`; body error is exactly `VersionConflict` with canonical conflict message.

### Behavior: maps persistence errors to sanitized 500
Test function: `fn update_trigger_handler_returns_500_persistence_when_datastore_update_fails()`
Given: Datastore stub deterministically returns `TriggerUpdateError::Persistence("db timeout details")` on update.
When: Valid PUT is executed.
Then: status `500`; body exactly `{ "error": "Persistence", "message": "internal persistence failure" }`.

### Behavior: maps serialization errors to exact 500
Test function: `fn update_trigger_handler_returns_500_serialization_when_response_encoding_fails()`
Given: Serialization path is forced to fail (e.g., test-only serializer failure hook enabled).
When: Valid PUT is executed.
Then: status `500`; body exactly `{ "error": "Serialization", "message": "failed to serialize response" }`.

### Behavior: successful update projects exact committed state
Test function: `fn update_trigger_handler_returns_200_and_trigger_view_equal_to_committed_trigger()`
Given: Trigger exists with known snapshot; valid update request with known values.
When: PUT is executed once.
Then: status `200`; JSON response fields exactly equal datastore post-commit trigger (`id/name/enabled/event/condition/action/metadata/created_at/updated_at`).

### Behavior: idempotent repeated PUT
Test function: `fn update_trigger_handler_keeps_same_mutable_state_when_same_request_applied_twice()`
Given: Existing trigger and deterministic clock sequence.
When: Same valid PUT is executed twice.
Then: `name/enabled/event/condition/action/metadata` after call 2 exactly equal after call 1; exactly one trigger row with same id exists.

### Behavior: atomic rollback on failure
Test function: `fn update_trigger_handler_preserves_preupdate_state_when_modify_closure_returns_error()`
Given: Existing trigger snapshot S0; datastore update path configured so modify closure returns `ValidationFailed("action must be non-empty after trim")`.
When: PUT is executed.
Then: status `400`; subsequent `get_trigger_by_id` equals S0 byte-for-byte.

### Transport boundary: minimum path-id length accepted
Test function: `fn update_trigger_handler_accepts_min_path_id_length_when_id_length_equals_min()`
Given: `TRIGGER_ID_MIN_LEN` constant and valid id of exactly that length.
When: Valid PUT executed.
Then: status `200` with exact expected body.

### Transport boundary: maximum path-id length accepted
Test function: `fn update_trigger_handler_accepts_max_path_id_length_when_id_length_equals_max()`
Given: valid id length `TRIGGER_ID_MAX_LEN`.
When: Valid PUT executed.
Then: status `200` and exact expected projection.

### Transport boundary: one-above-max path-id rejected
Test function: `fn update_trigger_handler_returns_400_invalid_id_format_when_path_id_length_exceeds_max_by_one()`
Given: id length `TRIGGER_ID_MAX_LEN + 1`.
When: Valid PUT executed.
Then: status `400`; exact error `InvalidIdFormat(<exact_overlong_id>)`.

### Transport boundary: maximum body size accepted
Test function: `fn update_trigger_handler_accepts_request_when_body_size_equals_max_bytes()`
Given: payload serialized size exactly `MAX_BODY_BYTES` and semantically valid.
When: PUT executed.
Then: status `200` and exact committed projection.

### Transport boundary: one-above-max body rejected
Test function: `fn update_trigger_handler_returns_400_validation_failed_when_body_size_exceeds_max_bytes_by_one()`
Given: payload size `MAX_BODY_BYTES + 1`.
When: PUT executed.
Then: status `400`; exact error body `{ "error": "ValidationFailed", "message": "request body exceeds max size" }`.

---

### Public function: `validate_trigger_update(path_id, req)`

### Behavior: valid request returns exact TriggerId
Test function: `fn validate_trigger_update_returns_ok_trigger_id_when_inputs_are_valid()`
Given: valid `path_id`; `name/event/action` non-empty after trim; optional `req.id` absent or equal.
When: `validate_trigger_update(path_id, &req)`.
Then: exactly `Ok(TriggerId(path_id.to_string()))`.

### Error: invalid id format
Test function: `fn validate_trigger_update_returns_invalid_id_format_when_path_id_is_invalid()`
Given: invalid path id string `bad$id`.
When: validator called.
Then: exactly `Err(TriggerUpdateError::InvalidIdFormat("bad$id".into()))`.

### Error: name blank-after-trim
Test function: `fn validate_trigger_update_returns_exact_name_validation_error_when_name_is_blank()`
Given: `name = "   "`; other fields valid.
When: validator called.
Then: exactly `Err(TriggerUpdateError::ValidationFailed("name must be non-empty after trim".into()))`.

### Error: event blank-after-trim
Test function: `fn validate_trigger_update_returns_exact_event_validation_error_when_event_is_blank()`
Given: `event = "\n\t"`; other fields valid.
When: validator called.
Then: exactly `Err(TriggerUpdateError::ValidationFailed("event must be non-empty after trim".into()))`.

### Error: action blank-after-trim
Test function: `fn validate_trigger_update_returns_exact_action_validation_error_when_action_is_blank()`
Given: `action = " "`; other fields valid.
When: validator called.
Then: exactly `Err(TriggerUpdateError::ValidationFailed("action must be non-empty after trim".into()))`.

### Error: metadata key invalid
Test function: `fn validate_trigger_update_returns_exact_metadata_validation_error_when_metadata_key_is_non_ascii_or_empty()`
Given: metadata includes key `""` (or `"ключ"`); other fields valid.
When: validator called.
Then: exactly `Err(TriggerUpdateError::ValidationFailed("metadata key must be non-empty ASCII".into()))`.

### Error: body/path id mismatch
Test function: `fn validate_trigger_update_returns_id_mismatch_when_body_id_differs()`
Given: `path_id="trg_1"`, `req.id=Some("trg_2")`.
When: validator called.
Then: exactly `Err(TriggerUpdateError::IdMismatch { path_id: "trg_1".into(), body_id: "trg_2".into() })`.

### Boundary: minimum required-field length accepted
Test function: `fn validate_trigger_update_accepts_required_fields_when_length_equals_min_one()`
Given: `name/event/action` are single-character strings.
When: validator called.
Then: returns exact `Ok(TriggerId(path_id))`.

### Boundary: maximum required-field length accepted
Test function: `fn validate_trigger_update_accepts_required_fields_when_length_equals_max()`
Given: each required field length exactly `TRIGGER_FIELD_MAX_LEN`.
When: validator called.
Then: exact `Ok(TriggerId(path_id))`.

### Boundary: one-above-max required-field rejected
Test function: `fn validate_trigger_update_rejects_required_field_when_length_exceeds_max_by_one()`
Given: `name.len() == TRIGGER_FIELD_MAX_LEN + 1`; other fields valid.
When: validator called.
Then: exact `Err(ValidationFailed("name exceeds max length"))`.

### Boundary: id overflow class rejected
Test function: `fn validate_trigger_update_returns_invalid_id_format_when_id_length_exceeds_max()`
Given: `path_id` length `TRIGGER_ID_MAX_LEN + 1`.
When: validator called.
Then: exact `Err(InvalidIdFormat(overlong_id))`.

---

### Public function: `apply_trigger_update(current, req, now_utc)`

### Behavior: mutable projection replaced
Test function: `fn apply_trigger_update_returns_trigger_with_exact_mutable_projection_from_request()`
Given: current trigger C0 and valid request R.
When: `apply_trigger_update(C0, R, now)`.
Then: `name/enabled/event/condition/action/metadata` equal exact normalized request values.

### Behavior: immutable fields preserved
Test function: `fn apply_trigger_update_preserves_id_and_created_at_when_request_valid()`
Given: current trigger with known `id` and `created_at`.
When: apply called with valid request.
Then: returned `id == original.id` and `created_at == original.created_at` exactly.

### Boundary killer: equality timestamp accepted
Test function: `fn apply_trigger_update_sets_updated_at_when_now_equals_previous_updated_at()`
Given: `current.updated_at = t0` and `now_utc = t0`.
When: apply called.
Then: exact `Ok(trigger)` with `trigger.updated_at == t0`.

### Boundary: monotonic forward timestamp accepted
Test function: `fn apply_trigger_update_sets_updated_at_to_now_when_now_is_after_previous_updated_at()`
Given: `current.updated_at = t0`, `now_utc = t0 + 1s`.
When: apply called.
Then: `updated_at == now_utc` exactly.

### Error: backward timestamp rejected
Test function: `fn apply_trigger_update_returns_exact_updated_at_validation_error_when_now_is_before_previous()`
Given: `current.updated_at = t1`, `now_utc = t1 - 1ns`.
When: apply called.
Then: exactly `Err(TriggerUpdateError::ValidationFailed("updated_at cannot move backwards".into()))`.

### Error: blank name rejected
Test function: `fn apply_trigger_update_returns_exact_name_validation_error_when_name_blank_after_trim()`
Given: request `name="   "`.
When: apply called.
Then: exact `Err(ValidationFailed("name must be non-empty after trim"))`.

### Error: blank event rejected
Test function: `fn apply_trigger_update_returns_exact_event_validation_error_when_event_blank_after_trim()`
Given: request `event="\t"`.
When: apply called.
Then: exact `Err(ValidationFailed("event must be non-empty after trim"))`.

### Error: blank action rejected
Test function: `fn apply_trigger_update_returns_exact_action_validation_error_when_action_blank_after_trim()`
Given: request `action=" "`.
When: apply called.
Then: exact `Err(ValidationFailed("action must be non-empty after trim"))`.

### Boundary: max required field lengths accepted
Test function: `fn apply_trigger_update_accepts_required_fields_when_lengths_equal_max()`
Given: request required fields each length `TRIGGER_FIELD_MAX_LEN`.
When: apply called.
Then: exact `Ok` projection with those values persisted in returned Trigger.

### Boundary: one-above-max rejected
Test function: `fn apply_trigger_update_returns_validation_failed_when_required_field_exceeds_max_by_one()`
Given: `event.len() == TRIGGER_FIELD_MAX_LEN + 1`.
When: apply called.
Then: exact `Err(ValidationFailed("event exceeds max length"))`.

## 4) Proptest Invariants

### `validate_trigger_update(path_id, req)`
1. **Valid-domain success**: all generated valid inputs return `Ok(TriggerId(path_id))` exactly.
   - Strategy: ids from parser-valid generator; required fields from `\S.*` and max length; optional `req.id` in `{None, Some(path_id)}`.
2. **Id mismatch always fails deterministically**: if `req.id=Some(x)` and `x!=path_id`, result is exactly `Err(IdMismatch { path_id, body_id: x })`.
3. **Blank-after-trim rejection**: any case where one required field trims to empty must return its exact field-specific `ValidationFailed` constant.
4. **Metadata key safety**: any metadata map containing at least one invalid key always returns exact metadata validation error.
5. **Boundary stability**: values at `MIN/MAX` limits are accepted; `MAX+1` always rejected with exact max-length message.

### `apply_trigger_update(current, req, now_utc)`
6. **Immutable preservation**: for any valid `(current, req, now>=current.updated_at)`, output keeps exact `id` and `created_at`.
7. **Projection correctness**: output mutable fields exactly equal normalized request projection.
8. **Timestamp equality invariant**: when `now == current.updated_at`, output is `Ok` and `updated_at == now`.
9. **Timestamp anti-invariant**: when `now < current.updated_at`, output is exact backward-time validation error.
10. **Length boundary invariant**: required fields at `MAX` accepted; at `MAX+1` rejected with exact field max-length message.

## 5) Fuzz Targets

### Fuzz target 1: TriggerId parser boundary
- Function boundary: path-id parsing used by validator/handler.
- Input type: arbitrary bytes -> UTF-8 lossy string.
- Risk class: panic, pathological parse complexity, acceptance of malformed ids.
- Corpus seeds: empty, whitespace, min valid, max valid, max+1, unicode confusables, embedded NUL.

### Fuzz target 2: `serde_json::from_slice::<TriggerUpdateRequest>`
- Input type: arbitrary bytes.
- Risk class: panic/OOM/deep recursion/malformed token handling.
- Corpus seeds: `{}`, minimal valid object, truncated JSON, duplicated keys, huge string fields.

### Fuzz target 3: metadata-key validation
- Input type: generated JSON objects for `metadata` map.
- Risk class: invalid UTF-8 normalization edge acceptance, empty-key bypass.
- Corpus seeds: empty key, non-ASCII key, long ASCII key, many-key map near limit.

### Fuzz target 4: full handler request envelope
- Input type: synthesized HTTP request bytes (path + headers + body).
- Risk class: route/header/body parser panic, content-type bypass bugs.
- Corpus seeds: valid PUT, wrong method, missing content-type, oversized body, invalid path segments.

### Fuzz target 5: error serialization envelope
- Input type: arbitrary `TriggerUpdateError` variant payload text.
- Risk class: serialization panic or accidental leaking of internal detail format.
- Corpus seeds: very long strings, escape-heavy strings, unicode separators, control chars.

### Fuzz target 6: idempotent replay stream
- Input type: sequence of repeated valid/invalid PUT payloads for same id.
- Risk class: state drift across repeated application and rollback paths.
- Corpus seeds: same-valid repeated N times, alternating valid+failing update, boundary-size payload replay.

## 6) Kani Harness Specifications

### Harness: immutable preservation proof
- Property: `apply_trigger_update` cannot mutate `id` or `created_at` for valid inputs.
- Bound: string lengths ≤ 8, metadata entries ≤ 3.
- Rationale: critical domain immutability.

### Harness: equality timestamp proof (mutation killer)
- Property: if `now == current.updated_at`, function succeeds and `updated_at == now`.
- Bound: bounded timestamp domain over equal pairs.
- Rationale: kills `>=` ↔ `>` operator mutation.

### Harness: backward timestamp rejection proof
- Property: if `now < current.updated_at`, output is exact backward-time validation error.
- Bound: bounded ordered timestamp triples.
- Rationale: guards monotonicity safety.

### Harness: id mismatch totality
- Property: all bounded `path_id != body_id` cases return exact `IdMismatch`.
- Bound: id lengths ≤ 8.
- Rationale: prevent cross-resource overwrite.

### Harness: HTTP mapping partition exactness
- Property: each error variant maps to exactly one contract status bucket: 400/404/409/500.
- Bound: exhaustive enum variants; bounded payload strings.
- Rationale: enforce mapping completeness.

### Harness: atomic update model
- Property: modeled datastore state unchanged when modify closure returns `Err`.
- Bound: map size ≤ 2 entities.
- Rationale: machine-check atomicity contract.

## 7) Mutation Testing Checkpoints

Run: `cargo mutants --package <trigger-package>`
Threshold: **kill rate ≥90% required**.

| Mutation | Expected killer test |
|---|---|
| Remove id parse failure branch | `update_trigger_handler_returns_400_invalid_id_format_when_path_id_is_unparseable` |
| Accept non-json content type | `update_trigger_handler_returns_400_unsupported_content_type_when_content_type_is_text_plain` |
| Replace malformed-json message with wildcard | `update_trigger_handler_returns_400_malformed_json_when_body_is_truncated_json` |
| Delete IdMismatch branch | `validate_trigger_update_returns_id_mismatch_when_body_id_differs` |
| Swap trim check with raw-empty check | field-specific blank-after-trim tests for name/event/action (validator + apply) |
| Change `now >= prev` to `now > prev` | `apply_trigger_update_sets_updated_at_when_now_equals_previous_updated_at` |
| Allow id mutation during apply | `apply_trigger_update_preserves_id_and_created_at_when_request_valid` |
| Drop rollback on closure error | `update_trigger_handler_preserves_preupdate_state_when_modify_closure_returns_error` |
| Map TriggerNotFound to wrong status | `update_trigger_handler_returns_404_trigger_not_found_when_trigger_missing` |
| Map VersionConflict to wrong status | `update_trigger_handler_returns_409_version_conflict_when_stale_version_supplied` |
| Leak raw persistence message | `update_trigger_handler_returns_500_persistence_when_datastore_update_fails` |
| Swallow serialization error | `update_trigger_handler_returns_500_serialization_when_response_encoding_fails` |
| Corrupt response projection field | `update_trigger_handler_returns_200_and_trigger_view_equal_to_committed_trigger` |

## 8) Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| happy path validate | valid ids + valid fields | `Ok(TriggerId(exact_path_id))` | unit |
| invalid path id | malformed / max+1 id | `Err(InvalidIdFormat(exact_input))` | unit |
| name blank | `name.trim()==""` | `Err(ValidationFailed("name must be non-empty after trim"))` | unit |
| event blank | `event.trim()==""` | `Err(ValidationFailed("event must be non-empty after trim"))` | unit |
| action blank | `action.trim()==""` | `Err(ValidationFailed("action must be non-empty after trim"))` | unit |
| metadata invalid | empty/non-ASCII key | `Err(ValidationFailed("metadata key must be non-empty ASCII"))` | unit |
| id mismatch | `req.id != path_id` | `Err(IdMismatch{exact ids})` | unit |
| required min boundary | len=1 | `Ok(TriggerId(...))` | unit |
| required max boundary | len=MAX | `Ok(TriggerId(...))` | unit |
| required max+1 | len=MAX+1 | `Err(ValidationFailed("<field> exceeds max length"))` | unit |
| apply happy path | valid current/request/now | `Ok(Trigger{exact projection})` | unit |
| apply equality boundary | `now==prev_updated_at` | `Ok` and `updated_at==now` | unit |
| apply backward time | `now<prev_updated_at` | `Err(ValidationFailed("updated_at cannot move backwards"))` | unit |
| HTTP unsupported media | `Content-Type=text/plain` | `400 + UnsupportedContentType("text/plain")` | integration |
| HTTP malformed json | truncated bytes | `400 + MalformedJson("malformed JSON body")` | integration |
| HTTP not found | missing id | `404 + TriggerNotFound(exact id)` | integration |
| HTTP persistence | datastore failure | `500 + Persistence("internal persistence failure")` | integration |
| HTTP serialization | forced encoder fail | `500 + Serialization("failed to serialize response")` | integration |
| HTTP id mismatch | body.id ≠ path.id | `400 + IdMismatch{exact ids}` | integration |
| HTTP success | existing id + valid request | `200 + exact TriggerView` | integration/e2e |
| idempotency replay | same valid request twice | identical persisted mutable state | integration/e2e |
| atomic rollback | closure returns Err | persisted state equals preimage | integration |

## Static Analysis Gates

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check --all-targets --all-features`
4. `cargo deny check`
5. `RUSTFLAGS='-D warnings' cargo test --all-targets`

## Exit-Criteria Compliance Checklist

- [x] Every public API function has BDD scenarios (`update_trigger_handler`, `validate_trigger_update`, `apply_trigger_update`).
- [x] Every `TriggerUpdateError` variant has explicit scenario coverage.
- [x] No scenario uses only `is_ok()` / `is_err()` assertions.
- [x] Pure multi-input functions have proptest invariants.
- [x] Parsing/deserialization/user-input boundaries have fuzz targets.
- [x] Mutation threshold (≥90%) stated with mutation-to-test mapping.
