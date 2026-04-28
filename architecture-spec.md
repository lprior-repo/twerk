# Architecture Spec: Twerk Strict Rust Systems Factory

## 1. Mission

Twerk becomes a strict Rust systems factory for the agentic software era. It converts user intent into a complete engineering contract, forces AI agents through specification and test design before implementation, executes a supervised GoMaster-style delivery pipeline, and stores evidence proving whether the produced Rust system is shippable.

Product contract:

```text
intent -> interrogation -> specification -> architecture contract -> test contract -> implementation -> verification -> evidence -> release verdict
```

Twerk does not optimize for generic automation first. It optimizes for one high-value promise:

> Agents can write Rust code. Twerk decides if it is real.

## 2. Source Artifacts

Canonical product source:

- `docs/plans/strict-rust-systems-factory-prd.md`

External doctrine sources:

- Blessed.rs curated Rust crate directory.
- Gerard J. Holzmann's Power of Ten rules, adapted to safe Rust.
- Martin Fowler test and refactoring principles: tests as executable specifications, Given/When/Then scenarios, behavior over implementation detail, continuous integration, evolutionary design, and ruthless refactoring under tests.
- GoMasterOrchestrator 15-state bead delivery pipeline.
- Functional Rust doctrine: Data -> Calculations -> Actions, strict domain types, no production `unwrap`/`expect`/`panic`, no silent errors, and explicit side-effect boundaries.

## 2.1 Contract Parity Matrix

| Requirement ID | PRD Source | Architecture Contract | Acceptance Obligation | Bead Target |
|---|---|---|---|---|
| REQ-SPEC-001 | PRD 7.1, 8.1, 14.2 | Sections 4.2, 9.2, 16.3 | `twerk spec interrogate` must refuse implementation readiness until required domain, invariant, error, performance, security, and acceptance fields exist. | Spec interrogation engine |
| REQ-ARCH-001 | PRD 7.2, 14.3 | Sections 4, 5, 6, 9, 16.3 | `twerk arch review` must emit blocking findings for stringly domain models, async leakage, missing error taxonomy, vague persistence, missing cancellation, or absent invariant tests. | Architecture review engine |
| REQ-TEST-001 | PRD 7.3, 14.2 | Sections 6, 10, 16.3 | `twerk tests plan` must create Given/When/Then, error, edge, property/fuzz/benchmark obligations before implementation. | Martin Fowler test planner |
| REQ-BLESSED-001 | PRD 7.4.1 | Sections 7, 15, 16.2 | `twerk blessed check` must reject unexplained non-blessed dependencies with `dependency_not_blessed`. | Blessed policy engine |
| REQ-HARNESS-001 | PRD 9.1, 10 | Sections 11, 15, 16.2 | `twerk harness init --profile rust/strict-system` must generate valid `twerk.yaml` with strict profile defaults. | Harness YAML schema/parser |
| REQ-EVIDENCE-001 | PRD 7.5, 13, 14.1 | Sections 12, 12.1, 16.2 | Every action run must persist redacted, checksummed evidence or fail closed. | Evidence model/storage |
| REQ-GATE-001 | PRD 9.3 | Section 11.1 | Gate missing/failure semantics must be deterministic and reflected in exit code, finding severity, and release verdict. | Verification runner |
| REQ-CLI-001 | PRD 9.1, 13 | Sections 15, 15.1 | Every MVP command must emit stable JSON under `--json` and map failures to documented exit codes. | JSON CLI contracts |
| REQ-GO-001 | PRD 16, 18 | Section 10 | Every implementation bead must pass all 15 GoMaster states with artifact evidence. | GoMaster artifact integration |
| REQ-REPORT-001 | PRD 14, 15 | Sections 9.2, 16.2 | `twerk report` must emit `Ship`, `DoNotShip`, or `HumanDecisionRequired` with linked evidence. | Release report generator |

## 3. Scope

### 3.1 In Scope

- Rust-only strict systems factory.
- Spec interrogation for incomplete user intent.
- Architecture contract generation and review.
- Martin Fowler style test-plan generation.
- Blessed-first dependency recommendation and enforcement.
- Twerk harness YAML for verification profiles.
- Rust action registry for verification tools.
- Evidence storage for every command run.
- JSON APIs for AI repair loops.
- Release report generation.
- Bead lifecycle integration through `bd`.
- GoMaster-style delivery gates for every implementation bead.

### 3.2 Out Of Scope

- Generic n8n clone.
- Multi-language generator.
- Distributed workflow engine.
- Temporal replacement.
- Full CI/CD replacement.
- Marketplace-first ecosystem.
- Enterprise RBAC product.
- Formal proof for arbitrary Rust code.
- Full AWS Step Functions compatibility.

## 4. Architectural Principles

### 4.1 Pure Rust Only

The generated systems are Rust systems. Twerk may invoke external Rust tooling, but system implementation guidance, contracts, and blessed stacks target Rust first.

### 4.2 Spec Before Code

No implementation bead is ready until the spec includes domain model, invariants, error taxonomy, state transitions, acceptance criteria, performance budgets, and verification obligations.

### 4.3 Tests Before Implementation

Every implementation bead must flow through contract and test-plan states before code. Tests specify behavior, not implementation details.

### 4.4 Blessed Libraries Before Novelty

Agents must choose battle-tested crates from Blessed.rs or from a project-local blessed override list. Non-blessed dependencies require explicit justification and black-hat approval.

### 4.5 Evidence Or It Did Not Happen

Every gate must write durable evidence: command, working directory, environment summary, stdout, stderr, exit code, timestamp, git revision, and parsed findings.

### 4.6 Fast By Construction

Hot paths use zero-copy parsing, bounded allocations, streaming command capture, parallel pure analysis with Rayon, bounded async concurrency, and incremental evidence updates.

### 4.7 Small, Typed, Boring Code

Prefer explicit domain types, small functions, small files, deterministic state machines, and linear control flow. Cleverness is a defect unless benchmarked and justified.

## 5. Holzmann Power Of Ten Adaptation

Twerk-generated production Rust and Twerk's own critical path must follow these adapted rules:

1. **Simple control flow only**: no hidden control transfer, no async spaghetti, no state encoded by `Option` soup.
2. **Bound all loops**: command polling, retries, log capture, query scans, and repair loops must have explicit bounds or timeouts.
3. **No unbounded allocation in hot paths**: pre-size buffers where possible; stream command output; use `SmallVec`, `Bytes`, `Cow`, or iterators when appropriate.
4. **Small functions**: strict profile target is 25 lines. Any function over 25 lines is a review finding; any exception requires written justification and evidence.
5. **Assertions and contracts**: critical calculations must expose preconditions, postconditions, or debug assertions where meaningful.
6. **Narrow scope**: variables, permissions, handles, secrets, locks, and tasks live in the smallest possible scope.
7. **Check every fallible result**: no ignored `Result`, no swallowed `JoinError`, no silent fallback.
8. **No macro/preprocessor cleverness as architecture**: macros may remove boilerplate but cannot hide domain logic or side effects.
9. **Limit indirection**: avoid abstraction stacks with one implementation; prefer direct data structures and explicit ports where they buy testability.
10. **Warnings are failures**: strict linting, formatting, dependency policy, tests, and static analysis are mandatory gates.

## 6. Martin Fowler Doctrine

Twerk must encode Fowler-style engineering discipline:

- Tests are executable specifications.
- Test names describe behavior in domain language.
- Given/When/Then scenarios are required for user-visible behavior.
- Refactoring is safe only under a green behavioral test suite.
- CI is not a script collection; it is a continuously executed quality contract.
- Architecture evolves through small verified changes, not speculative frameworks.
- Mocks are used at architectural boundaries, not as a substitute for proving behavior.
- Test suites must fail if the behavior they claim to cover is deleted.
- Release readiness requires real integration evidence, not only unit tests.

## 7. Blessed Library Policy

### 7.1 Source Of Truth

Blessed.rs is the primary curated external source for Rust crate recommendations.

Twerk also maintains project-local policy files:

```text
blessed.toml
blessed-overrides.toml
blessed-deny.toml
```

### 7.2 Dependency Decision Rules

- If Blessed.rs recommends a crate for a solved problem, generated specs and code must prefer that crate.
- If a non-blessed crate is selected, the architecture contract must include a dependency justification.
- If a dependency is security-critical, Twerk must require `cargo audit`, `cargo deny`, license review, and maintenance review evidence.
- If multiple Blessed crates are available, Twerk must choose based on constraints, not popularity alone.
- Project-local deny policy overrides Blessed recommendations when RustSec, license, maintenance, or product constraints require it.

### 7.2.1 Dependency Governance Contract

Decision order is mandatory:

1. `blessed-deny.toml` denies take precedence over every recommendation.
2. Active RustSec advisories block by default unless a written unreachable-advisory exception exists.
3. License policy blocks incompatible licenses before technical evaluation.
4. Project-local `blessed-overrides.toml` can bless newer or internal standards.
5. Pinned Blessed.rs snapshot recommendations are preferred for solved common problems.
6. Non-blessed dependencies require `DependencyJustification` and black-hat approval.

Default license policy:

- Allowed: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MPL-2.0.
- Conditional: Unicode-DFS, Zlib, OpenSSL, LGPL only with explicit product approval.
- Denied by default: AGPL, GPL, unknown licenses, custom licenses without review.

Maintenance policy:

- A dependency with no release or meaningful maintenance activity in 24 months is `maintenance_stale` unless it is stable, tiny, and explicitly approved.
- A dependency with unresolved critical/high RustSec advisories is `security_blocked`.
- A dependency adding more than 25 transitive crates must include transitive-risk justification unless it is already part of the blessed stack for that use case.

`DependencyJustification` fields:

- crate name;
- version requirement;
- use case;
- Blessed alternatives considered;
- why alternatives are insufficient;
- license;
- RustSec status;
- maintenance evidence;
- transitive dependency count;
- approval status.

### 7.3 Initial Blessed Stack

Core:

- errors: `thiserror`, `anyhow`, `color-eyre`;
- logging: `tracing`, `tracing-subscriber`;
- serialization: `serde`, `serde_json`, `toml`, `postcard`;
- collections/utilities: `itertools`, `smallvec`, `arrayvec`, `indexmap`, `bytes`;
- CLI: `clap`, `ignore`, `globset`, `directories`, `indicatif`, `inquire`;
- async/networking: `tokio`, `futures`, `axum`, `reqwest`, `http`, `hyper`, `tonic`;
- concurrency: `rayon`, `dashmap`, `arc-swap`, `parking_lot`, `crossbeam-channel`, `flume`;
- crypto/security: `zeroize`, `subtle`, `rustls`, RustCrypto crates;
- testing/tooling: `cargo-nextest`, `insta`, `criterion`, `divan`, `hyperfine`, `cargo-audit`, `cargo-deny`, `cargo-semver-checks`, `cargo-expand`;
- UI: `dioxus-web`, `dioxus-desktop`, `tauri`, `egui`, `iced`.

Twerk-specific additions:

- property testing: `proptest`;
- fuzzing: `cargo-fuzz`;
- mutation testing: `cargo-mutants`;
- formal and concurrency checks: `kani`, `loom`, `miri`;
- embedded evidence store: `fjall`.

## 8. System Architecture

### 8.1 Components

```text
Twerk CLI
  parses commands, emits stable JSON, runs local workflows

Spec Engine
  interrogates intent, validates completeness, writes architecture-spec.md

Architecture Reviewer
  applies DDD, Holzmann, Blessed, Fowler, async, and black-hat rules

Test Planner
  writes Martin Fowler test plans and proof obligations

Harness Engine
  parses twerk.yaml, resolves profiles, schedules verification actions

Rust Action Registry
  typed action definitions for fmt, clippy, nextest, audit, deny, coverage, mutants, fuzz, bench, and release report generation

Evidence Store
  durable command evidence, reports, artifacts, and parsed findings

Recommendation Engine
  maps findings to blessed crates, patterns, tests, and architecture repairs

UI Evidence Cockpit
  post-MVP renderer for stable read models; it must not drive MVP storage or execution semantics

Bead Pipeline Adapter
  converts architecture specs into bd beads and enforces GoMaster delivery states
```

### 8.2 Data -> Calculations -> Actions

Data:

- typed specs;
- architecture contracts;
- test contracts;
- harness profiles;
- action definitions;
- evidence records;
- findings;
- recommendations;
- release reports.

Calculations:

- spec completeness scoring;
- architecture rule evaluation;
- dependency blessing decisions;
- harness graph validation;
- finding severity aggregation;
- release verdict calculation.

Actions:

- filesystem reads/writes;
- command execution;
- evidence persistence;
- UI/API serving;
- `bd` issue creation;
- agent skill generation.

## 9. Domain Model

### 9.1 Core Types

- `Project`: parsed Rust repository under Twerk governance.
- `Intent`: raw user goal plus clarification history.
- `Spec`: complete requirements contract.
- `ArchitectureContract`: structural and dependency contract.
- `TestContract`: proof obligations.
- `HarnessProfile`: reusable verification profile, such as `rust/strict-system`.
- `HarnessRun`: one execution of a harness profile.
- `ActionDefinition`: typed command or analysis step.
- `ActionRun`: one execution of an action.
- `EvidenceRecord`: immutable command/artifact evidence.
- `Finding`: violation or risk with severity and remediation.
- `Recommendation`: blessed crate, design pattern, test, benchmark, or refactor suggestion.
- `ReleaseReport`: ship/no-ship decision with evidence links.

### 9.1.1 Required Type Contracts

The parse errors in this table are internal Rust error variants. Public CLI/API failures must map them to the canonical lowercase `ErrorKind` values in Section 14.2 before leaving the domain boundary.

| Type | Required Fields | Invariants | Parse Errors |
|---|---|---|---|
| `ProjectId` | content-derived or generated id | non-empty, stable for workspace root | `InvalidProjectId` |
| `TrustedWorkspacePath` | canonical absolute path | exists, directory, no symlink escape from root | `PathOutsideWorkspace`, `PathNotFound` |
| `HarnessRunId` | ULID/UUID | globally unique, sortable by creation time | `InvalidHarnessRunId` |
| `ActionId` | action namespace and name | lowercase segments, known in registry | `UnknownAction`, `InvalidActionId` |
| `EvidenceId` | content/checksum-derived id | immutable, unique per action attempt | `InvalidEvidenceId` |
| `FailureId` | stable id for a parsed failure | maps to one finding and one remediation hint | `InvalidFailureId` |
| `FindingSeverity` | enum | one of `Info`, `Warning`, `Major`, `Critical`, `Blocking` | `InvalidSeverity` |
| `GateStatus` | enum | one of `Planned`, `Running`, `Passed`, `Failed`, `Blocked`, `MissingEvidence`, `NotApplicableWithJustification` | `InvalidGateStatus` |
| `ReleaseVerdict` | enum | one of `Unknown`, `DoNotShip`, `HumanDecisionRequired`, `Ship` | `InvalidVerdict` |
| `BlessingDecision` | enum plus reason | one of `Blessed`, `Denied`, `RequiresJustification`, `ApprovedException` | `InvalidBlessingDecision` |
| `DependencyJustification` | crate, use case, alternatives, approval | required for non-blessed dependency | `MissingDependencyJustification` |
| `RedactedText` | redacted bytes/text path | secrets removed before persistence | `RedactionFailed` |

### 9.1.2 Aggregate Domain Contracts

| Aggregate | Required Fields | Invariants | Construction Boundary | Failure Modes |
|---|---|---|---|---|
| `Spec` | spec id, goal, domain model, state machine, invariants, error taxonomy, non-goals, acceptance scenarios | cannot become `SpecComplete` while any required section is empty; every invariant has at least one test obligation | parsed from interrogation answers or `architecture-spec.md` | `invalid_spec`, `unsupported_workspace` |
| `ArchitectureContract` | contract id, spec id, rules, dependency policy, async policy, storage policy, command policy | every blocking rule has a severity, rationale, and remediation; non-blessed deps require `DependencyJustification` | generated from `Spec` after completeness gate | `invalid_spec`, `dependency_not_blessed`, `dependency_denied` |
| `TestContract` | contract id, spec id, Given/When/Then scenarios, error tests, edge tests, property/fuzz/benchmark obligations | every precondition, postcondition, invariant, and error kind maps to at least one test obligation | generated only from `SpecComplete` or later | `invalid_spec` |
| `HarnessProfile` | version, name, profile id, spec requirements, architecture rules, gate graph, action refs | version must be supported; action refs must exist; gate graph must be acyclic; profile id is stable | parsed from `twerk.yaml` | `invalid_harness_yaml` |
| `ActionDefinition` | action id, command argv template, input schema, output schema, timeout, env allowlist, shell policy | command uses argv unless `shell_required`; timeout is explicit; outputs are redacted before persistence | loaded from built-in registry or approved extension | `invalid_harness_yaml`, `unsupported_workspace` |
| `ActionRun` | action run id, harness run id, action id, status, started, ended, exit model, evidence ids | terminal status requires evidence or blocking evidence failure; cancellation records process cleanup status | created by harness runner when gate starts | `missing_tool`, `command_failed`, `evidence_write_failed`, `redaction_failed` |
| `Finding` | finding id, severity, source, message, evidence ids, remediation, file/line optional | `Blocking` findings force `DoNotShip`; findings always link to evidence or explicit missing-evidence reason | parsed from evidence or reviewer output | `command_failed`, `dependency_denied`, `dependency_not_blessed` |
| `Recommendation` | recommendation id, category, target, rationale, blessed source, risk, next command | recommendations cannot downgrade a blocking finding; dependency recommendations obey governance order | generated from findings and policy rules | `internal_bug` |
| `ReleaseReport` | report id, harness run id, verdict, gate summaries, findings, missing evidence, dependency summary, benchmark summary | `Ship` requires all `MandatoryBlocking` gates passed and mandatory evidence present; report never reads raw unredacted logs | generated from stable read models after run terminal state | `evidence_write_failed`, `internal_bug` |

### 9.2 State Machines

Spec state:

```text
Draft -> NeedsClarification -> SpecComplete -> ArchitectureReviewed -> TestPlanApproved -> ReadyForImplementation
```

Allowed transitions:

- `Draft -> NeedsClarification` when required spec fields are missing.
- `Draft -> SpecComplete` only when every required spec field is present.
- `NeedsClarification -> SpecComplete` only after all blocking questions have answers or explicit assumptions.
- `SpecComplete -> ArchitectureReviewed` only after architecture review emits no blocking findings.
- `ArchitectureReviewed -> TestPlanApproved` only after Martin Fowler test plan review is approved.
- `TestPlanApproved -> ReadyForImplementation` only after bead decomposition has atomic implementation units.
- Any state may move back to `NeedsClarification` if new blocking ambiguity is found.

Harness run state:

```text
Planned -> Running -> Passed | Failed | Blocked | MissingEvidence
```

Allowed transitions:

- `Planned -> Running` when profile graph validates.
- `Running -> Passed` when every blocking gate passes and all mandatory evidence exists.
- `Running -> Failed` when a blocking gate executes and exits non-zero.
- `Running -> Blocked` when a required precondition or tool is unavailable and policy says it blocks execution.
- `Running -> MissingEvidence` when a gate cannot produce required evidence and policy allows report generation but forbids `Ship`.
- Terminal states may only transition to `Planned` through an explicit rerun.

Gate status:

```text
Planned -> Running -> Passed | Failed | Blocked | MissingEvidence | NotApplicableWithJustification
```

Gate status rules:

- `Passed`: command or analysis completed and required evidence was persisted.
- `Failed`: command or analysis ran and produced a failing result.
- `Blocked`: required tool, precondition, workspace support, or safe execution requirement is unavailable.
- `MissingEvidence`: gate is mandatory evidence, but the tool or output is not yet available.
- `NotApplicableWithJustification`: gate is explicitly skipped with machine-readable rationale and does not count as passed.

Release verdict:

```text
Unknown -> DoNotShip | HumanDecisionRequired | Ship
```

Verdict rules:

- `Ship`: every `MandatoryBlocking` gate passes, every mandatory evidence record exists, and no blocking findings remain.
- `DoNotShip`: any `MandatoryBlocking` gate fails, any dependency is denied, evidence redaction fails, or command execution security is violated.
- `HumanDecisionRequired`: only advisory failures remain, a non-blocking evidence waiver exists, or a justified non-blessed dependency awaits approval.
- `Unknown`: no complete verification run exists.

## 10. GoMasterOrchestrator Integration

Every bead generated from this architecture must be delivered through the 15-state GoMaster pipeline.

Required states:

1. Claim bead and initialize `.beads/<bead-id>/STATE.md`.
2. Explore and write `codebase-map.md`.
3. Generate `contract.md`.
4. Generate and approve `test-plan.md` and `test-plan-review.md`.
5. Write failing tests.
6. Implement with functional Rust.
7. Run first hands-on QA smoke test.
8. Run machine gates.
9. Run QA enforcer.
10. Run test-suite review.
11. Run Red Queen and black-hat review.
12. Run Kani or write a justification.
13. Run architectural drift and DDD polish.
14. Run final hands-on QA.
15. Close bead, sync, and prove cleanup.

No bead may skip contract, test, QA, black-hat, or evidence states.

## 11. Harness YAML Contract

Initial profile example:

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

## 12. Evidence Model

Every action writes an `EvidenceRecord`.

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

Secrets must be redacted before persistence.

### 12.1 Evidence Invariants

- Evidence is append-only. Existing evidence records are never mutated.
- Every evidence record has a checksum over redacted stdout, redacted stderr, metadata, and artifact references.
- Redaction happens before persistence, indexing, report generation, and UI reads.
- If redaction fails, the action status is `Failed`, release verdict is `DoNotShip`, and raw output is not persisted.
- Stdout/stderr are streamed to bounded chunks. Default chunk size is 64 KiB. Default retained output cap is 64 MiB per action unless profile overrides.
- Truncated output must set `truncated: true` and preserve the first and last chunks.
- Evidence paths are canonicalized under `.twerk/evidence/`; symlink escape is a blocking error.
- A release report cannot produce `Ship` unless every blocking gate has evidence.
- Evidence timestamps use system UTC plus monotonic duration for elapsed time.
- If evidence persistence fails, the harness run becomes `Blocked` and the command result cannot be treated as passed.

## 12.2 Stable Read Models For UI

The UI is post-MVP until these read models exist:

- `RunSummary`: run id, profile, status, verdict, started, ended, gate counts.
- `GateSummary`: gate id, status, duration, finding counts, evidence ids.
- `FindingSummary`: finding id, severity, source gate, file/line when available, remediation.
- `ReleaseReportView`: verdict, blocking findings, missing evidence, benchmark summary, dependency summary.

UI implementation must read these models. It must not scan raw logs for primary state.

## 12.3 Gate Policy

Gate kinds:

- `MandatoryBlocking`: missing tool, execution failure, or missing evidence produces non-zero exit and `DoNotShip`.
- `MandatoryEvidenceMayBeMissing`: missing tool produces non-zero exit for `verify`, `MissingEvidence` gate status, and `DoNotShip` until evidence exists.
- `Advisory`: failure produces non-zero only when `--strict-advisory` is set; default verdict is `HumanDecisionRequired`.
- `NotApplicableWithJustification`: skipped only when a written machine-readable justification exists.

MVP gate policy:

| Gate | Kind | Missing Tool | Failure | Evidence Required |
|---|---|---|---|---|
| `rust.fmt` | MandatoryBlocking | Blocked | DoNotShip | command output |
| `rust.clippy` | MandatoryBlocking | Blocked | DoNotShip | command output + parsed diagnostics |
| `rust.nextest` | MandatoryBlocking | Blocked | DoNotShip | command output + test summary |
| `rust.audit` | MandatoryBlocking | Blocked | DoNotShip | advisory report |
| `rust.deny` | MandatoryBlocking | Blocked | DoNotShip | policy report |
| `rust.coverage` | MandatoryEvidenceMayBeMissing | MissingEvidence | DoNotShip | coverage report |
| `rust.mutants` | MandatoryEvidenceMayBeMissing | MissingEvidence | DoNotShip | mutation report |
| `rust.fuzz` | MandatoryEvidenceMayBeMissing | MissingEvidence | DoNotShip | fuzz run report |
| `rust.bench` | MandatoryEvidenceMayBeMissing | MissingEvidence | DoNotShip | benchmark report |

## 12.4 Verification Pack Classification

Every verification item named by the PRD must be classified before bead generation:

| Verification Item | Action Id | Classification | MVP Behavior |
|---|---|---|---|
| Format | `rust.fmt` | MVP MandatoryBlocking | run `cargo fmt --check`; fail closed on missing tool or non-zero exit |
| Strict clippy | `rust.clippy` | MVP MandatoryBlocking | run strict clippy deny set; fail closed on diagnostics |
| Tests | `rust.nextest` | MVP MandatoryBlocking | run `cargo nextest run`; fail closed on failures or missing runner |
| Dependency audit | `rust.audit` | MVP MandatoryBlocking | run `cargo audit`; fail closed on high/critical advisories |
| Dependency policy | `rust.deny` | MVP MandatoryBlocking | run `cargo deny`; fail closed on denied deps/licenses/advisories |
| Coverage | `rust.coverage` | MVP MandatoryEvidenceMayBeMissing | discover/run coverage; `MissingEvidence` until threshold report exists |
| Mutation testing | `rust.mutants` | MVP MandatoryEvidenceMayBeMissing | discover/run mutation testing; `MissingEvidence` until kill-rate report exists |
| Fuzzing | `rust.fuzz` | MVP MandatoryEvidenceMayBeMissing | discover/run smoke fuzz targets; `MissingEvidence` until smoke report exists |
| Benchmarks | `rust.bench` | MVP MandatoryEvidenceMayBeMissing | discover/run Criterion or equivalent; `MissingEvidence` until baseline/report exists |
| Release report | `twerk.report.release` | MVP MandatoryBlocking | render markdown and JSON report from stable read models; fail if report cannot be written |
| Miri | `rust.miri` | Advisory | recommend for unsafe-adjacent/interpreter-sensitive code; not MVP blocking |
| Loom | `rust.loom` | Advisory | require justification when custom concurrency exists; not MVP blocking |
| Kani | `rust.kani` | Advisory via GoMaster state 12 | run when harnesses exist or require justification |
| Docs/API drift | `rust.docs_api_drift` | Post-MVP MandatoryEvidenceMayBeMissing | classify after core evidence schema is stable |
| Release artifact checks | `rust.release_artifacts` | Post-MVP MandatoryEvidenceMayBeMissing | classify after report/artifact model exists |

## 13. Performance Requirements

- Harness graph validation for typical projects must complete in under 100ms.
- Evidence writes must stream output without holding unbounded command logs in memory.
- Finding parsing must be incremental for long-running commands.
- Pure static analyses should use Rayon when scanning large file sets.
- Async command orchestration must have bounded concurrency.
- UI reads should be backed by indexed evidence summaries, not log-file scans.
- Benchmarks must compare against a stored baseline when available.

## 14. Security Requirements

- Never persist secrets in evidence.
- Redaction must apply to stdout, stderr, JSON findings, reports, and UI.
- Dependency recommendations must consider license, RustSec, maintenance, and transitive risk.
- Generated command actions must avoid shell interpolation unless explicitly requested.
- Tool execution must record exact command and working directory.
- Non-blessed dependencies require explicit approval.

### 14.1 Command Execution Contract

- Commands execute as argv arrays by default. Shell execution is denied unless an action definition explicitly declares `shell_required: true`.
- Every command has a timeout. Default timeout is 120 seconds for quick gates and must be explicit for long gates.
- Commands run under a canonical trusted workspace root.
- Working directories are canonicalized and must remain under the trusted workspace root.
- Symlink escape from workspace is a blocking security error.
- Environment defaults to a deny-all policy plus an explicit allowlist.
- Secret values are injected through redacted handles, never logged as cleartext environment dumps.
- Stdout and stderr are streamed through redaction before persistence.
- Cancellation kills the process group and waits for cleanup with a bounded timeout.
- Spawned command supervision must report exit status, signal termination, timeout, or cleanup failure distinctly.
- Network access policy is action-defined; verification commands default to current process network permissions until sandboxing exists, but reports must disclose network-unrestricted execution.

### 14.2 Error Taxonomy

| ErrorKind | Trigger | Exit Code | Recoverable | Evidence Required | Recommended Next Command |
|---|---|---:|---|---|---|
| `invalid_spec` | spec missing required field or invalid transition | 2 | yes | validation finding | `twerk spec interrogate --json` |
| `invalid_harness_yaml` | `twerk.yaml` parse/schema failure | 2 | yes | parse diagnostics | `twerk harness init --profile rust/strict-system --force` |
| `missing_tool` | required binary unavailable | 3 | yes | tool lookup evidence | install hint from `twerk explain` |
| `command_failed` | command exits non-zero | 1 | yes | command evidence | `twerk explain <failure-id>` |
| `dependency_denied` | deny policy or RustSec blocks crate | 4 | yes | dependency report | `twerk blessed explain <crate>` |
| `dependency_not_blessed` | non-blessed dependency lacks justification | 4 | yes | dependency report | `twerk blessed explain <crate>` |
| `redaction_failed` | secret redaction cannot prove safety | 5 | no | redaction failure metadata only | human intervention |
| `evidence_write_failed` | evidence cannot be persisted | 5 | maybe | filesystem error metadata | fix storage path and rerun |
| `unsupported_workspace` | repo shape cannot be classified | 2 | yes | workspace scan | `twerk explain <failure-id>` |
| `invalid_failure_id` | failure id is missing, malformed, or unknown | 2 | yes | failure-id parse diagnostic | rerun the command that produced the failure id |
| `internal_bug` | invariant violation inside Twerk | 70 | no | panic-free bug report | file issue with evidence id |

## 15. APIs And Commands

Required CLI:

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

### 15.1 CLI Contract Table

| Command | Args | Writes | Exit 0 | Non-zero |
|---|---|---|---|---|
| `twerk spec init` | optional output path | `architecture-spec.md` draft | draft created | `invalid_spec`, `evidence_write_failed` |
| `twerk spec interrogate` | optional spec path | spec questions or updated spec | no blocking questions remain or questions emitted | `invalid_spec` |
| `twerk spec validate` | spec path | evidence record | spec valid | `invalid_spec` |
| `twerk arch review` | spec path | findings evidence | no blocking architecture findings | `invalid_spec`, `command_failed` |
| `twerk tests plan` | spec path | `test-plan.md` | test plan generated | `invalid_spec` |
| `twerk harness init --profile rust/strict-system` | profile, optional force | `twerk.yaml` | harness written | `invalid_harness_yaml`, `evidence_write_failed` |
| `twerk verify --json` | optional profile/run filters | `.twerk/evidence/**`, report | all blocking gates pass | gate error kinds |
| `twerk explain <failure-id>` | failure id | none | explanation emitted | `invalid_failure_id` |
| `twerk report` | run id optional | `docs/release-evidence/<run-id>.md`, `docs/release-evidence/<run-id>.json` | report emitted | `evidence_write_failed` |
| `twerk blessed list` | none | none | policy list emitted | `internal_bug` |
| `twerk blessed explain <crate>` | crate name | none | decision emitted | `dependency_not_blessed` only when `--check` |
| `twerk blessed check` | optional manifest path | dependency evidence | all deps blessed/approved | `dependency_denied`, `dependency_not_blessed` |

Required JSON error shape:

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

## 16. Acceptance Criteria

### 16.1 Architecture Spec Acceptance

- The spec defines product scope, non-goals, domain model, state machines, evidence model, performance requirements, security requirements, and CLI/API contract.
- The spec explicitly includes Blessed-first dependency selection.
- The spec explicitly includes GoMaster 15-state delivery.
- The spec explicitly includes Holzmann and Fowler constraints.

### 16.2 MVP Acceptance

Given a Rust repo, when a user runs:

```bash
twerk harness init --profile rust/strict-system
twerk verify --json
```

Twerk must:

- generate `twerk.yaml`;
- detect workspace shape;
- run the MVP gates classified in Sections 12.3 and 12.4;
- report missing required tools as findings;
- reject unexplained non-blessed dependencies;
- store evidence;
- return structured JSON;
- produce `docs/release-evidence/<run-id>.md` and `docs/release-evidence/<run-id>.json` from `.twerk/evidence/` records;
- return non-zero when required gates fail.

### 16.3 Bead Acceptance

Every generated bead must include:

- EARS requirements;
- KIRK preconditions, postconditions, invariants;
- dependency blessing contract;
- Martin Fowler Given/When/Then tests;
- explicit failure modes;
- anti-hallucination read-before-write instructions;
- GoMaster artifact requirements;
- verification commands.

## 17. Atomic Bead Candidates

The bead pipeline must prefer atomic tasks like these, not broad epics:

1. `docs: Update README positioning to strict Rust systems factory`.
2. `schema: Define HarnessVersion and reject unknown twerk.yaml versions`.
3. `schema: Parse rust/strict-system profile header`.
4. `schema: Reject harness steps with unknown action ids`.
5. `cli: Add twerk harness init command skeleton`.
6. `cli: Emit stable JSON envelope for success and error responses`.
7. `evidence: Define EvidenceId and EvidenceRecord domain types`.
8. `evidence: Capture rust.fmt command stdout stderr exit code`.
9. `evidence: Redact configured secrets before evidence persistence`.
10. `evidence: Checksum redacted evidence records`.
11. `runner: Execute rust.fmt as argv with timeout`.
12. `runner: Execute rust.clippy with strict deny list`.
13. `runner: Execute rust.nextest and parse test summary`.
14. `runner: Return missing_tool finding when cargo-audit is absent`.
15. `runner: Classify gate statuses using GatePolicy`.
16. `blessed: Add blessed policy file parser`.
17. `blessed: Reject non-blessed dependency without justification`.
18. `blessed: Apply deny policy before blessed overrides`.
19. `spec: Generate initial architecture-spec.md template`.
20. `spec: Validate required spec sections`.
21. `arch: Reject stringly typed domain model in architecture review`.
22. `tests: Generate Martin Fowler Given/When/Then test-plan skeleton`.
23. `report: Emit markdown release report from gate summaries`.
24. `report: Emit JSON release verdict with evidence links`.
25. `gomaster: Generate required .beads artifact checklist for implementation beads`.
26. `security: Canonicalize workspace paths and reject symlink escape`.
27. `security: Kill process group on command timeout`.
28. `perf: Benchmark harness graph validation under 100ms target`.
29. `perf: Benchmark evidence streaming memory ceiling`.
30. `ui-readmodel: Define RunSummary GateSummary FindingSummary ReleaseReportView`.

## 18. Final Constraint

Twerk must be built under the same rules it enforces.

If Twerk cannot produce strict, blessed, evidence-backed Rust for itself, it cannot credibly sell strict, blessed, evidence-backed Rust to agents.
