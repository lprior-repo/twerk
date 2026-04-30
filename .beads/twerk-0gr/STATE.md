# STATE — twerk-0gr

## Bead
- **ID**: twerk-0gr
- **Title**: Fix broken twerk-cli test infra and handler defects
- **Status**: in_progress
- **Priority**: 1 (bug)
- **Workspace**: /home/lewis/src/twerk-0gr (jj workspace `twerk-0gr`)
- **Parent**: pkqozvtk fc2238c7 main

## Current State
**State 12: Kani Verification** — IN PROGRESS

## State History
| State | Status | Notes |
|-------|--------|-------|
| 1 | COMPLETE | Claimed bead, created JJ workspace twerk-0gr |
| 2 | COMPLETE | codebase-map.md (259 lines) — 140 tests passing, 9 error-body drop sites mapped |
| 3 | COMPLETE | contract.md (599 lines) — D1 test infra, D2 trigger tests, D3 handler error body drops |
| 4 | COMPLETE | test-plan.md (1244 lines, 78 behaviors) + test-plan-review.md (326 lines, APPROVED) — 10 review passes |
| 5 | COMPLETE | D1: 162 tests pass. D2: 21 trigger tests pass. D3: 36 handler tests (13 fail as expected). Total: 219 tests, 206 pass |
| 6 | COMPLETE | Fixed 9 body-drop sites in queue.rs, task.rs, node.rs, metrics.rs, user.rs — all adopt trigger.rs body-first pattern. 219/219 tests pass |
| 7 | COMPLETE | manual-qa-smoke.md — PASS. All 219 tests pass, build clean, clippy clean (pre-existing e2e warning only) |
| 8 | COMPLETE | moon-report.md — :test PASS, :ci fails on pre-existing fmt issue (generated_workload_contracts.rs) |
| 9 | COMPLETE | qa-report.md + qa-review.md — APPROVED |
| 10 | COMPLETE | test-suite-review.md — APPROVED (0 LETHAL, 0 MAJOR, 3 MINOR) after fixing 4 is_ok() + 9 test_ names |
| 11 | COMPLETE | black-hat-review.md — APPROVED after adding URL encoding (percent-encoding crate + encode_path_segment helper) |

## Scope
Three-pronged fix:
1. Repair bdd_behavior_report.rs Holzmann violations (loop Rule 2, LazyLock<Mutex> Rule 7), fix test_ naming violations
2. Add missing trigger tests: negative HTTP statuses, TriggerId boundaries, mutation kill verification
3. Fix queue/task handlers dropping server error bodies, add URL encoding of path segments

## Retry Budget
Remaining: 7

## Key Files
- `crates/twerk-cli/tests/bdd_behavior_report.rs` — Holzmann violations
- `crates/twerk-cli/tests/bdd_behavioral_contract_test.rs` — let _ = suppressions
- `crates/twerk-cli/src/handlers/queue.rs` — drops server error bodies
- `crates/twerk-cli/src/handlers/task.rs` — drops server error bodies
- `crates/twerk-cli/src/handlers/trigger.rs` — already fixed in prior session
- `crates/twerk-cli/src/error.rs` — clean on origin/main (ErrorKind, CliError)
- `crates/twerk-cli/src/migrate.rs` — test_ naming violations
- `crates/twerk-cli/src/commands.rs` — test_ naming violations
- `crates/twerk-cli/src/run.rs` — test_ naming violations
- `crates/twerk-cli/src/health.rs` — test_ naming violations
