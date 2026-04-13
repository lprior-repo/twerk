---
review_id: black-hat-review-twerk-r4l-trigger-update-v2
bead_id: twerk-r4l
bead_title: action: Implement trigger update endpoint (PUT /api/v1/triggers/{id})
review_timestamp: 2026-04-14T18:30:00Z
reviewer: black-hat-reviewer
---

# Black Hat Review: Trigger Update Endpoint — Round 2

## VERDICT: REJECTED

**Previous defects from round 1 were FIXED** (read-back pattern, apply_trigger_update length, error_response verbosity, handler length). However, this round identified **2 new critical defects and 1 major defect** that require remediation.

---

## PHASE 1: Contract Parity — **REJECTED**

### DEFECT-1: CRITICAL — `TriggerDatastore` trait does not exist in source

**Contract specification** (contract.md lines 143-151):
```rust
pub trait TriggerDatastore: Send + Sync {
    async fn get_trigger_by_id(&self, id: &TriggerId) -> Result<Trigger, TriggerUpdateError>;
    async fn update_trigger(
        &self,
        id: &TriggerId,
        modify: Box<dyn FnOnce(Trigger) -> Result<Trigger, TriggerUpdateError> + Send>,
    ) -> Result<(), TriggerUpdateError>;
}
```

**Reality**: This trait is **not defined anywhere** in the source. The code uses `InMemoryTriggerDatastore` struct directly (triggers.rs lines 246-315), with no trait abstraction.

**Impact**: The contract mandates a `TriggerDatastore` abstraction; the implementation bypasses it entirely. This is an architectural contract violation.

---

### DEFECT-2: CRITICAL — `update_trigger` return type mismatch with contract

**Contract requires**:
```rust
async fn update_trigger(...) -> Result<(), TriggerUpdateError>;  // Returns ()
```

**Implementation** (triggers.rs lines 283-303):
```rust
pub fn update_trigger(
    &self,
    id: &TriggerId,
    modify: Box<dyn FnOnce(Trigger) -> Result<Trigger, TriggerUpdateError> + Send>,
) -> Result<Trigger, TriggerUpdateError>  // Returns Trigger, not ()
```

**Issues**:
1. **Returns `Trigger`, not `()`** — The contract specifies `Result<(), E>` implying caller re-fetches; the implementation returns the entity directly
2. **Synchronous, not `async`** — Contract specifies `async fn`

**Note**: The implementation behavior (returning `Trigger`) is actually **better** for the handler (eliminates read-after-write). But it violates the contract's specified interface.

---

## PHASE 2: Farley Engineering Rigor — **REJECTED**

### DEFECT-3: MAJOR — `update_trigger_handler` exceeds 25-line limit

**Farley rule**: No function > 25 lines.

**Current implementation** (triggers.rs lines 463-510): **47 lines** — 88% over limit.

| Section | Lines |
|---------|-------|
| Body size check | 5 |
| Content-Type parsing | 4 |
| Request decoding | 5 |
| Validation | 4 |
| Version check | 3 |
| Timestamp + clone | 3 |
| Datastore call + match | 6 |
| Serialization | 4 |
| **Total** | **34 + 5 (signature) = 39... wait let me recount** |

Let me recount the actual body:
```rust
463: pub async fn update_trigger_handler(
464:     State(state): State<AppState>,
465:     Path(id): Path<String>,
466:     headers: HeaderMap,
467:     body: Bytes,
468: ) -> Result<Response, ApiError> {
469:     if body.len() > MAX_BODY_BYTES {              // 1
470:         return Ok(error_response(TriggerUpdateError::ValidationFailed(  // 2
471:             BODY_TOO_LARGE_MSG.to_string(),      // 3
472:         )));                                     // 4
473:     }                                           // 5
474:                                                  // 6
475:     let _content_type = match parse_content_type(&headers) {  // 7
476:         Ok(ct) => ct,                           // 8
477:         Err(err) => return Ok(error_response(err)),  // 9
478:     };                                          // 10
479:                                                  // 11
480:     let req = match decode_trigger_update_request(&body) {  // 12
481:         Ok(value) => value,                      // 13
482:         Err(err) => return Ok(error_response(err)),  // 14
483:     };                                          // 15
484:                                                  // 16
485:     let trigger_id = match validate_trigger_update(&id, &req) {  // 17
486:         Ok(parsed) => parsed,                   // 18
487:         Err(err) => return Ok(error_response(err)),  // 19
488:     };                                          // 20
489:                                                  // 21
490:     if let Some(err) = check_version_constraints(&headers, &req) {  // 22
491:         return Ok(error_response(err));          // 23
492:     }                                          // 24
493:                                                  // 25
494:     let now_utc = OffsetDateTime::now_utc();    // 26
495:     let req_for_update = req.clone();          // 27
496:                                                  // 28
496:     let updated = match state.trigger_state.trigger_ds.update_trigger(  // 29
497:         &trigger_id,                             // 30
498:         Box::new(move |current| apply_trigger_update(current, req_for_update, now_utc)),  // 31
499:     ) {                                         // 32
500:         Ok(t) => t,                             // 33
501:         Err(err) => return Ok(error_response(err)),  // 34
502:     };                                         // 35
503:                                                  // 36
504:     let view = TriggerView::from(updated);      // 37
505:     match serialize_view(view) {               // 38
506:         Ok(resp) => Ok(resp),                   // 39
507:         Err(err) => Ok(error_response(err)),    // 40
508:     }                                          // 41
509: }                                              // 42
```

That's 42 lines of body (lines 469-509 excluding signature). Plus the 5-line signature = 47 total.

**Implementation.md claimed** (line 44): "Handler now ~40 lines with flat control flow."

Even at 40 lines, this is **60% over the 25-line limit**.

---

## PHASE 3: NASA-Level Functional Rust (Big 6) — **APPROVED**

### PASSED

- No `unwrap()`, `expect()`, `panic!()` in non-test source
- `#![deny(clippy::unwrap_used)]` enforced
- All fallible operations use `Result<T, E>`
- `TriggerId` is a proper newtype wrapper
- `TriggerUpdateError` is exhaustive enum

---

## PHASE 4: Strict DDD — **WARNING**

### DEFECT-4: MINOR — Metadata uniqueness invariant not enforced

**Contract invariant** (contract.md line 72):
> `metadata` keys are unique, non-empty, and ASCII-safe.

**Implementation** (triggers.rs lines 167-178):
```rust
fn validate_metadata(metadata: Option<&HashMap<String, String>>) -> Result<(), TriggerUpdateError> {
    let invalid = metadata
        .into_iter()
        .flat_map(|map| map.keys())
        .any(|key| key.is_empty() || !key.is_ascii());
    // ↑ Only checks non-empty and ASCII. Does NOT check uniqueness!
    ...
}
```

**Impact**: Duplicate keys like `{"a": "1", "a": "2"}` are silently accepted (HashMap deduplicates). This violates the "unique keys" contract invariant.

---

## PHASE 5: Bitter Truth — **APPROVED**

### PASSED

- No clever tricks or over-engineering
- `err_json!` macro is appropriately simple
- `error_details()` lookup table is readable
- Tests assert behavior (WHAT), not implementation (HOW)

---

## Summary

| Phase | Verdict | Severity | Count |
|-------|---------|----------|-------|
| 1. Contract Parity | REJECTED | CRITICAL | 2 |
| 2. Farley Constraints | REJECTED | MAJOR | 1 |
| 3. Functional Rust | APPROVED | — | 0 |
| 4. Strict DDD | WARNING | MINOR | 1 |
| 5. Bitter Truth | APPROVED | — | 0 |

---

## Previous Defects — STATUS UPDATE

| Defect (Round 1) | Status |
|-----------------|--------|
| Read-after-write semantic violation | ✅ FIXED — `update_trigger` returns `Trigger` directly |
| `apply_trigger_update` 27 lines | ✅ FIXED — Extracted `validate_timestamp_monotonicity`, now 23 lines |
| `error_response` 48 lines | ✅ FIXED — `err_json!` macro + `error_details()` lookup, now 4 lines |
| `update_trigger_handler` 80 lines | ⚠️ PARTIAL — Reduced to 42-47 lines, still over 25-line limit |

---

## Required Fixes for Approval

### CRITICAL (Must Fix):

1. **DEFECT-1 (TriggerDatastore trait missing)**:
   - **Option A**: Remove `TriggerDatastore` trait from contract.md (since implementation uses `InMemoryTriggerDatastore` directly)
   - **Option B**: Implement the trait in source (if trait abstraction is required)

2. **DEFECT-2 (update_trigger signature mismatch)**:
   - **Option A**: Change implementation to `async fn update_trigger(...) -> Result<(), TriggerUpdateError>` (and handle the read-back differently)
   - **Option B**: Update contract.md to specify `Result<Trigger, TriggerUpdateError>` and sync (to match implementation)

### MAJOR (Must Fix):

3. **DEFECT-3 (Handler 47 lines)**:
   Extract remaining inline logic into helper functions until handler body is ≤25 lines:
   - Extract body size check to `check_body_size()`
   - Extract the entire request-processing pipeline to `process_update_request()`

### MINOR (Should Fix):

4. **DEFECT-4 (Metadata uniqueness not enforced)**:
   Add uniqueness check in `validate_metadata()` using a `HashSet`.

---

**Review Date**: 2026-04-14  
**Reviewer**: Black Hat (State 5.5)  
**STATUS**: REJECTED — Requires remediation before approval
