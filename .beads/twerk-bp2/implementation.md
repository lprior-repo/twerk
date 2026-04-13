# twerk-bp2 implementation

## Files changed

- `.beads/twerk-bp2/contract.md`
- `.beads/twerk-bp2/test-plan.md`
- `crates/twerk-core/src/eval/mod.rs`
- `crates/twerk-core/src/eval/template.rs`
- `crates/twerk-core/src/eval/transform.rs`
- `crates/twerk-core/src/eval/state_dispatch.rs`
- `crates/twerk-core/src/eval/state_dispatch/arms.rs`
- `crates/twerk-core/src/eval/state_dispatch/metadata.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/builders.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/eval_kind.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/eval_machine.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/eval_state.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/fixtures.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/machine_fixtures.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/mod.rs`
- `crates/twerk-core/src/eval/state_dispatch/tests/static_guards.rs`

## Clause mapping

- `evaluate_state` now implements the ASL-native public surface and preserves wrapper metadata (`contract.md` Postconditions: ASL-native boundary, shared field preservation, validated newtypes preservation; Invariants: `INV-S1-2`, `INV-S1-4`, `INV-S1-7`, `INV-S1-8`).
- `evaluate_state_kind` is now a slim 16-line exhaustive match that delegates each arm to a named helper, satisfying the 25-line ceiling (`contract.md` Postconditions: exhaustive internal dispatcher, variant-specific guarantees, no panic; Invariants: `INV-S1-1`, `INV-S1-3`, `INV-S1-5`, `INV-S1-7`, `INV-S1-8`).
- `evaluate_state_machine` now recursively dispatches ordered states, parallel branches, and map item processors, then re-validates the rebuilt machine and maps topology failures to `StateEvalError::StateMachine` (`contract.md` Postconditions: recursive `StateMachine` surface, topology preservation, error returns; Invariants: `INV-S1-6`, `INV-S1-8`; Error Taxonomy: `StateMachine`).
- `build_task_arm` / `build_choice_arm` / `build_parallel_arm` / `build_map_arm` are private dispatch seam functions. `build_task_arm`, `build_parallel_arm`, and `build_map_arm` now consume typed spec structs instead of raw parameter bags, removing the `clippy::too_many_arguments` waivers while also replacing the bead-local `Option<bool>` helper parameter with a named `ParallelArmFailFast` sum type (`contract.md` Error Taxonomy: `TaskState`, `ChoiceState`, `ParallelState`, `MapState`).
- `dispatch_task_from_state` / `eval_parallel_arm` / `eval_map_arm` are intermediate dispatch helpers bridging valid `StateKind` payloads into the typed seam specs and builders; `dispatch_task_from_state` remains definition-time only and rebuilds `TaskState` without evaluating `env` expressions.
- The contract/test plan now map parity to the real STATE 1 seams: `evaluate_state` and `evaluate_state_machine` cover reachable definition-time preservation plus machine validation re-entry, `evaluate_state_kind` covers only success-arm dispatch, and exact Task / Choice / Parallel / Map constructor failures are asserted only at `build_*_arm`.
- The bead-local suite now proves malformed Task env expressions survive both `evaluate_state(...)` and `evaluate_state_machine(...)` unchanged, while the remaining exact error inventory lives at the raw arm builders and invalid-input machine validation seam.
- The bead-local test harness no longer uses scattered `.expect("…")` calls or the panicing `must(...)` seam; fixture construction now propagates `TestResult` through typed builders and helper functions.
- Dense topology fixtures now decompose into named state and machine builders so the black-hat 25-line ceiling is satisfied without changing any dispatch assertions or machine topology.
- Architectural-drift follow-up split the old monolithic `state_dispatch.rs` into a thin public dispatcher (`state_dispatch.rs`), focused arm/metadata submodules, and a test tree under `state_dispatch/tests/`, bringing the bead-local dispatch files below the 300-line ceiling while preserving the accepted public contract.

## STATE 6 repair summary (relative to black-hat-review.md)

| Issue | Resolution |
|---|---|
| LETHAL: live `Eval` seam violated the definition-time contract | Removed the eager eval helper delegation, corrected the contract/test plan to keep malformed `Expression` payloads deferred, and now prove top-level and nested invalid Task env expressions are preserved unchanged |
| MAJOR: `evaluate_state_kind` 58-line god function | Decomposed to 16 lines + 7 named helpers |
| MAJOR: `build_task_arm` / `build_map_arm` exceeded the argument ceiling | Replaced raw parameter lists with `TaskArmSpec` / `MapArmSpec` typed inputs and removed both `clippy::too_many_arguments` suppressions |
| MAJOR: panic-heavy test fixtures | Removed the panicing `must(...)` seam and converted bead-local fixtures/tests to `TestResult`-returning builders |
| MAJOR: bead-local fixture/test functions exceeded the 25-line ceiling | Split `dense_all_variant_machine` plus the two nested-topology tests into named builders and a shared `assert_machine_dispatch(...)` helper |
| MINOR: `Option<bool>` helper parameter | Replaced bead-local `build_parallel_arm(..., Option<bool>)` with `ParallelArmSpec { fail_fast: ParallelArmFailFast, ... }` |

## STATE 5.7 Kani justification

- `cargo kani` is available in this environment and was executed for this bead, writing `.beads/twerk-bp2/kani-report.md`.
- The actual Kani result was `No proof harnesses (functions with #[kani::proof]) were found to verify.`
- The bead-local dispatch surface has **no checked-in Kani harness** under `crates/twerk-core/src/eval/state_dispatch*` or `.beads/twerk-bp2`; a workspace search for `kani::` / `cfg(kani)` returned no matches.
- The formal exception is therefore **missing harnesses, not missing tooling**. Executed substitute gates remain the bead-local static guards, the full 71-test `state_dispatch` suite, `cargo clippy -p twerk-core --all-targets -- -D warnings`, QA, suite inquisition, Red Queen, and black-hat review.

## STATE 4 moon gate repair

- Repaired the exact `root:ci-source` formatting diffs captured in `.beads/twerk-bp2/compiler-errors.log` for `crates/twerk-core/src/eval/template.rs`, `crates/twerk-core/src/eval/transform.rs`, `crates/twerk-core/src/eval/state_dispatch/tests/eval_machine.rs`, and `crates/twerk-core/src/eval/state_dispatch/tests/machine_fixtures.rs`.
- No behavior changed: the repair is rustfmt-only and keeps the bead-local ASL dispatch implementation and tests semantically identical.
