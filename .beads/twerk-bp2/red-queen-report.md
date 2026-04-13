---
bead_id: twerk-bp2
bead_title: Eval Engine: State-based evaluation dispatch
phase: state-5-red-queen-post-architectural-drift
updated_at: 2026-04-13T11:08:06Z
---

# Verdict

**CROWN DEFENDED** — post-architectural-drift adversarial replay found no surviving defects. The champion still passes `cargo test -p twerk-core state_dispatch --quiet` (71/71 passing), and every challenger was killed by the existing suite:

- `G1-A` metadata-integrity (`evaluate_state` without `attach_state_metadata`) → caught by 30 failing tests.
- `G1-B` metadata-integrity (`evaluate_state_machine` without `attach_machine_metadata`) → caught by 8 failing tests.
- `G2-A` machine-validation (removed post-dispatch `validate_machine`) → caught by 8 failing tests.
- `G3-A` recursive-dispatch (bypassed Parallel branch recursion) → caught by `evaluate_state_machine_returns_state_machine_error_when_parallel_branch_machine_is_invalid`.
- `G3-B` recursive-dispatch (bypassed Map item-processor recursion) → caught by `evaluate_state_machine_returns_state_machine_error_when_map_item_processor_is_invalid`.

# Defects Found

None.
