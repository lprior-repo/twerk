# Structure: tw-jjhk

## Files in scope

- `AGENTS.md`
  Repo-wide operational contract and CRISPY repo mapping.
- `CLAUDE.md`
  Thin entrypoint doc pointing to canonical workflow surfaces.
- `.claude/CRISPY.md`
  Canonical staged workflow definition and artifact contract.
- `.claude/commands/crispy-qr.md`
  Q/R stage contract and `research.md` template.
- `.claude/commands/crispy-dspw.md`
  D/S/P/W stage contract and artifact outputs.
- `.claude/commands/verify.md`
  Verification gate for docs-only and source changes.
- `.claude/commands/review.md`
  Review command surface that must match the advertised review entrypoint.
- `.claude/commands/done.md`
  Landing command surface that must match the repo's canonical completion story.
- `.claude/commands/handoff.md`
  Session continuity command referenced by the CRISPY flow.
- `README.md`
  Contributor-facing summary that should point to the canonical workflow without restating it.

## Artifact roles

- `.beads/tw-jjhk/research.md`
  Fact base for this bead.
- `.beads/tw-jjhk/design.md`
  Invariants, failure modes, boundary changes, and acceptance criteria.
- `.beads/tw-jjhk/structure.md`
  File-level map for the alignment pass.
- `.beads/tw-jjhk/plan.md`
  Vertical-slice execution order and verification.

## Cross-reference edges that must stay aligned

- `AGENTS.md` <-> `CLAUDE.md` <-> `.claude/CRISPY.md`
  Same top-level stage order and same meaning for non-trivial workflow.
- `AGENTS.md` <-> `.claude/commands/done.md`
  Same landing/completion story.
- `AGENTS.md` <-> `CLAUDE.md` <-> `.claude/CRISPY.md` <-> `.claude/commands/review.md`
  Same review entrypoint and wording.
- `.claude/CRISPY.md` <-> `.claude/commands/crispy-qr.md`
  Same research-stage boundaries and artifact semantics.
- `.claude/CRISPY.md` <-> `.claude/commands/crispy-dspw.md`
  Same design/structure/plan outputs.
- `.claude/CRISPY.md` <-> `.claude/commands/handoff.md`
  Same session-continuity role.
- `README.md` <-> canonical workflow docs
  Pointer only, with no contradictory restatement.

## Code and test surface

- No runtime code files are in scope.
- No Rust types or functions are expected to change.
- Verification is docs-only consistency and reference checking unless scope expands.
