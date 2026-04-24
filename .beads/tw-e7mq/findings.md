# Findings: tw-e7mq - QA-MANUAL: exploratory test round 8

## Session Context
- **Polecat**: brahmin (rig: twerk)
- **Date**: 2026-04-24
- **Task**: Exploratory QA testing of twerk codebase

## Build Status
- **Result**: PASS
- `cargo build --release` completes successfully in ~39s

## Code Quality Observations

### 1. Error Handling - GOOD
- The codebase has comprehensive error handling with `ApiError` enum
- Internal errors hide details from clients (security: only "Internal Server Error" returned)
- Validation errors are properly returned with specific messages
- Most modules use `anyhow::Result` and `thiserror` for typed errors

### 2. Panics in Tests Only
- Found 1 panic in broker utils test (`#[allow(clippy::panic)]`) - this is in test code only
- The `deny(clippy::panic)` lint is active in coordinator handlers module
- No panic statements found in production code paths

### 3. YAML Parsing - OBSERVATION
- Null bytes in YAML body return generic "YAML parse error" (could be more specific)
- MAX_YAML_BODY_SIZE is 512KB - reasonable limit
- Duplicate keys in YAML are caught and return an error

### 4. Validation Coverage - GOOD
- Job name validation: required, max 256 chars, no empty/whitespace-only
- Task validation: at least one task required, task names must be non-empty
- Queue name validation: 1-128 chars, lowercase alphanumeric, hyphens, underscores, dots
- Duration, priority, queue defaults all validated

### 5. API Observations
- `wait=true` mode (Blocking) has 1-hour timeout - very long for client use
- `wait=blocking` is the same as `wait=true`
- Jobs with `wait=true` and a running server: observed job stuck in PENDING state
  - A job with 2 tasks was submitted and remained PENDING for >30s
  - This could indicate a worker processing issue or simply that standalone mode workers aren't picking up jobs

### 6. Code Organization
- Clean separation: `twerk-core`, `twerk-app`, `twerk-web`, `twerk-infrastructure`, `twerk-cli`
- Extensive test coverage with integration tests, benchmark tests, and mutation tests
- OpenAPI spec generation present

## Edge Cases Tested

| Input | Expected | Actual |
|-------|----------|--------|
| Empty YAML body | 400 "YAML body is empty" | PASS |
| Invalid job ID format | 400 "invalid job ID format" | PASS |
| Nonexistent job GET | 404 Not Found | PASS |
| Invalid content type | 400 "unsupported content type" | PASS |
| `wait=true` on running server | Job result after completion | Job stuck in PENDING |

## Potential Issues

### Issue 1: Jobs Stuck in PENDING
**Severity**: Medium
**Description**: When submitting jobs via POST /jobs, jobs remain in PENDING state even after extended time. This was observed on a running standalone server.
**Possible causes**:
- Worker not picking up jobs from queue
- In-memory broker/queue not properly connected to executor
- Task execution not triggered

### Issue 2: wait=true Timeout Very Long
**Severity**: Low (design choice)
**Description**: The blocking wait timeout is 1 hour (3600 seconds). Clients using `wait=true` may hang for extended periods.
**Note**: This appears to be intentional for long-running jobs, but could surprise clients expecting shorter waits.

## Files Reviewed
- `/home/lewis/src/twerk/crates/twerk-web/src/api/error/core.rs` - Error handling
- `/home/lewis/src/twerk/crates/twerk-web/src/api/yaml.rs` - YAML parsing
- `/home/lewis/src/twerk/crates/twerk-core/src/validation/job.rs` - Job validation
- `/home/lewis/src/twerk/crates/twerk-core/src/domain/queue_name.rs` - Queue validation
- `/home/lewis/src/twerk/crates/twerk-infrastructure/src/broker/utils.rs` - Queue utilities
- `/home/lewis/src/twerk/crates/twerk-app/src/engine/datastore/proxy.rs` - Datastore proxy
- `/home/lewis/src/twerk/crates/twerk-web/src/api/content_type.rs` - Content type handling

## Conclusion
The codebase is well-structured with good error handling practices. The main observation is that jobs appear to get stuck in PENDING state when submitted to a standalone server, which may indicate an issue with job execution in inmemory mode.
