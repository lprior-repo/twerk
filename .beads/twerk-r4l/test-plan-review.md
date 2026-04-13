---
bead_id: twerk-r4l
bead_title: "action: Implement trigger update endpoint (PUT /api/v1/triggers/{id})"
phase: state-1.6-test-plan-review
review_mode: plan-inquisition
reviewed_at: 2026-04-13T00:00:00Z
plan_revision_reviewed: 2
---

# Test Plan Re-Review: Trigger Update Endpoint

**STATUS: APPROVED**

No LETHAL findings. No rejection thresholds hit.

## Hard Evidence

### 1) Contract parity — PASS

- Public contract functions are all covered by BDD scenarios:
  - `update_trigger_handler` declared at `contract.md:136-142`, covered in BDD block `test-plan.md:82-185`.
  - `validate_trigger_update` declared at `contract.md:153-157`, covered in BDD block `test-plan.md:188-255`.
  - `apply_trigger_update` declared at `contract.md:158-163`, covered in BDD block `test-plan.md:258-319`.

- Error variants in `TriggerUpdateError` (`contract.md:106-123`) all have explicit scenario coverage in plan:
  - `InvalidIdFormat` → `test-plan.md:35`, `169-173`, `196-201`, `251-255`
  - `UnsupportedContentType` → `test-plan.md:36`, `90-95`
  - `MalformedJson` → `test-plan.md:37`, `96-101`
  - `ValidationFailed` → `test-plan.md:38-40`, `102-107`, `181-185`, `202-225`, `284-319`
  - `IdMismatch` → `test-plan.md:41`, `108-113`, `226-231`
  - `TriggerNotFound` → `test-plan.md:43`, `114-119`
  - `VersionConflict` → `test-plan.md:44`, `120-125`
  - `Persistence` → `test-plan.md:45`, `126-131`
  - `Serialization` → `test-plan.md:46`, `132-137`

### 2) Assertion sharpness — PASS

- No `Then:` uses banned `is_ok()` / `is_err()` shortcuts.
  - Evidence sweep across BDD sections: `test-plan.md:88-184`, `194-255`, `264-319`.
- Then assertions are concrete (exact JSON payloads / exact enum variants / exact field equality), e.g.:
  - Exact malformed JSON payload: `test-plan.md:100`
  - Exact id mismatch payload: `test-plan.md:112`
  - Exact enum variant assertions for validator/apply: `test-plan.md:200`, `206`, `230`, `288`

### 3) Trophy allocation gates — PASS

- Unit count floor satisfied:
  - Planned unit scenarios = `16` (`test-plan.md:14`, `73`)
  - Public function count = `3` (`contract.md:136`, `153`, `158`)
  - Required minimum = `5 × 3 = 15`
  - Actual `16 >= 15` ✅

- Proptest invariants present for pure multi-input functions (`validate_trigger_update`, `apply_trigger_update`):
  - `test-plan.md:320-336`

- Parser/deserializer fuzz targets present:
  - TriggerId parser fuzz: `test-plan.md:339-344`
  - `serde_json::from_slice::<TriggerUpdateRequest>` fuzz: `test-plan.md:345-349`

### 4) Boundary completeness — PASS

- `update_trigger_handler` has explicit transport boundaries:
  - min id: `test-plan.md:156-161`
  - max id: `test-plan.md:162-167`
  - one-above-max id: `test-plan.md:168-173`
  - max body bytes: `test-plan.md:174-179`
  - body max+1 rejection: `test-plan.md:180-185`
  - empty object handling: `test-plan.md:102-107`

- `validate_trigger_update` has explicit field/id boundaries:
  - min length accepted: `test-plan.md:232-237`
  - max length accepted: `test-plan.md:238-243`
  - max+1 rejected: `test-plan.md:244-249`
  - id overflow class rejected: `test-plan.md:250-255`
  - empty-after-trim failures: `test-plan.md:202-219`

- `apply_trigger_update` has explicit timestamp/field boundaries:
  - equality edge (`now == updated_at`): `test-plan.md:272-277`
  - monotonic forward boundary: `test-plan.md:278-283`
  - backward rejection: `test-plan.md:284-289`
  - max field length accepted: `test-plan.md:308-313`
  - max+1 rejected: `test-plan.md:314-319`

### 5) Mutation survivability planning — PASS

- Explicit mutation-to-test mapping exists with named killers:
  - table at `test-plan.md:407-422`
  - includes `>` vs `>=` timestamp mutation killer: `test-plan.md:414`
  - includes rollback deletion killer: `test-plan.md:416`
  - includes status mapping killers: `test-plan.md:417-418`

## Findings Summary

### LETHAL FINDINGS
- None.

### MAJOR FINDINGS (0)
- None.

### MINOR FINDINGS (1/5 threshold)
1. `test-plan.md:106` uses wording "exact first failing message constant" for `{}` body validation. This is stricter than wildcard matching, but still order-dependent language. Consider pinning a single explicit canonical message string for this scenario to remove ambiguity.

## Verdict

**STATUS: APPROVED**

Plan now satisfies Mode 1 hard gates: contract parity, exact error-variant planning, unit-density floor, proptest coverage for pure functions, and parser/deserializer fuzz coverage.
