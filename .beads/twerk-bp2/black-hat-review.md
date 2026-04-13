---
bead_id: twerk-bp2
bead_title: Eval Engine: State-based evaluation dispatch
phase: state-5.5-black-hat-review-post-architectural-drift
updated_at: 2026-04-13T11:11:39Z
---

# Black Hat Review — twerk-bp2

## Verdict
APPROVED.

## Scope
- `.beads/twerk-bp2/contract.md`
- `.beads/twerk-bp2/test-plan.md`
- `.beads/twerk-bp2/implementation.md`
- `.beads/twerk-bp2/kani-justification.md`
- `crates/twerk-core/src/eval/mod.rs`
- `crates/twerk-core/src/eval/template.rs`
- `crates/twerk-core/src/eval/transform.rs`
- `crates/twerk-core/src/eval/state_dispatch.rs`
- `crates/twerk-core/src/eval/state_dispatch/arms.rs`
- `crates/twerk-core/src/eval/state_dispatch/metadata.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/*.rs`

## Phase 1 — Contract & Bead Parity
- PASS — The public boundary is ASL-native exactly as contracted: `evaluate_state` returns `State` and reapplies wrapper metadata at `crates/twerk-core/src/eval/state_dispatch.rs:39-46`, while `evaluate_state_machine` returns `StateMachine`, preserves topology, and re-validates before success at `crates/twerk-core/src/eval/state_dispatch.rs:49-63`, matching `contract.md:52-76, 82-90, 129-150`.
- PASS — The dispatcher is exhaustive over the closed eight-variant union with no legacy task fallback or unsupported arm at `crates/twerk-core/src/eval/state_dispatch.rs:66-82`; the static guards lock that boundary in at `crates/twerk-core/src/eval/state_dispatch/tests/static_guards.rs:10-50`.
- PASS — Test-plan parity is real, not performative: the preservation, recursion, invalid-topology, and deferred-invalid-expression scenarios listed in `test-plan.md:125-223` are covered by `crates/twerk-core/src/eval/state_dispatch/tests/eval_state.rs:22-229`, `eval_machine.rs:7-283`, `eval_kind.rs:8-70`, and `builders.rs:4-141`.
- PASS — The post-drift implementation still matches the bead narrative: the dispatcher split into thin public/arm/metadata seams and test-tree decomposition described in `implementation.md:24-33` is what is actually checked in.
- PASS — The Kani no-harness exception is documented as a no-proof reality, not a tooling lie: `kani-justification.md:10-24` and `implementation.md:46-51` record the successful `cargo kani` run, the lack of checked-in bead-local proof harnesses, and the substitute executed gates.

## Phase 2 — Farley Rigor
- PASS — The reviewed production dispatch code is small and surgical. `evaluate_state_kind` is 17 lines at `crates/twerk-core/src/eval/state_dispatch.rs:66-82`; the heaviest arm builder remains `build_task_arm` at `crates/twerk-core/src/eval/state_dispatch/arms.rs:57-81`, still under the 25-line ceiling.
- PASS — No reviewed dispatch or metadata function takes more than five parameters. The raw constructor slop was boxed into typed specs (`TaskArmSpec`, `ParallelArmSpec`, `MapArmSpec`) at `crates/twerk-core/src/eval/state_dispatch/arms.rs:16-34, 125-129, 169-173`.
- PASS — Functional-core discipline holds. The reviewed surface is pure reconstruction plus validation only; no I/O, shelling, waiting, or template evaluation is smuggled into dispatch (`state_dispatch.rs:39-82`, `metadata.rs:7-29`, `contract.md:116-120, 152-160`).
- PASS — Tests assert behavior instead of private call choreography. They prove round-trip preservation, exact constructor failures, and recursive validation surfaces rather than implementation trivia (`eval_state.rs:18-229`, `eval_machine.rs:8-283`, `builders.rs:4-141`).

## Phase 3 — Functional Rust Big 6
- PASS — Illegal states stay pushed to constructors and sum types. `ParallelArmFailFast` replaces the old boolean ambiguity, and the typed arm specs keep the public surface from degenerating into raw-primitive sludge (`crates/twerk-core/src/eval/state_dispatch/arms.rs:98-129`).
- PASS — Parse-don't-validate discipline holds. The dispatcher consumes validated `State`, `StateKind`, and `StateMachine` values and deliberately preserves malformed runtime `Expression` payloads for later phases, exactly as required by `contract.md:69-76, 116-120` and proved by `eval_state.rs:223-229` plus `eval_machine.rs:230-252`.
- PASS — Workflows are explicit state-to-state and machine-to-machine transitions, not an Option-driven pseudo-machine. Parallel and Map recurse through named helpers (`arms.rs:148-159, 212-220`) and machine validation is explicit (`metadata.rs:24-29`).

## Phase 4 — Ruthless Simplicity & DDD
- PASS — No panic seam survived the reviewed surface. There are no `unwrap`, `expect`, `panic!`, `todo!`, or `unimplemented!` uses in the reviewed production dispatch modules or bead-local tests.
- PASS — Metadata handling is intentionally boring. Comment/input/output/assign and machine comment/timeout are reattached by explicit helpers instead of clever generic indirection at `crates/twerk-core/src/eval/state_dispatch/metadata.rs:7-80`.
- PASS — No wildcard fallback, no legacy adapter, no fake runtime behavior. The code does exactly one thing: definition-time ASL dispatch.

## Phase 5 — Bitter Truth
- PASS — The drift repair made the code more boring, not more baroque. The public dispatcher is thin, the builders are named, the metadata helpers are obvious, and the test matrix is split by behavior instead of dumped into one vanity blob (`implementation.md:24-33`).
- PASS — I found no speculative abstraction built for imaginary future flexibility in the reviewed bead-local surface.

## Validation
- `cargo test -p twerk-core state_dispatch -- --nocapture` ✅ (71/71 passed)
- `cargo clippy -p twerk-core --all-targets -- -D warnings` ✅
- Panic-vector audit across reviewed production and bead-local test files ✅
- Contract/test-plan/implementation/Kani parity audit ✅

## Blockers
- None.

STATUS: APPROVED
