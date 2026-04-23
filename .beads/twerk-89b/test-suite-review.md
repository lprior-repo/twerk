bead_id: twerk-89b
bead_title: drift: repo-wide architectural cleanup and DRY sweep
phase: state-10-test-suite-review-rerun-current-snapshot-after-latest-non-density-tier-0-repairs
updated_at: 2026-04-23T23:05:00Z

STATUS: REJECTED

## VERDICT: REJECTED

### Scope
- Read `.beads/twerk-89b/moon-report.md`, `.beads/twerk-89b/qa-report.md`, `.beads/twerk-89b/qa-review.md`, `.beads/twerk-89b/test-suite-review.md`, and `.beads/twerk-89b/STATE.md` first.
- Re-ran State 10 on the current snapshot with emphasis on the latest repaired Tier-0 files.
- Did not modify source code.

### Focus-file recheck
- `crates/twerk-core/tests/domain_verification_test.rs` — prior banned `is_ok()` / `is_err()` blockers are repaired; current assertions are concrete (`:20-21`, `:68-69`, `:93-94`).
- `crates/twerk-infrastructure/tests/rabbitmq_test.rs` — prior `let _ = tx.send(()).await` discard is repaired; the send now propagates failure with `map_err(...)` at `:303-305`.
- `crates/twerk-web/tests/batch_yaml_100_task_test/boundary_cases.rs:5`, `.../post_jobs.rs:5`, `.../comprehensive_api_test/job_endpoints.rs:5`, `.../queue_and_user_endpoints.rs:4`, `.../system_endpoints.rs:3`, and `.../openapi_contract_test/endpoint_contracts.rs:7` no longer use `/tests/` `use crate::...`; those purity blockers are repaired.
- `crates/twerk-core/src/trigger/tests.rs` is not clean: the Kani proof still uses nested loops at `:1506-1508`.
- `crates/twerk-infrastructure/tests/runtime_test.rs` is not clean: probe tests still embed sleep-driven timing at `:135` and `:218`.

### Tier 0 — Static
[FAIL] Banned pattern scan
- `crates/twerk-core/tests/red_queen_adversarial.rs:531` — banned `assert!(result.is_ok())`.
- `crates/twerk-core/tests/red_queen_adversarial.rs:538` — banned `assert!(result.is_ok())`.
- `crates/twerk-core/tests/red_queen_gen3.rs:333` — banned `assert!(result.is_ok())`.
- `crates/twerk-core/tests/red_queen_gen3.rs:341` — banned `assert!(result.is_ok())`.
- `crates/twerk-core/tests/red_queen_gen4.rs:89` — banned `assert!(result.is_err())`.
- `crates/twerk-core/tests/red_queen_gen4.rs:95` — banned `assert!(result.is_ok())`.
- `crates/twerk-core/src/trigger/data/webhook_trigger.rs:174` — banned `assert!(result.is_ok())`.

[FAIL] Holzmann rule scan
- `crates/twerk-core/src/trigger/tests.rs:1506` — loop in proof body.
- `crates/twerk-core/src/trigger/tests.rs:1507` — nested loop in proof body.
- `crates/twerk-core/src/trigger/tests.rs:1508` — triple-nested loop in proof body.
- `crates/twerk-infrastructure/tests/runtime_test.rs:135` — sleep-driven timing in exercised command.
- `crates/twerk-infrastructure/tests/runtime_test.rs:218` — sleep-driven timing in exercised command.

[PASS] Mock interrogation
- No `mockall`, `Mock*::new()`, or `.expect_` blocker found in `crates`.

[PASS] Integration test purity
- No current `use crate::` hit in `crates/*/tests`.

[FAIL] Density audit (253 tests / 290 public functions = 0.87x — target ≥5x)

### Tier 1 — Execution
[NOT RUN] Blocked by Tier 0 lethal findings.

### Tier 2 — Coverage
[NOT RUN] Blocked by Tier 0 lethal findings.

### Tier 3 — Mutation
[NOT RUN] Blocked by Tier 0 lethal findings.

### LETHAL FINDINGS
- `crates/twerk-core/tests/red_queen_adversarial.rs:531` — still pretending `is_ok()` is proof.
- `crates/twerk-core/tests/red_queen_gen4.rs:89` — still pretending `is_err()` is an exact error assertion.
- `crates/twerk-core/src/trigger/data/webhook_trigger.rs:174` — source test still uses banned `is_ok()`.
- `crates/twerk-core/src/trigger/tests.rs:1506-1508` — loop-driven proof body violates Rule 2.
- `crates/twerk-infrastructure/tests/runtime_test.rs:135` — docker probe test still depends on `sleep 5`.
- `crates/twerk-infrastructure/tests/runtime_test.rs:218` — podman probe test still depends on `sleep 10`.
- `density` — `253 / 290 = 0.87x`, still catastrophically below the 5.0x floor.

### MAJOR FINDINGS (0)
- None recorded. Tier 0 already killed the suite.

### MINOR FINDINGS (0/5 threshold)
- None recorded. Tier 0 already killed the suite.

### MANDATE
- Remove every remaining `assert!(result.is_ok())` / `assert!(result.is_err())` and replace them with exact value or exact variant assertions.
- Remove loop-driven proof/test bodies where Tier 0 bans them, including `crates/twerk-core/src/trigger/tests.rs:1506-1508`.
- Eliminate sleep-based timing from `crates/twerk-infrastructure/tests/runtime_test.rs` probe tests.
- Raise density above the hard floor; `0.87x` is nowhere close.
- After repair, rerun State 10 from Tier 0. Full rerun. No partial-credit nonsense.

### Top reasons
- The latest repaired web/API files are cleaner, but the suite still contains fresh global Tier-0 lethal hits in `red_queen_*` tests and `webhook_trigger.rs`.
- `crates/twerk-core/src/trigger/tests.rs:1506-1508` still uses nested loops in a proof body.
- `crates/twerk-infrastructure/tests/runtime_test.rs:135` and `:218` still hard-code sleep-based timing.
- Density is still dead on arrival: `253 tests / 290 pub fn = 0.87x`.
