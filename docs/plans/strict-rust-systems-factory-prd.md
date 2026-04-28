# PRD: Twerk Strict Rust Systems Factory

## 1. One-Line Pitch

Twerk turns AI-generated Rust into production-grade Rust by forcing every project through specification, NASA/JPL Holzmann-style architecture discipline, test design, verification gates, dependency discipline, allocation-aware performance benchmarks, and evidence-backed release reports.

## 2. Product Thesis

AI agents are getting fast enough to write entire systems. They are not yet disciplined enough to know when those systems are correct, maintainable, secure, performant, or shippable.

Twerk owns that missing layer for Rust.

The product is not a generic workflow runner. It is not a CI clone. It is not an n8n clone. It is a strict Rust systems factory: an agent-native quality control plane that converts intent into a spec, a spec into an architecture contract, the contract into tests, and the implementation into evidence.

Product sentence:

> Strict Rust, built by agents, verified by Twerk.

Longer positioning:

> Twerk is the Swamp-like agent-native workflow layer for production-grade Rust: typed specs, blessed libraries, strict architecture, real tests, real benchmarks, and release evidence in one repo-native harness.

## 3. Why Now

Agentic software creation is crossing from autocomplete into system assembly. That creates a new bottleneck.

The hard part is no longer producing code. The hard part is proving the code is good.

Current agent workflows fail in predictable ways:

- They code before requirements are clear.
- They skip domain modeling and encode state with strings and booleans.
- They write tests after implementation, often testing mocks instead of behavior.
- They miss failure modes: cancellation, retries, persistence, redaction, concurrency, and recovery.
- They choose random crates instead of battle-tested ecosystem defaults.
- They claim success without running the actual tools.
- They do not preserve evidence that a human or another agent can audit later.

Rust is the best wedge because the ecosystem already values correctness, typed APIs, explicit errors, strict tooling, and release discipline. Twerk makes those standards executable for agents.

## 4. Inspiration And Differentiation

Swamp validates the interface shift: agents can generate typed operational models and deterministic workflows directly in a repo, instead of humans manually building visual workflows first.

Twerk borrows that insight, but applies it to a narrower, higher-trust domain.

Swamp direction:

- Agent-generated typed operational models.
- Workflows around APIs, CLIs, SSH, and external systems.
- Repo-local definitions and versioned data.

Twerk direction:

- Agent-generated Rust system specifications.
- Architecture contracts that define what good Rust means.
- Test plans before implementation.
- Blessed.rs-first dependency governance.
- Verification harnesses that run real Rust tooling.
- Evidence-backed release verdicts.

The comparable primitive is not a generic API model. The comparable primitive is a typed engineering contract for production Rust.

## 5. Target Users

### Primary User: AI-Heavy Rust Builder

A developer using Claude Code, OpenCode, Codex, Cursor, or similar agents to build Rust systems quickly.

They need:

- A way to stop agents from coding too early.
- A strict definition of production-grade Rust.
- Automated gates for correctness, security, performance, and maintainability.
- Machine-readable failures agents can repair.
- A human-readable release report they can trust.

### Secondary User: Rust Maintainer

A maintainer who wants contributors and agents to follow project standards.

They need:

- Repo-local verification profiles.
- Dependency policy enforcement.
- Consistent test and benchmark expectations.
- Evidence for release decisions.

### Tertiary User: Agent Platform Or Software Factory

A system running many agents across many Rust repositories.

They need:

- Standardized spec, architecture, test, and verification contracts.
- JSON APIs for repair loops.
- Durable evidence storage.
- Fleet-level visibility into quality drift.

## 6. Core Promise

If Rust code is built through Twerk, it must pass through:

- intent clarification;
- complete specification;
- architecture review;
- test-plan generation;
- NASA/JPL Holzmann-style implementation constraints;
- strict Rust implementation rules;
- blessed dependency review;
- real command execution;
- coverage, mutation, fuzz, and benchmark evidence where applicable;
- security and dependency gates;
- release-readiness reporting.

Twerk does not accept “looks good.” Twerk requires proof.

## 7. Product Pillars

### 7.1 Spec First

No implementation should start until the spec captures the load-bearing details.

Required spec sections:

- user goal;
- domain model;
- glossary;
- commands and events;
- valid and invalid states;
- input/output contracts;
- invariants;
- preconditions and postconditions;
- error taxonomy;
- persistence requirements;
- async and concurrency boundaries;
- cancellation and timeout semantics;
- security assumptions;
- performance targets;
- observability requirements;
- acceptance criteria;
- non-goals.

If the spec is incomplete, Twerk asks targeted questions instead of generating implementation tasks.

### 7.2 Architecture Contract

Twerk must encode what “good Rust architecture” means.

The default contract requires:

- functional core / imperative shell;
- sync core / async shell;
- Elm-style architecture for interactive app state;
- Scott Wlaschin style domain modeling;
- railway-oriented error flow with typed `Result` pipelines;
- explicit domain types;
- parse, don’t validate;
- typed errors with `thiserror` in core;
- `anyhow` only at application or action boundaries;
- no hidden I/O in pure logic;
- no unbounded concurrency;
- no unnecessary async;
- no blocking work on the async runtime;
- no production panics;
- no `unsafe` by default;
- small functions and files;
- explicit observability.

Strict profile limits:

- max function lines: 25;
- max file lines: 300;
- no production `unwrap`, `expect`, or `panic!`;
- no warnings;
- no silent errors.

### 7.2.1 NASA/JPL Holzmann Performance Doctrine

Twerk-generated Rust should be boring, bounded, inspectable, and fast. Performance is not allowed to come from cleverness that hides risk. Performance must come from well-written Rust: simple control flow, explicit data shapes, bounded resource use, minimal heap pressure, minimal cloning, cache-friendly layouts, and measured hot paths.

Holzmann-style rules for generated production code:

- simple control flow only;
- all loops bounded by data, count, timeout, or explicit cancellation;
- no unbounded allocation in hot paths;
- no hidden recursion or unbounded async fan-out;
- functions target 25 lines or less;
- every fallible result is handled;
- no production `unwrap`, `expect`, `panic!`, or silent fallback;
- no macro cleverness hiding domain logic;
- no abstraction stack with only one real implementation;
- warnings, lint failures, and missing evidence are treated as failures.

Fast-by-construction Rust requirements:

- parse at boundaries into trusted types;
- prefer borrowed data, `Cow`, `Bytes`, `SmallVec`, and iterator pipelines before heap-heavy structures;
- minimize heap allocation on hot paths;
- avoid cloning unless ownership or lifetime boundaries require it;
- prefer references, borrowed views, `Arc<str>`, `Bytes`, or copy-small value types where they make ownership explicit;
- keep data layouts compact and cache-friendly;
- avoid intermediate `Vec`/`String` allocations in transformation pipelines;
- stream command output instead of buffering whole logs;
- use Rayon for CPU-bound pure analysis;
- use bounded Tokio concurrency only for I/O-bound work;
- never block the async runtime with CPU or sync I/O;
- pre-size buffers when sizes are known;
- measure allocation counts, throughput, latency percentiles, and memory ceilings for hot paths;
- store compact evidence indexes for UI/report queries;
- benchmark every hot path before claiming it is fast;
- reject performance regressions beyond the configured threshold.

The target is not merely safe Rust. The target is maximum safe performance: production Rust that a ruthless NASA-style reviewer can reason about quickly, that avoids wasteful allocation and cloning, and that a benchmark can prove under load.

### 7.2.2 Async Rust Doctrine

Async Rust must be used only where it earns its cost. Twerk-generated code should treat async as the I/O shell, not the domain model.

Rules:

- domain crates must not depend on `tokio`, `futures`, `async-std`, or runtime-specific crates;
- pure calculations must be synchronous and runtime-free;
- `async fn` is forbidden unless it contains a real `.await`;
- CPU-bound work uses sync functions plus Rayon, not async tasks;
- blocking I/O uses `spawn_blocking` or a dedicated thread, never a runtime worker;
- every concurrent operation has an explicit bound;
- use stream combinators or `JoinSet`/`FuturesUnordered`, not manual `Vec<JoinHandle>` bookkeeping;
- `tokio::spawn` lives at the edge only: handlers, main, infrastructure adapters;
- spawned tasks must propagate errors and inherit tracing spans;
- cancellation must leave the system in a valid recoverable state;
- async hot paths require throughput and concurrency-scaling benchmarks.

The default architecture is sync core, async shell. If sync code is simpler and correct, Twerk must prefer sync code.

### 7.2.3 Elm And Railway Architecture

Twerk-generated applications should make state transitions obvious, typed, and testable. For interactive systems, especially the Rust frontend, Twerk should force an Elm-style architecture or a close equivalent.

Elm-style UI/application rules:

- `Model` owns all renderable state;
- `Msg` is a closed enum of every user, network, timer, and system event;
- `update(model, msg)` is the only state transition function;
- `view(model)` is a pure projection of state into UI;
- side effects are returned as explicit commands/effects, never hidden inside view or domain code;
- impossible UI states are represented as enum variants, not boolean/option soup;
- state transitions are covered by table-driven tests.

Scott Wlaschin type-safety rules:

- make illegal states unrepresentable;
- parse, don't validate;
- model workflows as explicit typed transitions;
- use semantic newtypes for IDs, names, paths, versions, thresholds, durations, and percentages;
- avoid boolean parameters in domain APIs;
- avoid `Option` fields that secretly encode lifecycle state;
- use enums/sum types for domain alternatives and lifecycle phases;
- keep primitive types at I/O boundaries, not in the core domain.

Railway-oriented programming rules:

- expected failures are `Result<T, DomainError>`;
- domain errors are explicit enums, not strings;
- transformations compose through `map`, `and_then`, `map_err`, and small named functions;
- validation/parsing happens once at the boundary, producing trusted types;
- the happy path stays linear;
- error enrichment happens at shell boundaries with context;
- no silent fallback, no swallowed errors, no panic-based control flow.

The result should feel like Scott Wlaschin's type-driven design applied to Rust: domain concepts in the type system, workflows as compile-checked transitions, and failures flowing explicitly down the railway.

### 7.2.4 Token-Efficient Agent Contract

Twerk should help agents produce the highest-quality Rust with the smallest useful context.

Agent-facing output must be concise, structured, and repair-oriented:

- return stable failure IDs instead of dumping entire logs into prompts;
- summarize the first blocking failure before secondary noise;
- link evidence files instead of repeating large stdout/stderr bodies;
- provide exact next commands and exact files to inspect;
- emit JSON that agents can parse without prose scraping;
- preserve full evidence on disk while sending only minimal repair context to the agent;
- make retries targeted by gate, failure ID, or affected file set.

The product should optimize for low-token repair loops without losing auditability: short responses for agents, complete evidence on disk for humans.

### 7.3 Test Contract

Tests are executable specifications, not implementation confirmation.

The test contract must include:

- Given/When/Then behavior scenarios;
- unit tests for pure calculations;
- integration tests for adapters and boundaries;
- property tests for parsers, state machines, and invariants;
- fuzz targets for untrusted input;
- mutation-testing expectations;
- benchmark scenarios for hot paths;
- crash/restart scenarios when persistence exists;
- API contract tests when HTTP/OpenAPI exists.

### 7.4 Blessed-First Dependency Policy

Twerk should push agents toward battle-tested Rust libraries before novelty.

Primary external source:

- Blessed.rs, the curated Rust crate guide.

Policy:

- Prefer Blessed.rs crates for solved common problems.
- Require explicit justification for non-blessed crates.
- Reject denied dependencies before considering blessed status.
- Block unresolved high/critical RustSec advisories by default.
- Require license, maintenance, and transitive-risk review for security-sensitive dependencies.

Default recommendations:

- errors: `thiserror`, `anyhow`, `color-eyre`;
- telemetry: `tracing`, `tracing-subscriber`;
- serialization: `serde`, `serde_json`, `toml`, `postcard`;
- async: `tokio`, `futures`;
- HTTP: `axum`, `reqwest`, `http`;
- CLI: `clap`, `ignore`, `globset`, `directories`, `indicatif`;
- concurrency: `rayon`, `dashmap`, `arc-swap`, `parking_lot`, `crossbeam-channel`, `flume`;
- testing and tooling: `cargo-nextest`, `insta`, `criterion`, `divan`, `hyperfine`, `cargo-deny`, `cargo-audit`, `cargo-semver-checks`;
- security: `zeroize`, `subtle`, RustCrypto crates, `rustls`.

Twerk-specific additions:

- `proptest` for property testing;
- `cargo-fuzz` for fuzzing;
- `cargo-mutants` for mutation testing;
- `loom` for concurrency model checking;
- `kani` for bounded formal verification;
- `fjall` for embedded Twerk evidence storage.

### 7.4.1 Battle-Tested Runtime And Rust Frontend Stack

Twerk should use boring, dominant, battle-tested infrastructure. Novelty is reserved for the product insight, not the stack.

Backend defaults:

- async runtime: `tokio`;
- HTTP server: `axum`;
- middleware: `tower`;
- telemetry: `tracing`, `tracing-subscriber`, OpenTelemetry-compatible output;
- concurrency: `rayon` for CPU-bound pure work, `tokio` for I/O, `flume` or Tokio channels for bounded message passing;
- storage: `fjall` for embedded evidence/state where Twerk needs a single-binary local store;
- serialization: `serde`, `serde_json`, `toml`, `postcard` where appropriate;
- CLI: `clap`.

Frontend default:

- Rust only;
- not Yew;
- not Dioxus;
- use Leptos for the browser UI because it is the strongest Rust-first web choice for a serious app today;
- compile to WASM and serve the UI from the Rust/Axum binary;
- avoid a Node runtime in production;
- keep frontend state typed with Rust structs shared from the API contract where feasible;
- structure interactive UI state with Elm-style `Model`, `Msg`, `update`, `view`, and explicit effects.

Frontend libraries:

- web framework: `leptos`;
- routing: `leptos_router`;
- server/API types: shared Rust crates plus `serde`;
- browser interop: `wasm-bindgen`, `web-sys`, `js-sys` only at thin boundaries;
- styling: CSS/Tailwind generated at build time, but no TypeScript application layer;
- graph/evidence canvas: Rust-rendered SVG/canvas via Leptos components and thin `web-sys` bindings;
- forms: typed Rust form models with boundary validation;
- logs/terminal: custom Rust/WASM log viewer first, xterm-compatible interop only if required;
- charts: Rust/WASM SVG charts first, JavaScript chart interop only behind an explicit adapter if the Rust option fails.

Frontend rule: Rust stays end-to-end. The UI may use browser APIs through narrow bindings, but product logic, state models, API contracts, evidence rendering, and validation stay in Rust.

Frontend state rule: Leptos components render typed state; they do not become the state model. Application behavior lives in compile-checked messages, pure update functions, and typed effect descriptions.

### 7.5 Verification Harness

Twerk must run real tools and preserve evidence.

MVP gates:

| Gate | Action Id | Classification | MVP Behavior |
|---|---|---|---|
| Format | `rust.fmt` | MandatoryBlocking | Run `cargo fmt --check`; fail closed on missing tool or non-zero exit |
| Strict clippy | `rust.clippy` | MandatoryBlocking | Run strict clippy deny set; fail closed on diagnostics |
| Tests | `rust.nextest` | MandatoryBlocking | Run `cargo nextest run`; fail closed on failures or missing runner |
| Dependency audit | `rust.audit` | MandatoryBlocking | Run `cargo audit`; fail closed on high/critical advisories |
| Dependency policy | `rust.deny` | MandatoryBlocking | Run `cargo deny`; fail closed on denied deps/licenses/advisories |
| Coverage | `rust.coverage` | MandatoryEvidenceMayBeMissing | Missing evidence until threshold report exists |
| Mutation | `rust.mutants` | MandatoryEvidenceMayBeMissing | Missing evidence until kill-rate report exists |
| Fuzzing | `rust.fuzz` | MandatoryEvidenceMayBeMissing | Missing evidence until smoke fuzz report exists |
| Benchmarks | `rust.bench` | MandatoryEvidenceMayBeMissing | Missing evidence until baseline/report exists |
| Release report | `twerk.report.release` | MandatoryBlocking | Render markdown and JSON report from stable read models |

Advisory or later gates:

- `miri`;
- `loom`;
- `kani`;
- docs/API drift;
- release artifact checks.

## 8. User Journeys

### 8.1 Create A New Rust System

1. User describes what they want.
2. Twerk interrogates missing requirements.
3. Twerk writes `architecture-spec.md`.
4. Twerk writes `test-plan.md`.
5. Twerk generates `twerk.yaml`.
6. Agent implements the system.
7. Twerk runs verification gates.
8. Agent receives machine-readable failures.
9. Agent repairs the system.
10. Twerk produces release evidence.

### 8.2 Harden Existing Rust Repo

1. User runs `twerk harness init --profile rust/strict-system`.
2. Twerk scans repo structure and tooling.
3. Twerk recommends missing crates, tests, lints, benchmarks, and harness steps.
4. Twerk writes a repo-specific verification harness.
5. User or agent runs `twerk verify --json`.
6. Twerk stores evidence and produces a ship / do-not-ship report.

### 8.3 Agent Repair Loop

1. Agent runs `twerk verify --json`.
2. Twerk returns structured failures.
3. Agent calls `twerk explain <failure-id> --json`.
4. Agent edits code.
5. Agent reruns the relevant gate.
6. Twerk appends evidence.
7. Full verification runs before completion.

## 9. MVP Scope

### 9.1 Required CLI

```bash
twerk spec init
twerk spec interrogate
twerk spec validate
twerk arch review
twerk tests plan
twerk harness init --profile rust/strict-system
twerk verify
twerk verify --json
twerk explain <failure-id>
twerk report
twerk blessed list
twerk blessed explain <crate>
twerk blessed check
```

### 9.2 Generated Files

```text
twerk.yaml
architecture-spec.md
test-plan.md
.twerk/runs/
.twerk/evidence/
docs/release-evidence/
```

`.twerk/evidence/` stores machine evidence records.

`docs/release-evidence/` stores human-reviewable markdown and JSON release reports derived from the evidence.

### 9.3 Harness YAML

```yaml
version: twerk/v1
name: rust-strict-system
profile: rust/strict-system

spec:
  require:
    domain_model: true
    invariants: true
    error_taxonomy: true
    acceptance_criteria: true
    performance_targets: true

architecture:
  enforce:
    functional_core: true
    async_shell: true
    blessed_first_dependencies: true
    no_stringly_domain: true
    no_hidden_io: true
    max_file_lines: 300
    max_function_lines: 25

rust:
  deny:
    - warnings
    - clippy::unwrap_used
    - clippy::expect_used
    - clippy::panic
    - unsafe_code

tests:
  require:
    bdd: true
    property: true
    fuzz_for_parsers: true
    benchmarks_for_hot_paths: true

steps:
  fmt:
    uses: rust.fmt
    with:
      check: true

  clippy:
    uses: rust.clippy
    needs: [fmt]
    with:
      workspace: true
      all_targets: true
      deny: "{{ rust.deny }}"

  tests:
    uses: rust.nextest
    needs: [clippy]
    with:
      workspace: true

  audit:
    uses: rust.audit
    needs: [tests]

  deny:
    uses: rust.deny
    needs: [tests]

  coverage:
    uses: rust.coverage
    needs: [tests]
    with:
      fail_under_lines: 80

  mutants:
    uses: rust.mutants
    needs: [tests]
    with:
      minimum_kill_rate: 90

  fuzz:
    uses: rust.fuzz
    needs: [tests]
    with:
      smoke_seconds: 60

  benchmarks:
    uses: rust.bench
    needs: [tests]
    with:
      compare_to: baseline
      max_regression_percent: 10

  report:
    uses: twerk.report.release
    needs: [audit, deny, coverage, mutants, fuzz, benchmarks]
```

## 10. Core Domain Model

### 10.1 Aggregates

| Aggregate | Required Fields | Invariants | Failure Modes |
|---|---|---|---|
| `Spec` | goal, domain model, states, invariants, error taxonomy, non-goals, acceptance scenarios | Cannot become `SpecComplete` with missing required sections | `invalid_spec`, `unsupported_workspace` |
| `ArchitectureContract` | rules, dependency policy, async policy, storage policy, command policy | Every blocking rule has severity, rationale, and remediation | `invalid_spec`, `dependency_not_blessed`, `dependency_denied` |
| `TestContract` | Given/When/Then scenarios, error tests, edge tests, property/fuzz/benchmark obligations | Every invariant and error kind maps to at least one test obligation | `invalid_spec` |
| `HarnessProfile` | version, profile id, gate graph, action refs | Supported version, known actions, acyclic gate graph | `invalid_harness_yaml` |
| `ActionDefinition` | action id, argv template, schemas, timeout, env allowlist, shell policy | Argv by default, explicit timeout, redacted outputs | `invalid_harness_yaml`, `unsupported_workspace` |
| `ActionRun` | run id, action id, status, timing, exit model, evidence ids | Terminal status requires evidence or blocking evidence failure | `missing_tool`, `command_failed`, `evidence_write_failed`, `redaction_failed` |
| `Finding` | id, severity, source, message, evidence ids, remediation | `Blocking` finding forces `DoNotShip` | `command_failed`, `dependency_denied`, `dependency_not_blessed` |
| `Recommendation` | id, category, target, rationale, source, risk, next command | Cannot downgrade blocking findings | `internal_bug` |
| `ReleaseReport` | id, run id, verdict, gate summaries, findings, missing evidence, dependency summary | `Ship` requires passing blocking gates and mandatory evidence | `evidence_write_failed`, `internal_bug` |

### 10.2 State Machines

Spec state:

```text
Draft -> NeedsClarification -> SpecComplete -> ArchitectureReviewed -> TestPlanApproved -> ReadyForImplementation
```

Gate status:

```text
Planned -> Running -> Passed | Failed | Blocked | MissingEvidence | NotApplicableWithJustification
```

Release verdict:

```text
Unknown -> DoNotShip | HumanDecisionRequired | Ship
```

Verdict rules:

- `Ship`: every blocking gate passes, mandatory evidence exists, and no blocking findings remain.
- `DoNotShip`: any blocking gate fails, any dependency is denied, evidence redaction fails, or command execution security is violated.
- `HumanDecisionRequired`: only advisory failures remain or a justified exception awaits approval.
- `Unknown`: no complete verification run exists.

## 11. Evidence Model

Every action writes an immutable `EvidenceRecord`.

Required fields:

- evidence id;
- project id;
- harness run id;
- action id;
- command;
- arguments;
- working directory;
- environment allowlist;
- start time;
- end time;
- duration;
- exit code;
- stdout path;
- stderr path;
- parsed findings;
- artifact paths;
- git revision;
- tool versions;
- host summary.

Evidence invariants:

- Evidence is append-only.
- Evidence is checksummed.
- Secrets are redacted before persistence.
- Redaction failure blocks release.
- Output is streamed in bounded chunks.
- Evidence paths are canonicalized under `.twerk/evidence/`.
- A release report cannot produce `Ship` unless every blocking gate has evidence.

## 12. JSON API Contract

Success shape:

```json
{
  "type": "success",
  "command": "twerk verify",
  "version": "0.1.0",
  "data": {},
  "evidence_ids": []
}
```

Error shape:

```json
{
  "type": "error",
  "command": "twerk verify",
  "error": {
    "kind": "invalid_spec|invalid_harness_yaml|missing_tool|command_failed|dependency_denied|dependency_not_blessed|redaction_failed|evidence_write_failed|unsupported_workspace|invalid_failure_id|internal_bug",
    "message": "human readable message",
    "failure_id": "failure_01h...",
    "recommended_next_command": "twerk explain failure_01h... --json"
  }
}
```

Exit code policy:

| ErrorKind | Exit Code |
|---|---:|
| `command_failed` | 1 |
| `invalid_spec` | 2 |
| `invalid_harness_yaml` | 2 |
| `unsupported_workspace` | 2 |
| `invalid_failure_id` | 2 |
| `missing_tool` | 3 |
| `dependency_denied` | 4 |
| `dependency_not_blessed` | 4 |
| `redaction_failed` | 5 |
| `evidence_write_failed` | 5 |
| `internal_bug` | 70 |

## 13. Acceptance Criteria

### 13.1 MVP Acceptance

Given a Rust repository, when the user runs:

```bash
twerk harness init --profile rust/strict-system
twerk verify --json
```

Then Twerk must produce:

- `twerk.yaml` containing `version: twerk/v1`, `profile: rust/strict-system`, and `max_function_lines: 25`;
- `.twerk/evidence/` records for each attempted gate;
- `docs/release-evidence/<run-id>.md`;
- `docs/release-evidence/<run-id>.json`;
- JSON with `type`, `command`, `version`, `data.run_id`, `data.verdict`, `data.gates[]`, and `evidence_ids[]`;
- exit code `3` and `missing_tool` when a required tool is unavailable;
- non-zero exit, `DoNotShip`, and at least one `Blocking` finding when mandatory gates fail;
- exit code `0` only when every `MandatoryBlocking` gate passes and every mandatory evidence gate has evidence.

### 13.2 Dependency Acceptance

Given an unexplained dependency that is not blessed, when `twerk blessed check` runs, then Twerk must return exit code `4`, error kind `dependency_not_blessed`, and a finding naming the crate plus required justification fields.

### 13.3 Spec Acceptance

Given an incomplete user request, when spec interrogation runs, Twerk must ask targeted questions instead of generating implementation tasks.

Interrogation must cover:

- domain nouns;
- state transitions;
- invalid states;
- error modes;
- persistence;
- concurrency;
- security;
- performance;
- acceptance tests.

If any category is missing, spec state must be `NeedsClarification`, and `ReadyForImplementation` is forbidden.

### 13.4 Architecture Acceptance

Given an architecture spec, Twerk must reject implementation readiness if:

- domain models are stringly typed;
- async code leaks into pure domain logic;
- error taxonomy is missing;
- persistence semantics are vague;
- cancellation behavior is unspecified for async or process work;
- no test strategy exists for critical invariants.

## 14. Roadmap

### Phase 0: Product Pivot Artifact

- Write PRD.
- Update README positioning.
- Define `rust/strict-system` profile.
- Define harness YAML schema.

### Phase 1: Verification Harness MVP

- Parse `twerk.yaml` version/profile header.
- Reject unknown harness versions.
- Reject unknown action ids.
- Implement `twerk harness init --profile rust/strict-system`.
- Capture evidence for `rust.fmt`.
- Capture evidence for `rust.clippy`.
- Capture evidence for `rust.nextest`.
- Capture evidence for `rust.audit`.
- Capture evidence for `rust.deny`.
- Classify `rust.coverage`, `rust.mutants`, `rust.fuzz`, and `rust.bench` as `MissingEvidence` until reports exist.
- Emit stable JSON command output.
- Generate markdown and JSON release reports.

### Phase 2: Spec And Architecture Loop

- Add spec interrogation template.
- Validate required spec sections.
- Add architecture contract schema.
- Reject stringly typed domain models.
- Reject missing error taxonomy.
- Reject unbounded async/process work.
- Generate Martin Fowler test-plan skeleton.
- Require test-plan approval before implementation readiness.

### Phase 3: Evidence Cockpit

- Define `RunSummary`.
- Define `GateSummary`.
- Define `FindingSummary`.
- Define `ReleaseReportView`.
- Add run list.
- Add gate timeline.
- Add evidence viewer.
- Add findings and recommendations.
- Add release report page.

### Phase 4: Recommendation Engine

- Add crate and pattern recommendation rules.
- Add missing-test detection.
- Add benchmark/fuzz/property-test suggestions.
- Add repair guidance for common failures.

### Phase 5: Agent-Native Builder Loop

- Add `twerk explain`.
- Add targeted rerun commands.
- Add structured failure IDs.
- Add skill files for Claude, OpenCode, Codex, and Cursor.

## 15. Non-Goals

Initial versions must not become:

- a generic workflow automation tool;
- a full CI/CD platform;
- a distributed workflow engine;
- a Temporal replacement;
- a full AWS Step Functions compatible runtime;
- a multi-language product;
- a marketplace-first extension platform;
- an enterprise RBAC product;
- a formal verification IDE.

Twerk may run workflows internally, but the product is strict Rust system construction and verification.

## 16. Success Metrics

Product metrics:

- time from prompt to verified Rust skeleton;
- percentage of generated projects with passing strict gates;
- number of issues caught before implementation;
- number of findings fixed by agent repair loops;
- release reports generated per repo;
- repeat usage per repository.

Quality metrics:

- zero production `unwrap`/`expect`/`panic` in generated code;
- strict clippy pass rate;
- test pass rate;
- mutation score where configured;
- coverage where configured;
- benchmark regression rate;
- RustSec advisory count.

Adoption metrics:

- Rust projects initialized with Twerk harness;
- verification packs installed;
- agent integrations using JSON interface;
- published reusable harness/action blocks.

## 17. Risks

### Risk: Too Broad

Mitigation: own Rust first. Do not add other languages until Rust is excellent.

### Risk: Verification Without Building

Mitigation: product must include spec and architecture generation, not just CI-style checks.

### Risk: UI Before Proof

Mitigation: build evidence capture and CLI first. UI renders existing evidence.

### Risk: False Confidence

Mitigation: release reports must distinguish passed gates from missing evidence.

### Risk: Agent Hallucination

Mitigation: every action, profile, schema, and recommendation must be machine-readable and validated.

## 18. Canonical Bead Seeds

`architecture-spec.md` Section 17 is the only canonical source for decomposition seed beads. This PRD intentionally does not maintain a competing task list.

The bead pipeline must decompose from the architecture spec so each bead has one behavior, one command/API/file contract, one acceptance scenario, explicit dependencies, and a verification command.

## 19. Final Product Line

Twerk is not just a workflow runner.

Twerk is the strict Rust systems factory for the agentic software era.

It owns the quality contract:

```text
intent -> spec -> architecture -> tests -> implementation -> verification -> evidence
```

Agents can write code. Twerk decides if it is real.
