# Improvements

## Scope

This document turns the exhaustive first-party repo read into a concrete remediation plan.

- Scope covered: all tracked first-party files in this repository.
- Explicitly excluded: `vendor/` third-party vendored code.
- Coverage result:
  - `1424` tracked first-party files accounted for.
  - `1386` line-oriented text files read fully.
  - `38` binary or non-line-oriented tracked files explicitly excluded from line-by-line coverage.
  - `0` unread tracked first-party files.

This is not a vague wishlist. It is a prioritized fix list based on directly read code, tests, docs, generated docs, QA assets, and tracked project metadata.

## What "bullet proof" means here

If this codebase is going to be reliable under Functional Rust / Holzmann-style discipline, the following rules need to become true across the repo:

1. Cancellation, timeout, and shutdown paths must tell the truth.
2. Illegal states must not be constructible through `Default`, unchecked constructors, or infallible `From` impls.
3. Invalid input must fail closed, not silently coerce to defaults.
4. Persistence must surface corruption rather than defaulting over it.
5. Event payloads must match persisted state.
6. Observability must be structured, consistent, and wired through async boundaries.
7. CLI and API machine-readable contracts must actually be machine-readable.
8. Docs, generated docs, QA assets, and tests must stop claiming behavior the code does not implement.

## Priority Order

Work should be done in this order:

1. Truthful cancellation, timeout, and shutdown behavior.
2. Newtypes and illegal-state prevention.
3. Fail-closed parsing and persistence.
4. State and event contract parity.
5. Observability and machine-facing contract cleanup.
6. Docs, generated docs, QA assets, and stale test repair.

## Priority 0: Fix correctness lies first

These are the issues most likely to make the system say one thing while doing another.

### 1. Cancellation currently reports success too often

Files:

- `crates/twerk-infrastructure/src/worker/internal/execution.rs`
- `crates/twerk-infrastructure/src/worker/internal/worker.rs`
- `crates/twerk-app/src/engine/worker/mod.rs`

Problems:

- Cancellation returns `Ok(())` and the execution path can later mark work `Completed`.
- Worker stop waits for active tasks to drain but does not actively stop them.
- Subscriber tasks are not wired to terminate promptly.

Why this matters:

- A cancelled task that gets reported as completed is worse than a hard failure. It corrupts the control plane.
- Operators will trust state transitions that are not true.

What to change:

- Introduce a distinct cancellation result path that maps to `TaskState::Cancelled`, never `Completed`.
- Make worker shutdown enumerate active tasks and call `runtime.stop(...)` explicitly.
- Ensure all subscription loops select on cancellation before or alongside long-lived subscription work.
- Stop using success return values to represent cancelled or timed-out execution.

Acceptance criteria:

- Cancelling a running task causes the runtime stop path to run.
- Final persisted state is `Cancelled`.
- No cancellation path returns a success value that is later mapped to `Completed`.
- Tests cover shell, Docker, and Podman cancellation behavior.

### 2. Timeout behavior is not authoritative

Files:

- `crates/twerk-infrastructure/src/worker/internal/execution.rs`
- `crates/twerk-infrastructure/src/runtime/docker/runtime/mod.rs`
- `crates/twerk-app/src/engine/worker/podman.rs`
- `crates/twerk-app/src/engine/worker/shell.rs`

Problems:

- Invalid timeout strings can silently disable timeout enforcement.
- Docker `stop()` is a no-op success, so timed-out work can continue running.
- Podman stop tracking is broken.
- Shell shutdown uses external `wait`, which is not valid as written.

Why this matters:

- A timed-out task that keeps running is a resource leak and a contract violation.
- Timeout configuration becomes advisory instead of enforced.

What to change:

- Parse timeout once at the boundary into a validated duration type.
- Reject malformed timeout values rather than falling back to no-timeout execution.
- Make Docker and Podman stop paths real and testable.
- Replace shell `wait` command usage with child-handle-based waiting or a correct process control strategy.

Acceptance criteria:

- A malformed timeout fails the task definition or request, not runtime enforcement.
- A timed-out Docker or Podman task is actually stopped.
- Shell stop path proves the child is gone before returning success.

### 3. Shutdown lifecycle is incomplete and can hang

Files:

- `crates/twerk-app/src/engine/engine_lifecycle.rs`
- `crates/twerk-app/src/engine/worker/mod.rs`
- `crates/twerk-cli/src/run.rs`

Problems:

- Signal handlers stop components but do not complete engine termination state.
- `run()` can wait forever for termination notification that never arrives.
- CLI startup can partially start the engine before API server startup fails.

Why this matters:

- Shutdown paths are part of correctness, not polish.
- Partial-start failures leave ambiguous process state.

What to change:

- Centralize termination in one explicit engine lifecycle path.
- Make signal handlers call the same termination function normal shutdown uses.
- Reverse startup order or add cleanup-on-failure so partially started services are torn down deterministically.

Acceptance criteria:

- SIGINT and SIGTERM complete shutdown and return control to `run()`.
- Failed API bind after engine start triggers cleanup.
- No background task is left detached without lifecycle ownership.

## Priority 1: Make illegal states unrepresentable

### 4. Validated IDs are bypassable

Files:

- `crates/twerk-core/src/id/common.rs`
- `crates/twerk-core/src/id/job_id.rs`
- `crates/twerk-core/src/id/trigger_id.rs`
- call sites across app and infrastructure that use `Type::from(...)`

Problems:

- Validated IDs derive `Default`.
- Validated IDs implement infallible `From<String>` and `From<&str>`.
- Tests explicitly prove empty and invalid IDs can be constructed.

Why this matters:

- This defeats parse-don't-validate.
- Empty string IDs and other invalid IDs can leak across the system while pretending to be trusted domain values.

What to change:

- Remove `Default` from validated IDs.
- Replace infallible `From` impls with `TryFrom` or named parsing constructors.
- Audit all call sites that currently create IDs without error handling.
- Do the same audit for non-ID validated wrappers such as ports, progress values, retry limits, task counts, and positions.

Acceptance criteria:

- Production code cannot construct an invalid ID without an explicit unchecked escape hatch that is private or heavily isolated.
- Empty string IDs cannot compile into default paths.
- `cargo clippy` or compile failures catch reintroduced infallible construction.

### 5. `Endpoint` is not total and still allows panic

Files:

- `crates/twerk-core/src/domain/endpoint.rs`

Problems:

- `new_unchecked()` is public.
- `as_url()` reparses and uses `expect(...)`.

Why this matters:

- A type advertised as validated should not contain a panic trigger in normal usage.

What to change:

- Remove or privatize `new_unchecked()`.
- Store the parsed value or return a `Result` instead of `expect(...)`.
- Audit all callers to ensure the invariant is established only once at the boundary.

Acceptance criteria:

- No production `expect` or panic path remains in `Endpoint`.
- The type cannot be publicly constructed into an invalid state.

### 6. Primitive wrappers also bypass validation

Files:

- `crates/twerk-core/src/types/port.rs`
- `crates/twerk-core/src/types/progress.rs`
- `crates/twerk-core/src/types/retry_limit.rs`
- `crates/twerk-core/src/types/task_count.rs`
- `crates/twerk-core/src/types/task_position.rs`

Problems:

- Infallible constructors allow illegal ports, progress values, retry limits, and task counts.

Why this matters:

- These types look safe but are not actually enforcing their contracts.

What to change:

- Remove infallible `From` impls for validated wrappers.
- Use `TryFrom` or explicit parse/validate functions.
- Keep custom serde validation and align manual constructors with it.

Acceptance criteria:

- The only way to create one of these values in production code is through validated construction.

### 7. Raw strings still cross too many safety-critical boundaries

Files:

- `crates/twerk-core/src/trigger/data.rs`
- `crates/twerk-common/src/conf/env.rs`
- `crates/twerk-common/src/env.rs`
- `crates/twerk-cli/src/run.rs`
- `crates/twerk-infrastructure/src/worker/api/server.rs`
- `crates/twerk-infrastructure/src/runtime/podman/runtime/task_execution.rs`

Problems:

- Trigger webhook URLs are validated by string prefix checks.
- Hostnames and socket addresses are built from raw strings.
- Env/config key mapping is lossy.
- File injection paths are not represented by safe path types.

Why this matters:

- Most correctness failures in this codebase cluster at raw string boundaries.

What to change:

- Parse trigger URLs into `WebhookUrl` or an equivalent validated type.
- Use structured socket address construction instead of string assembly.
- Introduce typed filename/path wrappers for container file injection.
- Define a single reversible env-key mapping strategy.

Acceptance criteria:

- No raw-string prefix checks remain for URL validation.
- No `":8001"`-style socket string construction remains.
- Path traversal via `..` is impossible by construction.

## Priority 2: Fail closed on input and persistence

### 8. API parsing is too tolerant and silently defaults bad input

Files:

- `crates/twerk-web/src/api/trigger_api/handlers/parsing.rs`
- `crates/twerk-web/src/api/handlers/tasks.rs`
- `crates/twerk-web/src/api/handlers/mod.rs`
- `crates/twerk-web/src/api/domain/pagination.rs`

Problems:

- Trigger update parsing manually extracts fields and coerces wrong types to defaults.
- Invalid pagination input is normalized instead of rejected.
- User extraction can collapse to empty or guest values.

Why this matters:

- Silent coercion hides caller mistakes and makes the system non-deterministic from a client perspective.

What to change:

- Replace hand-rolled trigger JSON parsing with strict serde DTOs.
- Reject malformed pagination values with stable `400` responses.
- Make authentication expectations explicit at handler boundaries.
- Stop using empty strings as "missing user" sentinels.

Acceptance criteria:

- Wrong input types fail with precise `400` responses.
- Pagination errors are deterministic and documented.
- Missing auth is either impossible on protected routes or explicitly modeled.

### 9. Persistence hides corruption by defaulting over it

Files:

- `crates/twerk-infrastructure/src/datastore/postgres/records/job.rs`
- `crates/twerk-infrastructure/src/datastore/postgres/records/task.rs`
- `crates/twerk-infrastructure/src/datastore/postgres/records/scheduled_job.rs`

Problems:

- Persisted state strings are parsed with `unwrap_or_default()`.

Why this matters:

- Corrupt database values get silently reinterpreted as normal states instead of surfacing as corruption.

What to change:

- Replace defaulting with explicit datastore serialization/corruption errors.
- Prefer typed database enums or explicit conversion layers.

Acceptance criteria:

- Invalid persisted state values fail reads with a typed error.
- No state machine enum in persistence is recovered through `unwrap_or_default()`.

### 10. Task persistence contracts drift from runtime contracts

Files:

- `crates/twerk-infrastructure/src/datastore/postgres/impl_task_logs.rs`
- `crates/twerk-infrastructure/src/runtime/docker/container/tcontainer.rs`
- `crates/twerk-infrastructure/src/datastore/postgres/impl_tasks.rs`
- `crates/twerk-infrastructure/src/datastore/postgres/schema.rs`

Problems:

- Runtime log publishers emit `TaskLogPart { id: None, ... }` while Postgres requires an ID.
- Probe config is dropped when tasks round-trip through Postgres.
- `users_roles` insert shape does not match schema.

Why this matters:

- These are direct cross-layer contract breaks.

What to change:

- Align schema, persistence, and runtime data shapes.
- Decide whether IDs are generated at source or persistence boundary, then make it consistent.
- Add round-trip tests that prove no fields are lost.

Acceptance criteria:

- Task logs persist successfully end to end.
- Probe config survives persistence round-trips.
- Role assignment SQL matches the actual schema.

### 11. Config parsing and duration parsing are inconsistent

Files:

- `crates/twerk-common/src/conf/parsing.rs`
- `crates/twerk-common/src/conf/lookup.rs`
- `crates/twerk-core/src/domain_types.rs`
- `crates/twerk-core/src/validation.rs`
- `crates/twerk-infrastructure/src/worker/internal/execution.rs`

Problems:

- YAML filenames are searched but parser is TOML-only.
- Fractional seconds rounding is incorrect.
- Duration parsing behavior is duplicated and drifts across modules.
- Oversized durations can saturate instead of erroring.

Why this matters:

- Timeouts, schedules, and config values should not have multiple subtly different languages.

What to change:

- Define one canonical duration parser/type.
- Use it across common, core, app, infrastructure, and web.
- Either support YAML for real or remove YAML from the default search path.
- Reject overflow instead of saturating.

Acceptance criteria:

- Every duration boundary accepts and rejects the same language.
- `1.5s` means the same thing everywhere.
- YAML support claims match real behavior.

## Priority 3: Repair state and event contract parity

### 12. Events do not always reflect persisted state

Files:

- `crates/twerk-web/src/api/handlers/jobs/mutation.rs`
- `crates/twerk-web/src/api/handlers/scheduled/lifecycle.rs`
- `crates/twerk-web/src/api/handlers/scheduled/shared.rs`

Problems:

- Job cancel/restart handlers publish stale pre-update payloads.
- Deleting an active scheduled job emits a pause-shaped event instead of a delete-shaped event.

Why this matters:

- Consumers receive contradictory state depending on whether they trust the database or the event bus.

What to change:

- Publish post-mutation payloads only.
- Introduce explicit deletion events or clearly separate deletion from pause semantics.
- Add parity tests between datastore state and published payload state.

Acceptance criteria:

- Consumers never see stale payload state after a successful mutation.
- Delete operations publish delete semantics, not overloaded pause semantics.

### 13. Scheduler and cancellation semantics are inconsistent

Files:

- `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs`
- `crates/twerk-app/src/engine/coordinator/handlers/cancellation.rs`
- `crates/twerk-app/src/engine/coordinator/scheduler/each.rs`
- `crates/twerk-app/src/engine/coordinator/schedule.rs`

Problems:

- Cancelled jobs can skip persistence due to `is_job_active(...)` gating.
- Node-affined task cancellation republishes instead of cancelling.
- `each.var` is ignored.
- Repeated active events can duplicate scheduled jobs.

Why this matters:

- Control-plane behavior becomes surprising and irreproducible.

What to change:

- Persist cancellation unconditionally when the requested state is cancellation.
- Separate redispatch from cancellation.
- Honor configured iterator variable names.
- Deduplicate scheduled jobs by stable identity.

Acceptance criteria:

- Cancellation means cancellation, not requeue.
- `each.var` behaves as configured.
- Repeated active events do not create duplicate schedulers.

### 14. In-memory repository logic is too stringly and permissive

Files:

- `crates/twerk-core/src/repository_inmemory.rs`

Problems:

- Referential integrity for user-role assignment is weak.
- Pagination uses signed inputs and manual arithmetic.
- CPU averaging is biased.
- The file is overly large and mixes too many concerns.

Why this matters:

- Large, imperative, stringly repositories are where subtle domain bugs survive.

What to change:

- Split into smaller pure helpers.
- Key maps by typed IDs instead of raw strings.
- Validate pagination inputs through typed page/page-size values.
- Enforce referential integrity before mutation.

Acceptance criteria:

- Repository invariants are explicit and testable.
- No empty-string sentinel IDs remain in repository state.

## Priority 4: Fix observability and machine-facing contracts

### 15. Observability is thin, inconsistent, or misleading

Files:

- `crates/twerk-common/src/logging.rs`
- `crates/twerk-web/src/api/router.rs`
- web handlers and worker/coordinator code using `#[instrument(skip_all)]`

Problems:

- Logging setup can panic because it uses `.init()` instead of `try_init()`.
- `TracingWriter` logs write chunks, not lines.
- Request/trace propagation is weak.
- Too many spans skip all fields, reducing correlation value.
- JSON-mode logging behavior is inconsistent across CLI and shared logging code.

Why this matters:

- When async systems fail, observability is the only evidence chain.

What to change:

- Switch to `try_init()` and return errors instead of panicking.
- Buffer line-based writer behavior or make the contract explicitly chunk-based.
- Add request ID and trace ID middleware and propagate through spawned tasks and broker events.
- Replace broad `skip_all` usage with targeted field capture.
- Unify JSON logging behavior for CLI automation paths.

Acceptance criteria:

- Repeated logging setup cannot panic the process.
- Request-scoped traces survive async boundaries.
- JSON logging mode behaves consistently wherever it is claimed.

### 16. CLI JSON mode is not a real contract

Files:

- `crates/twerk-cli/src/cli.rs`
- `crates/twerk-cli/src/health.rs`
- `crates/twerk-cli/src/main.rs`

Problems:

- Some JSON output is hand-built and unescaped.
- Top-level failures still print plain text.
- Logging is disabled in JSON mode while runtime code still emits logs.

Why this matters:

- Automation cannot safely consume a CLI that only sometimes emits valid JSON.

What to change:

- Define typed output structs and serialize them with `serde_json`.
- Make all success and error paths respect JSON mode.
- Decide whether logs go to stderr in structured form or are disabled consistently.

Acceptance criteria:

- `--json` always returns valid JSON for both success and failure.
- No manual string interpolation remains for JSON rendering.

### 17. API error taxonomy is too coarse

Files:

- `crates/twerk-web/src/api/error/conversions.rs`
- `crates/twerk-web/src/api/error/core.rs`
- trigger response mapping files

Problems:

- Different paths map similar domain errors differently.
- Invalid client input can become `500` in generic conversion paths.

Why this matters:

- Clients cannot reliably distinguish their mistakes from server faults.

What to change:

- Introduce explicit API error categories for validation, invalid ID, conflict, timeout, datastore unavailable, and internal fault.
- Make trigger and generic API paths share the same mapping rules.

Acceptance criteria:

- Similar errors map to the same status code and response shape across endpoints.
- `500` is reserved for real server faults.

## Priority 5: Repair docs, generated docs, QA assets, and stale tests

### 18. Generated website content is stale relative to source docs

Files:

- `website/src/cli.md`
- `website/src/jobs.md`
- `website/src/VERIFICATION.md`
- `website/book/cli.html`
- `website/book/jobs.html`
- `website/book/print.html`

Problems:

- Generated book pages still document a `--config` flag that source docs say does not exist.
- Priority semantics disagree between source and generated docs.

Why this matters:

- Users will trust published docs over source markdown.

What to change:

- Fix source docs first.
- Regenerate the mdBook output.
- Add a doc consistency check so stale generated pages are caught in CI.

Acceptance criteria:

- Source docs and generated book agree on flags and priority semantics.

### 19. QA and example assets contain directly broken details

Files:

- `qa/02-submit-job-yaml.yaml`
- `qa/05-get-job-and-task-logs.yaml`
- `qa/06-parallel-tasks.yaml`
- `qa/07-each-iterator.yaml`
- `qa/08-subjob-nested.yaml`
- `examples/twerk-massive-parallel.yaml`
- `docs/PERFORMANCE_TESTING.md`

Problems:

- Several QA files use `$TORK_OUTPUT` instead of `$TWERK_OUTPUT`.
- The massive parallel example claims `200` tasks but enumerates `151`.
- A performance testing command contains broken text.

Why this matters:

- QA assets and examples are executable documentation.

What to change:

- Fix the environment variable name.
- Align example descriptions with actual task counts.
- Repair broken shell examples.

Acceptance criteria:

- QA assets run as documented.
- Example prose matches example content exactly.

### 20. Some tests and reports are stale enough to mislead

Files:

- `crates/twerk-infrastructure/src/runtime/docker/tests.rs`
- `crates/twerk-infrastructure/src/runtime/docker/reference_test.rs`
- `crates/twerk-infrastructure/src/runtime/docker/auth_test.rs`
- `twerk-twi/tests/trigger_registry_tests.rs`
- report files under `twerk-ctz`, `.beads`, and root review documents

Problems:

- Some tracked tests are structurally stale or reference APIs and constants that no longer exist.
- Some reports claim stronger verification than the tracked evidence supports.

Why this matters:

- Broken or stale tests are noise generators.
- Stale reports create false confidence.

What to change:

- Delete or repair tests that no longer reflect the live code.
- Separate historical notes from current truth.
- Treat compile-blocking stale tests as priority work, not background cleanup.

Acceptance criteria:

- Tracked tests compile and assert current contracts.
- Review documents clearly distinguish current facts from historical claims.

### 21. Tracked cache artifacts and evidence files need a repo policy

Files:

- `.moon/cache/states/root/ci-source/lastRun.json`
- `.moon/cache/states/root/ci/lastRun.json`
- `.moon/cache/outputs/*.tar.gz`
- `mutants.out/**`
- `mutants.out.old/**`
- `TRUTH_SERUM_REPORT.md`
- `TRUTH_SERUM_DISTRIBUTED.md`

Problems:

- Tracked Moon cache state currently includes failed run artifacts.
- Tracked cache archives add noisy binary review surface with little code-review value.
- Mutation and review evidence is present, but the repo does not define what should be raw tool output versus curated evidence.
- Some truth-serum style reports claim stronger verification than the tracked evidence in the repo actually shows.

Why this matters:

- Generated cache outputs and weakly governed evidence files make it harder to tell what is canonical truth versus incidental residue.
- Reviewers waste time diffing build artifacts instead of code and contracts.

What to change:

- Decide which generated artifacts are intentional fixtures and which should stop being tracked.
- Remove ephemeral Moon cache outputs and state snapshots from version control unless they are required as golden fixtures.
- If mutation results are intentionally tracked, normalize them into a documented evidence format instead of raw tool sprawl.
- Require report files to cite exact commands, exit codes, and log/artifact locations for every verification claim.

Acceptance criteria:

- The repo has an explicit policy for tracked generated artifacts and evidence files.
- Failed cache state and incidental binary outputs are not committed as routine work products.
- Verification reports cannot claim "verified" without pointing to concrete evidence.

## Crate-by-crate detailed checklist

### `crates/twerk-common`

- Fix `reexec.rs` so child-only setup is done in the child, not the parent.
- Remove or rewrite inaccurate process-group comments.
- Replace logging `.init()` with `try_init()`.
- Decide whether `TracingWriter` should be line-based or chunk-based and implement/document it honestly.
- Make config file type support truthful: real YAML support or TOML-only search paths.
- Unify env-key mapping so `_` and `.` collisions are not lossy.
- Fix fractional duration parsing and stop rounding seconds in unsafe ways.
- Remove docs that overclaim primitive `unmarshal()` support unless the implementation is expanded.
- Fix nested insert behavior so scalar/table conflicts are surfaced instead of silently dropped.

### `crates/twerk-core`

- Remove `Default` and infallible `From` impls from validated IDs.
- Do the same for validated primitive wrappers.
- Remove public unchecked endpoint construction or isolate it behind a private trusted boundary.
- Replace trigger webhook URL string-prefix validation with a validated URL type.
- Fix webhook transport retry so connection failures actually retry.
- Replace expression validation that executes against empty runtime context with syntactic or context-aware validation.
- Make repository pagination typed and safe.
- Enforce referential integrity in in-memory repositories.
- Reject overflowed durations instead of saturating.
- Add `deny_unknown_fields` where fail-open webhook config is currently tolerated unintentionally.

### `crates/twerk-infrastructure`

- Build worker API socket addresses structurally, not from invalid strings.
- Include datastore health in overall health status.
- Make cancellation and timeout stop the actual runtime.
- Implement real Docker stop behavior.
- Stop swallowing Podman execution failures.
- Fix Podman task/container tracking map keys.
- Block path traversal in file injection.
- Stop acking malformed RabbitMQ payloads as success.
- Align role-assignment SQL with schema.
- Align log-part persistence requirements with runtime publishers.
- Persist probe config end to end.
- Make task selection deterministic with explicit ordering.
- Make merged job-log ordering deterministic.
- Ensure heartbeat payloads contain fields required by persistence.

### `crates/twerk-app`

- Make rate limiting shared across requests.
- Centralize engine termination and notification.
- Stop active tasks on worker shutdown.
- Make subscriber loops cancellation-aware.
- Repair shell stop and Podman stop semantics.
- Fix Podman `--workdir` argument placement and container env passing.
- Persist cancelled job state reliably.
- Make node-affined cancellation actually cancel.
- Honor `each.var`.
- Deduplicate scheduled job registration.
- Stop using `blocking_write()` on Tokio locks in async-facing paths.
- Remove duplicated hostenv logic by defining one canonical implementation.

### `crates/twerk-web`

- Replace trigger hand parsing with strict typed request DTOs.
- Reject invalid pagination instead of silently defaulting.
- Publish post-mutation state, not stale objects.
- Strengthen redaction so failure to load parent job does not leak sensitive data.
- Make list-summary redaction use actual secret values, not key-name heuristics only.
- Emit explicit delete semantics for scheduled-job deletion.
- Unify feature flag default behavior with domain logic.
- Tighten handler auth assumptions so protected routes cannot degrade to guest semantics accidentally.
- Unify trigger and generic API error taxonomies.
- Normalize trigger invalid-ID messaging and error-envelope shapes so similar failures look the same across endpoints.

### `crates/twerk-cli`

- Either implement a real migration confirmation flow or remove `--yes` and its docs.
- Make `--json` a consistent success and error contract.
- Replace hand-built JSON strings with typed serialization.
- Decide how logging should work in JSON mode and implement it consistently.
- Fix partial-start cleanup when API server startup fails.
- Use stronger hostname validation.
- Trim and validate banner mode parsing.
- Add explicit timeout and better diagnostics to health checks.
- Fix misleading standalone log messages.

### `crates/twerk-openapi-gen`

- Stop hardcoding workspace root as `../..`.
- Add explicit CLI arguments for workspace root and dry-run behavior.
- Improve failure classification beyond one generic stderr print and exit failure.
- Canonicalize or normalize resolved paths before use.

## Verification plan

These fixes should not land without stronger gates.

### Required repo-wide gates

- Add centralized workspace lint policy in the root `Cargo.toml`.
- Use `workspace.lints` or equivalent shared enforcement instead of piecemeal crate-local lint drift.
- Deny `clippy::unwrap_used`, `clippy::expect_used`, and `clippy::panic` for production code.
- Forbid `unsafe_code` by default and isolate justified exceptions explicitly.
- Add `cargo-deny` policy checks for dependency hygiene and layering violations.
- Add custom Clippy or Dylint rules for validated-type bypass patterns such as `Default` on domain IDs, infallible `From` on validated wrappers, and `unwrap_or_default()` on persisted state parsing.
- Add snapshot coverage for machine-facing JSON, OpenAPI, and error-envelope shape stability.
- Add mutation testing with a documented floor for critical crates.
- Add compile-time boundary hardening where possible with typestates, sealed traits, and stricter newtypes instead of runtime-only checks.
- Add CI checks that verify generated docs are up to date.
- Add CI checks that verify tracked generated artifacts and evidence files comply with repo policy.
- Add contract tests for event payload parity and persistence parity.

### Truth-serum requirements for this plan

- No future update to this document should claim a behavior without either a directly read file reference or executed command evidence.
- Any future implementation report must include command, exit code, and artifact/log pointers for every claim marked fixed, verified, or released.
- No tests should be deleted, weakened, or commented out to satisfy the plan without an explicit defect record explaining why.
- No placeholder prose such as `...`, "rest of code here", or vague "handled elsewhere" statements should remain in implementation notes or follow-up plans.
- Every concrete file path cited in this plan must be verified to exist when the plan is revised.
- Generated docs, generated caches, and generated evidence must be clearly labeled as either canonical artifacts or disposable outputs.

### Required targeted test suites

- Cancellation and timeout tests for shell, Docker, and Podman runtimes.
- API tests that prove malformed input returns stable `400` responses.
- Round-trip persistence tests for task logs, probes, and state enums.
- Doc regression checks for generated website output.
- CLI tests that prove `--json` is valid JSON on both success and failure.
- Property or adversarial tests for validated ID constructors to prevent backsliding.
- Snapshot tests for CLI/API response bodies and error envelopes so contract drift becomes explicit.
- Mutation tests for the highest-risk crates: `twerk-core`, `twerk-app`, `twerk-infrastructure`, and `twerk-web`.
- Regression tests proving cache/report policy and generated-doc policy are enforced in CI.

## Suggested implementation batches

If this work is going to be done incrementally, the cleanest batch order is:

1. Cancellation, timeout, and shutdown truthfulness.
2. Validated ID and newtype hardening.
3. Fail-closed parsing and persistence corruption surfacing.
4. Event payload parity and scheduler semantics.
5. Observability and CLI/API machine-facing contract cleanup.
6. Docs, generated docs, QA assets, and stale test cleanup.

## Bottom line

The main problem is not that this repo lacks types or tests. It is that too many boundaries still allow unchecked construction, silent defaulting, stale event publication, misleading docs, and success-shaped failure behavior.

If the goal is to make this codebase genuinely hard to break, the most important shift is this:

- stop defaulting over invalid state,
- stop reporting success when the runtime truth is failure or cancellation,
- stop letting validated types be bypassed,
- and stop shipping docs/tests that describe a different system than the code actually implements.
