---
bead_id: twerk-bp2
bead_title: Eval Engine: State-based evaluation dispatch
phase: state-1.8-final-parity-repair
updated_at: 2026-04-13T00:00:00Z
---

# Test Plan: Eval Engine: State-based evaluation dispatch

## Summary

- Behaviors identified: 71
- Trophy allocation: 47 unit / 22 integration / 0 e2e / 2 static
- Proptest invariants: 0 checked-in bead-local assets
- Fuzz targets: 0 checked-in bead-local assets
- Kani harnesses: 0 checked-in bead-local assets
- Target mutation score: >=90%
- Executed verification for this bead is the deterministic 71-test suite, 2 static guards, clippy, and the recorded Red Queen run.
- Wait coverage is split into explicit single-behavior proofs; no loop-heavy aggregate remains in the plan.

## Boundary Map

### Public API: `evaluate_state`
- Covers only reachable definition-time success/preservation behavior for already-validated `State` inputs.
- Keeps exact preservation proofs for wrapper metadata, Task/Pass/Choice/Wait/Succeed/Fail/Parallel/Map payloads, typed ASL values, deferred invalid `Expression` payloads, and definition-time-only semantics.
- Does not claim exact Task/Choice/Parallel/Map constructor failures here; those are unreachable through validated ASL constructors.

### Public API: `evaluate_state_machine`
- Covers reachable machine-surface behavior: empty machine, invalid input machine topology (`start_at` missing from keys, transition target missing, missing Choice/default targets, no terminal state), machine comment/timeout preservation, recursive success/topology preservation, nested invalid child-machine failures proving the real recursive calls, nested deferred invalid `Expression` payloads, post-dispatch `validate()` re-entry, and definition-time-only semantics.
- Does not claim nested child `TaskState`/`ChoiceState`/`ParallelState`/`MapState` propagation or synthetic recursive rewrites of `start_at` / transition targets; those failures are not reachable through the real recursive entrypoints as implemented.

### Internal dispatcher: `evaluate_state_kind`
- Covers only the eight reachable success arms for the closed `StateKind` union.
- Error parity for exact constructor failures belongs to the raw seam builders, not this validated dispatcher entrypoint.

### Raw seam builders
- `build_task_arm`, `build_choice_arm`, `build_parallel_arm`, and `build_map_arm` own the exact arm-error inventory.
- Every exact Task / Choice / Parallel / Map constructor failure is asserted here with the exact `StateEvalError` wrapper variant.

## Behavior Inventory

### `evaluate_state` (28)
1. preserves Task wrapper fields
2. preserves Task payload fields
3. preserves Task timeout minimum boundary
4. preserves Task heartbeat minimum boundary
5. preserves Task heartbeat one below timeout
6. preserves Pass `result = None`
7. preserves Pass `result = Some(json)`
8. preserves Pass `Transition::Next`
9. preserves Pass `Transition::End`
10. preserves Choice rule order
11. preserves Choice payloads and default
12. keeps Choice declarative
13. preserves Wait `Seconds`
14. preserves Wait `Timestamp`
15. preserves Wait `SecondsPath`
16. preserves Wait `TimestampPath`
17. preserves Succeed terminality
18. preserves Fail with neither literal
19. preserves Fail with only `error`
20. preserves Fail with only `cause`
21. preserves Fail with both literals
22. preserves Parallel branch order
23. preserves Map tolerance `None`
24. preserves Map tolerance `0.0`
25. preserves Map tolerance `100.0`
26. preserves validated ASL newtypes
27. remains definition-time only
28. preserves invalid Task env expressions unchanged

### `evaluate_state_machine` (22)
29. returns exact `StateMachine([EmptyStates])` for an empty machine
30. returns exact `StateMachine([StartAtNotFound { .. }])` for invalid input whose `start_at` key is missing
31. returns exact `StateMachine([TransitionTargetNotFound { .. }])` for invalid input whose `Next(target)` is missing
32. returns exact `StateMachine([ChoiceTargetNotFound { .. }])` when a Choice rule targets a missing state
33. returns exact `StateMachine([DefaultTargetNotFound { .. }])` when a Choice default targets a missing state
34. returns exact `StateMachine([NoTerminalState])` when the machine has no terminal state
35. preserves a minimum valid machine
36. preserves a dense all-variant flat machine
37. preserves machine comment
38. preserves machine `timeout = None`
39. preserves machine `timeout = Some(0)`
40. preserves machine `timeout = Some(1)`
41. preserves machine `timeout = Some(u64::MAX)`
42. recursively dispatches nested Parallel branches
43. returns exact nested `StateMachine([StartAtNotFound { .. }])` when a Parallel branch machine is invalid
44. recursively dispatches nested Map item processors
45. returns exact nested `StateMachine([TransitionTargetNotFound { .. }])` when a Map item processor is invalid
46. preserves Parallel-inside-Map topology
47. preserves Map-inside-Parallel topology
48. re-runs `StateMachine::validate()` before returning success
49. preserves nested invalid Task env expressions unchanged
50. remains definition-time only

### `evaluate_state_kind` success arms (8)
51. dispatches Task arm successfully
52. dispatches Pass arm successfully
53. dispatches Choice arm successfully
54. dispatches Wait arm successfully
55. dispatches Succeed arm successfully
56. dispatches Fail arm successfully
57. dispatches Parallel arm successfully
58. dispatches Map arm successfully

### Raw seam builders (11)
59. `build_task_arm` returns `TimeoutTooSmall(0)`
60. `build_task_arm` returns `HeartbeatTooSmall(0)`
61. `build_task_arm` returns heartbeat-equals-timeout error
62. `build_task_arm` returns heartbeat-above-timeout error
63. `build_task_arm` returns `EmptyEnvKey`
64. `build_choice_arm` returns `EmptyChoices`
65. `build_parallel_arm` returns `EmptyBranches`
66. `build_map_arm` returns below-range tolerance error
67. `build_map_arm` returns above-range tolerance error
68. `build_map_arm` returns NaN tolerance error
69. `build_map_arm` returns positive-infinity tolerance error

### Static guards (2)
70. public dispatch signatures and error surface never reference legacy task types
71. no unsupported-state fallback exists for the closed `StateKind` union

## Scenario Catalog

### `evaluate_state`

| Behavior | Test name | Given | Then | Layer |
|---|---|---|---|---|
| Task wrapper preservation | `evaluate_state_preserves_task_wrapper_fields_when_task_state_is_valid` | valid Task with wrapper metadata | exact wrapper fields preserved | Unit |
| Task payload preservation | `evaluate_state_preserves_task_payload_fields_when_task_state_is_valid` | valid Task with populated payload fields | exact Task payload preserved | Unit |
| Task timeout minimum | `evaluate_state_preserves_task_timeout_of_one_when_timeout_is_minimum_valid` | valid Task with `timeout = Some(1)` | exact `Some(1)` preserved | Unit |
| Task heartbeat minimum | `evaluate_state_preserves_task_heartbeat_of_one_when_timeout_is_two` | valid Task with `(timeout, heartbeat) = (2,1)` | exact values preserved | Unit |
| Task heartbeat one below timeout | `evaluate_state_preserves_task_heartbeat_when_heartbeat_is_one_below_timeout` | valid Task with `(10,9)` | exact values preserved | Unit |
| Pass absent result | `evaluate_state_preserves_pass_result_none_when_pass_result_is_absent` | valid Pass with `result = None` | exact absence preserved | Unit |
| Pass present result | `evaluate_state_preserves_pass_result_some_when_pass_result_is_present` | valid Pass with `result = Some(json)` | exact JSON preserved | Unit |
| Pass next transition | `evaluate_state_preserves_pass_next_transition_when_transition_is_next` | valid Pass with `Transition::Next(target)` | exact next transition preserved | Unit |
| Pass end transition | `evaluate_state_preserves_pass_end_transition_when_transition_is_end` | valid Pass with `Transition::End` | exact end transition preserved | Unit |
| Choice rule order | `evaluate_state_preserves_choice_rule_order_when_choice_state_is_valid` | valid Choice with deliberate rule order | exact order preserved | Unit |
| Choice payload/default | `evaluate_state_preserves_choice_rule_payloads_and_default_when_choice_state_is_valid` | valid Choice with populated `condition`, `next`, `assign`, `default` | exact payload/default preserved | Unit |
| Choice declarative | `evaluate_state_does_not_select_a_branch_when_choice_state_is_dispatched` | valid Choice and runtime-shaped context | returns Choice definition unchanged; no branch selected | Unit |
| Wait seconds | `evaluate_state_preserves_wait_seconds_when_wait_duration_is_seconds` | valid Wait with `WaitDuration::Seconds` | exact discriminant/value preserved | Unit |
| Wait timestamp | `evaluate_state_preserves_wait_timestamp_when_wait_duration_is_timestamp` | valid Wait with `WaitDuration::Timestamp` | exact discriminant/value preserved | Unit |
| Wait seconds path | `evaluate_state_preserves_wait_seconds_path_when_wait_duration_is_seconds_path` | valid Wait with `WaitDuration::SecondsPath` | exact discriminant/value preserved | Unit |
| Wait timestamp path | `evaluate_state_preserves_wait_timestamp_path_when_wait_duration_is_timestamp_path` | valid Wait with `WaitDuration::TimestampPath` | exact discriminant/value preserved | Unit |
| Succeed terminality | `evaluate_state_preserves_succeed_terminality_when_succeed_state_is_valid` | valid Succeed | remains transitionless | Unit |
| Fail neither literal | `evaluate_state_preserves_fail_without_error_or_cause_when_both_literals_are_absent` | valid Fail with no literals | exact absence preserved | Unit |
| Fail only error | `evaluate_state_preserves_fail_error_when_only_error_literal_is_present` | valid Fail with `error = Some`, `cause = None` | exact literals preserved | Unit |
| Fail only cause | `evaluate_state_preserves_fail_cause_when_only_cause_literal_is_present` | valid Fail with `error = None`, `cause = Some` | exact literals preserved | Unit |
| Fail both literals | `evaluate_state_preserves_fail_error_and_cause_when_both_literals_are_present` | valid Fail with both literals | exact literals preserved | Unit |
| Parallel success | `evaluate_state_preserves_parallel_branch_order_when_parallel_state_is_valid` | valid Parallel with deliberate branch order | exact branch order preserved | Unit |
| Map tolerance none | `evaluate_state_preserves_map_tolerated_failure_percentage_when_value_is_valid` | valid Map with `None` | exact `None` preserved | Unit |
| Map tolerance lower edge | `evaluate_state_preserves_map_tolerated_failure_percentage_when_value_is_valid` | valid Map with `0.0` | exact `Some(0.0)` preserved | Unit |
| Map tolerance upper edge | `evaluate_state_preserves_map_tolerated_failure_percentage_when_value_is_valid` | valid Map with `100.0` | exact `Some(100.0)` preserved | Unit |
| Typed ASL boundaries | `evaluate_state_preserves_validated_newtypes_when_asl_state_contains_them` | state containing `Expression`, `JsonPath`, `StateName`, `ShellScript`, `Transition` | typed values preserved; no raw-string downgrade | Unit + Static |
| Definition-time only | `evaluate_state_remains_definition_time_only_when_runtime_shaped_fields_are_present` | valid state containing runtime-shaped nested definitions | exact state preserved; no execution, no waiting, no iteration | Unit |
| Deferred invalid expression | `evaluate_state_preserves_invalid_task_env_expression_without_eager_evaluation` | Task env contains syntactically invalid `Expression` literal | exact state preserved unchanged | Unit |

### `evaluate_state_machine`

| Behavior | Test name | Given | Then | Layer |
|---|---|---|---|---|
| Empty machine | `evaluate_state_machine_returns_empty_states_error_when_machine_is_empty` | `states = {}` | exact `StateMachine([EmptyStates])` | Integration |
| Missing `start_at` key | `evaluate_state_machine_returns_start_at_not_found_when_input_machine_is_invalid` | input machine whose `start_at` is not a state key | exact `StartAtNotFound` wrapped in `StateMachine(...)` | Integration |
| Missing transition target | `evaluate_state_machine_returns_transition_target_not_found_when_input_machine_is_invalid` | input machine whose `Next(target)` is not a state key | exact `TransitionTargetNotFound` wrapped in `StateMachine(...)` | Integration |
| Missing Choice target | `evaluate_state_machine_returns_choice_target_not_found_when_choice_rule_target_is_invalid` | input machine whose Choice rule `next` is not a state key | exact `ChoiceTargetNotFound` wrapped in `StateMachine(...)` | Integration |
| Missing Choice default | `evaluate_state_machine_returns_default_target_not_found_when_choice_default_target_is_invalid` | input machine whose Choice `default` is not a state key | exact `DefaultTargetNotFound` wrapped in `StateMachine(...)` | Integration |
| No terminal state | `evaluate_state_machine_returns_no_terminal_state_when_machine_has_no_terminal_state` | input machine with only non-terminal states and valid internal transitions | exact `NoTerminalState` wrapped in `StateMachine(...)` | Integration |
| Minimum valid machine | `evaluate_state_machine_preserves_minimum_valid_machine_when_single_terminal_state_is_present` | one-state terminal machine | exact machine preserved | Integration |
| Machine comment preservation | `evaluate_state_machine_preserves_machine_comment_when_comment_is_present` | valid terminal machine with `comment` | exact comment preserved on returned machine | Integration |
| Dense all-variant machine | `evaluate_state_machine_preserves_dense_all_variant_machine_when_timeout_is_u64_max` | validated flat machine containing every `StateKind` | exact discriminants, keys, timeout preserved | Integration |
| Timeout absent | `evaluate_state_machine_preserves_timeout_when_machine_timeout_is_valid` | validated machine with no timeout | exact `None` preserved | Integration |
| Timeout zero | `evaluate_state_machine_preserves_timeout_when_machine_timeout_is_valid` | validated machine with `timeout = 0` | exact `Some(0)` preserved | Integration |
| Timeout one | `evaluate_state_machine_preserves_timeout_when_machine_timeout_is_valid` | validated machine with `timeout = 1` | exact `Some(1)` preserved | Integration |
| Timeout max | `evaluate_state_machine_preserves_timeout_when_machine_timeout_is_valid` | validated machine with `timeout = u64::MAX` | exact `Some(u64::MAX)` preserved | Integration |
| Parallel recursion | `evaluate_state_machine_recurses_into_parallel_branches_when_parallel_states_are_present` | validated machine containing Parallel | nested branches recursively dispatched; order preserved | Integration |
| Parallel recursion failure proof | `evaluate_state_machine_returns_state_machine_error_when_parallel_branch_machine_is_invalid` | Parallel branch machine has missing `start_at` | exact nested `StateMachine(StartAtNotFound)` proves real branch recursion | Integration |
| Map recursion | `evaluate_state_machine_recurses_into_map_item_processor_when_map_states_are_present` | validated machine containing Map | nested item processor recursively dispatched | Integration |
| Map recursion failure proof | `evaluate_state_machine_returns_state_machine_error_when_map_item_processor_is_invalid` | Map item processor has missing `Next(target)` | exact nested `StateMachine(TransitionTargetNotFound)` proves real item-processor recursion | Integration |
| Parallel-inside-Map topology | `evaluate_state_machine_preserves_parallel_inside_map_topology_when_nested_machine_is_valid` | Map child machine contains Parallel | exact nested topology preserved | Integration |
| Map-inside-Parallel topology | `evaluate_state_machine_preserves_map_inside_parallel_topology_when_nested_machine_is_valid` | Parallel branch contains Map | exact nested topology preserved | Integration |
| Success requires `validate()` | `evaluate_state_machine_returns_only_validated_machine_when_recursive_dispatch_succeeds` | machine whose recursive dispatch succeeds | success only if returned machine validates | Integration |
| Nested deferred invalid expression | `evaluate_state_machine_preserves_nested_invalid_task_env_expression_without_eager_evaluation` | nested child Task env contains invalid `Expression` literal | exact nested machine preserved unchanged | Integration |
| Definition-time only | `evaluate_state_machine_remains_definition_time_only_when_nested_runtime_shaped_states_are_present` | nested machine with Task/Choice/Wait/Parallel/Map definitions | exact machine preserved; no execution/branch choice/wait/item iteration | Integration |

### `evaluate_state_kind`

| Behavior | Test name | Given | Then | Layer |
|---|---|---|---|---|
| Task arm success | `evaluate_state_kind_dispatches_task_arm_when_task_kind_is_valid` | valid `StateKind::Task` | exact Task kind preserved | Unit |
| Pass arm success | `evaluate_state_kind_dispatches_pass_arm_when_pass_kind_is_valid` | valid `StateKind::Pass` | exact Pass kind preserved | Unit |
| Choice arm success | `evaluate_state_kind_dispatches_choice_arm_without_selecting_a_branch` | valid `StateKind::Choice` | exact Choice kind preserved; no branch selected | Unit |
| Wait arm success | `evaluate_state_kind_dispatches_wait_arm_preserving_duration_discriminant` | valid `StateKind::Wait` | exact Wait kind preserved | Unit |
| Succeed arm success | `evaluate_state_kind_dispatches_succeed_arm_without_adding_transition` | valid `StateKind::Succeed` | exact Succeed kind preserved | Unit |
| Fail arm success | `evaluate_state_kind_dispatches_fail_arm_without_adding_transition` | valid `StateKind::Fail` | exact Fail kind preserved | Unit |
| Parallel arm success | `evaluate_state_kind_dispatches_parallel_arm_preserving_branch_order` | valid `StateKind::Parallel` | exact branch order preserved | Unit |
| Map arm success | `evaluate_state_kind_dispatches_map_arm_preserving_item_processor_and_tolerance` | valid `StateKind::Map` | exact topology/tolerance preserved | Unit |

### Raw seam builders

| Seam | Test name | Given | Then | Layer |
|---|---|---|---|---|
| `build_task_arm` timeout zero | `build_task_arm_returns_task_state_error_when_timeout_is_zero` | raw Task seam with `timeout = Some(0)` | exact `TaskState(TimeoutTooSmall(0))` | Unit |
| `build_task_arm` heartbeat zero | `build_task_arm_returns_task_state_error_when_heartbeat_is_zero` | raw Task seam with `heartbeat = Some(0)` | exact `TaskState(HeartbeatTooSmall(0))` | Unit |
| `build_task_arm` heartbeat equals timeout | `build_task_arm_returns_task_state_error_when_heartbeat_equals_timeout` | raw Task seam with `(5,5)` | exact `HeartbeatExceedsTimeout { heartbeat: 5, timeout: 5 }` | Unit |
| `build_task_arm` heartbeat above timeout | `build_task_arm_returns_task_state_error_when_heartbeat_exceeds_timeout` | raw Task seam with `(5,10)` | exact `HeartbeatExceedsTimeout { heartbeat: 10, timeout: 5 }` | Unit |
| `build_task_arm` empty env key | `build_task_arm_returns_task_state_error_when_env_key_is_empty` | raw Task seam with `env` containing `\"\"` | exact `EmptyEnvKey` | Unit |
| `build_choice_arm` empty choices | `build_choice_arm_returns_choice_state_error_when_choices_is_empty` | raw Choice seam with `choices = []` | exact `ChoiceState(EmptyChoices)` | Unit |
| `build_parallel_arm` empty branches | `build_parallel_arm_returns_parallel_state_error_when_branches_is_empty` | raw Parallel seam with `branches = []` | exact `ParallelState(EmptyBranches)` | Unit |
| `build_map_arm` below range | `build_map_arm_returns_map_state_error_when_tolerance_is_below_range` | raw Map seam with `-1.0` | exact `MapState(InvalidToleratedFailurePercentage(-1.0))` | Unit |
| `build_map_arm` above range | `build_map_arm_returns_map_state_error_when_tolerance_is_out_of_range` | raw Map seam with `101.0` | exact `MapState(InvalidToleratedFailurePercentage(101.0))` | Unit |
| `build_map_arm` NaN | `build_map_arm_returns_map_state_error_when_tolerance_is_not_finite` | raw Map seam with `NaN` | exact `MapState(NonFiniteToleratedFailurePercentage)` | Unit |
| `build_map_arm` positive infinity | `build_map_arm_returns_map_state_error_when_tolerance_is_positive_infinity` | raw Map seam with `+inf` | exact `MapState(NonFiniteToleratedFailurePercentage)` | Unit |

### Static guards

| Behavior | Test name | Given | Then | Layer |
|---|---|---|---|---|
| No legacy task surface | `public_dispatch_signatures_and_error_surface_never_reference_legacy_task_types` | compiled public API and error surface | no `crate::task::Task` appears | Static |
| Closed-union guard | `evaluate_state_kind_matches_closed_union_without_unsupported_fallback` | exhaustive dispatcher source | no wildcard / unsupported fallback exists | Static |

## Higher-tier verification status

- No bead-local `proptest`, `fuzz_target!`, `kani::`, or `cfg(kani)` assets are currently checked into this workspace for `twerk-bp2`.
- This bead therefore relies on the deterministic unit/integration suite, static guards, clippy, and the recorded Red Queen run for the verification actually executed in this session.
- The Kani exception is deliberate and documented in `.beads/twerk-bp2/implementation.md`.

## Mutation Checkpoints

1. Wrapper fields are dropped or rewritten -> killed by Task wrapper preservation.
2. Task payload fields are rewritten -> killed by Task payload preservation.
3. Pass `None`/`Some` handling collapses -> killed by split Pass result proofs.
4. Pass `Next`/`End` handling conflates -> killed by split Pass transition proofs.
5. Wait discriminants are normalized -> killed by the four explicit Wait proofs.
6. Fail literal combinations collapse -> killed by the four explicit Fail proofs.
7. Choice rules are sorted, deduplicated, or executed -> killed by Choice order/payload/declarative proofs.
8. Parallel branches are reversed or sorted -> killed by Parallel preservation and recursive topology proofs.
9. Map recursion is skipped -> killed by Map recursion and nested topology proofs.
10. Task timeout minimum check is removed -> killed by `build_task_arm_returns_task_state_error_when_timeout_is_zero`.
11. Task heartbeat minimum check is removed -> killed by `build_task_arm_returns_task_state_error_when_heartbeat_is_zero`.
12. Heartbeat relation mutates from `>=` to `>` or vice versa -> killed by one-below/equality/above-timeout seam proofs.
13. Empty Task env-key validation is deleted -> killed by `build_task_arm_returns_task_state_error_when_env_key_is_empty`.
14. Choice empty-check is deleted -> killed by `build_choice_arm_returns_choice_state_error_when_choices_is_empty`.
15. Parallel empty-check is deleted -> killed by `build_parallel_arm_returns_parallel_state_error_when_branches_is_empty`.
16. Map tolerance range/finite checks are weakened -> killed by the four raw Map seam proofs.
17. Post-dispatch `StateMachine::validate()` is removed -> killed by invalid-input machine topology proofs and validation-success gate.
18. Deferred invalid `Expression` payloads start evaluating eagerly -> killed by top-level and nested invalid-env preservation proofs.
19. Legacy `crate::task::Task` or unsupported fallback resurfaces -> killed by the two static guards.

## Notes

- Exact arm-error parity now lives only at the raw seam builders; public and validated internal entrypoints cover only reachable success or machine-validation behavior.
- The existing top-level and nested invalid Task env preservation proofs remain mandatory.
- Another scoped black-hat + test-review rerun should focus on whether the repaired suite matches this boundary map, not on unreachable constructor failures through validated entrypoints.
