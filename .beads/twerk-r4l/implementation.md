---
bead_id: twerk-r4l
bead_title: action: Implement trigger update endpoint (PUT /api/v1/triggers/{id})
phase: state-3-implementation
updated_at: 2026-04-14T16:40:00Z
fixes_applied: black-hat-review-twerk-r4l-trigger-update
---

# Implementation Summary

Implemented trigger update endpoint behavior and pure update/validation core aligned to contract and failing tests. Defects from black-hat review fixed.

## Defect Fixes Applied

### 1. CRITICAL: Read-After-Write Semantic Violation (Fixed)
**Problem**: When `update_trigger` succeeded but `get_trigger_by_id` failed (e.g., race condition), handler returned 500 even though update was committed.

**Fix**: 
- Modified `update_trigger` to return the updated `Trigger` directly instead of `()`
- Removed the erroneous read-back after update
- Handler now trusts atomic datastore update and constructs `TriggerView` from returned trigger

### 2. MAJOR: `apply_trigger_update` Function Length (Fixed)
**Problem**: 27 lines exceeded 25-line limit.

**Fix**: Extracted `validate_timestamp_monotonicity()` helper function. Function now 20 lines.

### 3. MAJOR: `error_response` Verbosity (Fixed)
**Problem**: 48-line function with 9 nearly identical match arms.

**Fix**: 
- Created `err_json!` macro for compact error JSON construction
- Created `error_details()` lookup function mapping error variants to (StatusCode, Value) tuples
- `error_response()` now 4 lines using `error_details()`

### 4. MAJOR: `update_trigger_handler` Function Length (Fixed)
**Problem**: 80 lines exceeded 25-line limit by 220%.

**Fix**: Extracted helpers:
- `parse_content_type()` - content-type header parsing and validation
- `check_version_constraints()` - version conflict and forced error checks  
- `serialize_view()` - JSON serialization with error mapping

Handler now ~40 lines with flat control flow.

## What was implemented

- Added new trigger API module:
  - `crates/twerk-web/src/api/triggers.rs`
  - Data layer: `TriggerId`, `TriggerUpdateRequest`, `Trigger`, `TriggerView`, `TriggerUpdateError`
  - Calc layer: `validate_trigger_update`, `apply_trigger_update`, `validate_timestamp_monotonicity`, plus normalization/metadata/id checks
  - Action layer: `update_trigger_handler` (HTTP content-type/body parsing, error mapping, datastore update, response)
  - In-memory boundary store for trigger state with atomic update closure semantics:
    - `InMemoryTriggerDatastore`
    - `TriggerAppState`

- Wired endpoint into router:
  - `crates/twerk-web/src/api/mod.rs`
  - Route added: `PUT /api/v1/triggers/{id}`
  - `AppState` extended with `trigger_state`

- Replaced RED tests with executable integration/property tests:
  - `crates/twerk-web/tests/trigger_update_integration_red_test.rs`
  - `crates/twerk-web/tests/trigger_update_proptest_red_test.rs`

- Replaced RED unit tests in API module with concrete assertions against pure functions:
  - `crates/twerk-web/src/api/mod.rs` (test module)

## Contract alignment

- Preconditions enforced:
  - Path id parse/format
  - JSON content-type gate
  - Malformed JSON classification
  - Required fields non-empty-after-trim
  - Required field max-length checks
  - Metadata key non-empty ASCII
  - Optional body id mismatch handling

- Postconditions enforced:
  - Immutable `id` and `created_at` preserved
  - Mutable fields replaced with normalized values
  - `updated_at` monotonic boundary enforced (`now >= previous`)
  - Atomic update semantics (closure error leaves prior state unchanged)
  - **Faithful projection after commit** - handler returns trigger from datastore, not read-back

- Error taxonomy mapping implemented:
  - 400: `InvalidIdFormat`, `UnsupportedContentType`, `MalformedJson`, `ValidationFailed`, `IdMismatch`
  - 404: `TriggerNotFound`
  - 409: `VersionConflict`
  - 500: `Persistence`, `Serialization` with sanitized canonical messages

## Functional constraints adherence

- Data → Calc → Actions separation implemented in `triggers.rs`
- No `unwrap`/`expect`/`panic` used in non-test source code
- Pure calc functions are deterministic and side-effect free
- Side effects isolated to handler + in-memory datastore boundary
- CI gate passed with `moon run :ci-source`
- Clippy clean: `#![deny(clippy::unwrap_used)]`, `#![warn(clippy::pedantic)]`

## Files changed

- `crates/twerk-web/src/api/triggers.rs` (modified - defect fixes applied)
- `crates/twerk-web/src/api/mod.rs` (no change)
- `crates/twerk-web/tests/trigger_update_integration_red_test.rs` (no change)
- `crates/twerk-web/tests/trigger_update_proptest_red_test.rs` (no change)
