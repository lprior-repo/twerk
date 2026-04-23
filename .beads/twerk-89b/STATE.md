bead_id: twerk-89b
bead_title: drift: repo-wide architectural cleanup and DRY sweep
phase: state-9-approved-recovered
updated_at: 2026-04-23T00:13:00Z

# Recovery state

- original isolated workspace `/home/lewis/src/twerk-89b` disappeared during the prior session
- recreated isolated workspace at the same path from current repo `@` using `jj workspace add`
- `bd show twerk-89b --json` still reports the bead as `in_progress`

# Recovered prior history from session summaries

- the lost workspace had previously reached:
  - State 8 green
  - State 9 approved
  - State 10 rejected on exact trigger-update body assertions
- a targeted repair for those exact-body assertions had been attempted before the workspace disappeared, but was never revalidated afterward

# Recreated-workspace resumption

- files were materialized with `jj workspace update-stale`
- first trustworthy failure in the recreated workspace was State 8 `CONTRACT_PARITY` because root `:quick` was missing
- targeted parity repair restored root `:quick` compatibility in `moon.yml` and fixed the surfaced lint/format blockers

# State 8 - Machine gates

- latest status: PASS
- latest evidence: `.beads/twerk-89b/moon-report.md` with `STATUS: PASS`
- latest resolved formatter drift:
  - `crates/twerk-core/tests/red_queen_adversarial.rs`
  - `crates/twerk-core/tests/red_queen_gen3.rs`
  - `crates/twerk-core/tests/red_queen_gen4.rs`
- latest rerun status remains green on the current snapshot after the latest core/infra test-discipline repair.
- environment note: successful latest rerun used `TMPDIR="$PWD/.tmp"` to avoid transient tmpfs quota failures in worker/runtime tests during `moon run :test` / `:ci`.

# State 9 - QA

- latest rerun status: APPROVED
- latest evidence:
  - `.beads/twerk-89b/qa-report.md` -> `STATUS: PASS`
  - `.beads/twerk-89b/qa-review.md` -> `STATUS: APPROVED`
- latest verified focus areas:
  - no remaining discard patterns in `crates/twerk-web/src/helpers.rs`
  - no remaining discard patterns in `crates/twerk-cli/tests/bdd_behavioral_contract_test.rs`
  - no remaining discard patterns in `crates/twerk-core/tests/red_queen_trigger_error.rs`
  - targeted repaired tests pass
  - full Moon machine gates pass on the current snapshot with `TMPDIR=/home/lewis/src/twerk-89b/.tmp` and `RUSTC_WRAPPER=`

# State 10 - Test suite review

- latest rerun status: REJECTED
- latest evidence: `.beads/twerk-89b/test-suite-review.md` with `STATUS: REJECTED`
- latest concrete blockers from the current snapshot:
  - banned assertions remain in:
    - `crates/twerk-core/tests/red_queen_adversarial.rs:531,538`
    - `crates/twerk-core/tests/red_queen_gen3.rs:333,341`
    - `crates/twerk-core/tests/red_queen_gen4.rs:89,95`
    - `crates/twerk-core/src/trigger/data/webhook_trigger.rs:174`
  - Holzmann Rule 2 still broken in `crates/twerk-core/src/trigger/tests.rs:1506-1508`
  - sleep-driven timing still exists in `crates/twerk-infrastructure/tests/runtime_test.rs:135,218`
  - density still fails hard: `253 tests / 290 pub fn = 0.87x`
- next repair scope is the concrete non-density blockers above; density remains unresolved and may still block later review.

# State 11 - Adversarial and black-hat review

- latest red-queen status: `CROWN FORFEIT`
- latest black-hat status: `REJECTED`
- latest blockers:
  - `crates/twerk-core/src/types/retry_limit.rs` still allows invalid states through `RetryLimit::new` / `From<u32>`
  - oversized mixed-concern schedulers remain in `crates/twerk-app/src/engine/coordinator/scheduler/parallel.rs` and `.../each.rs`
  - `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs` and `task_handlers.rs` still contain long event-handler blobs
  - `crates/twerk-app/src/engine/worker/docker.rs` still uses boolean flag constructor design
  - panic/unwrap discipline still fails in `crates/twerk-cli/src/cli/mod.rs` and `crates/twerk-app/src/engine/worker/shell.rs`
  - `crates/twerk-web/tests/trigger_update_adversarial_test.rs:402-406` still uses permissive `>=` instead of proving `updated_at` strictly advances
  - Red Queen still reports 10 surviving MAJOR global quality-gate defects (`cargo clippy` strict gates, coverage floor, `cargo audit`, `cargo deny`)

# Current next gate

- State 8 is green again on the current snapshot after formatter repair in:
  - `crates/twerk-core/tests/domain_verification_test.rs`
  - `crates/twerk-web/tests/comprehensive_api_test/system_endpoints.rs`
- State 9 and State 10 artifacts are now stale after the latest core/infra test-discipline repair.
- Latest State 9 evidence:
  - `.beads/twerk-89b/qa-report.md` -> `STATUS: PASS`
  - `.beads/twerk-89b/qa-review.md` -> `STATUS: APPROVED`
  - latest repaired non-density Tier-0 files and full machine gates verified green with `TMPDIR=/home/lewis/src/twerk-89b/.tmp` and `RUSTC_WRAPPER=`
- State 10 artifacts are stale because the latest repair changed:
  - `crates/twerk-core/tests/red_queen_adversarial.rs`
  - `crates/twerk-core/tests/red_queen_gen3.rs`
  - `crates/twerk-core/tests/red_queen_gen4.rs`
  - `crates/twerk-core/src/trigger/data/webhook_trigger.rs`
  - `crates/twerk-core/src/trigger/tests.rs`
  - `crates/twerk-infrastructure/tests/runtime_test.rs`
- State 11 artifacts remain stale relative to the latest code changes.
- Next gate: rerun State 9 on the current snapshot, then rerun State 10 if State 9 approves. Density remains unresolved unless later review accepts proceeding despite it.
