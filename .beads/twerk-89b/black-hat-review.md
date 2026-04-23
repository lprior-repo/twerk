bead_id: twerk-89b
phase: state-11-black-hat-review
updated_at: 2026-04-23T23:59:59Z

STATUS: REJECTED

## VERDICT: REJECTED

### Scope
- Read `.beads/twerk-89b/STATE.md`, `.beads/twerk-89b/moon-report.md`, `.beads/twerk-89b/qa-report.md`, `.beads/twerk-89b/qa-review.md`, `.beads/twerk-89b/test-suite-review.md`, and `.beads/twerk-89b/implementation.md` first.
- Reviewed the current snapshot in the changed Rust source and test files for contract parity, functional-rust rigor, exactness, and hidden fraud.
- Did not modify source code.

### Phase 1 — Contract & Bead Parity
- FAIL — `crates/twerk-core/src/types/retry_limit.rs:11-19,28-35,73-76` lies about the contract. `RetryLimit` is presented as a validated type, but `RetryLimit::new` accepts any `u32` and `From<u32>` bypasses validation entirely. Illegal states are still representable.
- FAIL — `crates/twerk-core/src/validation/primitives.rs:47-66,112-114` keeps the real range check in `parse_retry` while exposing a domain type that can still be constructed out-of-contract elsewhere. That is not type-enforced parity. That is a paper contract.

### Phase 2 — Farley Engineering Rigor
- FAIL — `crates/twerk-app/src/engine/coordinator/scheduler/parallel.rs:16-115` is a 100-line imperative blob that does datastore mutation, task evaluation, broker publishing, and rollback in one place. Functional core and imperative shell are smeared together.
- FAIL — `crates/twerk-app/src/engine/coordinator/scheduler/each.rs:26-87` is another oversized mixed-concern handler, and `crates/twerk-app/src/engine/coordinator/scheduler/each.rs:114-122` gives `spawn_each_tasks` 6 parameters. Hard constraints blown.
- FAIL — `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs:89-132,168-204,230-286` and `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs:27-59,159-189,232-283` are still long event-handler blobs instead of small, boring state transitions.

### Phase 3 — Functional Rust / Big 6
- FAIL — `crates/twerk-core/src/types/retry_limit.rs:32-35,73-76` violates “make illegal states unrepresentable” and “parse, don’t validate.” The type itself does not enforce the invariant.
- FAIL — `crates/twerk-app/src/engine/worker/docker.rs:82-87` still uses boolean flags (`privileged: bool`) in a constructor. That is weak type design and garbage documentation-by-convention.

### Phase 4 — Ruthless Simplicity & DDD
- FAIL — `crates/twerk-cli/src/cli/mod.rs:263-268` explicitly suppresses `unwrap_used`/`expect_used` and then calls `get_endpoint().unwrap()`. Deliberate panic-vector normalization is not acceptable.
- FAIL — `crates/twerk-app/src/engine/worker/shell.rs:486,510` still uses `expect(...)` in tests. The panic vector is still present in reviewed files.

### Phase 5 — Bitter Truth / Hidden Fraud
- FAIL — `crates/twerk-web/tests/trigger_update_adversarial_test.rs:402-406` claims to verify that `updated_at` advances, then asserts only `second_updated_at >= first_updated_at`. That permits “did not advance” and calls it success. Fraud.
- FAIL — the bead claims repo-wide architectural cleanup and DRY sweep, but the changed snapshot still leaves giant handlers, giant schedulers, boolean flags, and fake-validated types in place. This is not architectural cleanup. It is surface repair plus formatting.

### Decision
REJECTED.

The machine gates are green. Good. That does not save this snapshot. The type contract for retry limits is still fake, the changed coordinator paths still violate the hard size/shape constraints, and at least one adversarial test still hides behind a permissive assertion. Rewrite the affected paths until the invariants live in types, the handlers are broken into explicit state transitions, and the tests stop pretending approximate behavior is exact.
