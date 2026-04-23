bead_id: twerk-89b
updated_at: 2026-04-23T23:59:00Z

## Summary

- Made `RetryLimit` a real validated type by enforcing the `1..=10` invariant in construction and deserialization, then aligned retry-limit tests with the typed contract.
- Split scheduler and coordinator hot paths into smaller helpers so the cited `parallel`, `each`, `job_handlers`, and `task_handlers` flows no longer keep datastore mutation, evaluation, routing, and publish compensation fused in single blobs.
- Replaced the Docker boolean constructor flag with a typed policy seam, and removed the cited unwrap/expect discipline issues from the CLI and shell worker files.

## Files Changed

- `crates/twerk-core/src/types/retry_limit.rs`
- `crates/twerk-core/src/validation/primitives.rs`
- `crates/twerk-core/src/types/types_test.rs`
- `crates/twerk-core/tests/types_integration_test.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/mod.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/shared.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/parallel.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/each.rs`
- `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs`
- `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs`
- `crates/twerk-app/src/engine/worker/docker.rs`
- `crates/twerk-app/src/engine/worker/runtime_adapter.rs`
- `crates/twerk-app/src/engine/worker/shell.rs`
- `crates/twerk-cli/src/cli/mod.rs`
- `.beads/twerk-89b/implementation.md`

## Targeted Repair Mapping

- `RetryLimit` now rejects illegal states at the type boundary, including serde entry points, so callers cannot smuggle `0` or `>10` through `new`/conversion paths.
- Scheduler repair extracted shared identity/running/publish-compensation actions and reduced `each` spawn inputs to a typed request instead of a 6-argument helper.
- Handler repair extracted persistence and routing helpers so completion/failure/error flows are smaller and more explicit.
- Docker runtime construction now takes `DockerRuntimePolicy { privilege, image_ttl_secs }` with `DockerPrivilege` instead of a bare boolean flag.
- The cited CLI and shell files no longer rely on the reviewed unwrap/expect sites.

## Verification

- Focused proofs passed for retry-limit integration behavior, CLI endpoint override handling, and shell task-log publication.
- `moon run :ci-source` passed on the repaired snapshot.

## Remaining Out of Scope

- The adversarial `updated_at` strict-advance assertion in `crates/twerk-web/tests/trigger_update_adversarial_test.rs` was not changed in this repair pass.
- Global State 11 quality-gate survivors called out by Red Queen (`cargo audit`, `cargo deny`, coverage floor, broader strict-clippy sweep) were intentionally left for later targeted work.
