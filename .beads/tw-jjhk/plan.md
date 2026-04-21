# Plan: tw-jjhk

## Worktree decision

Stay in the current tree.

Reason: this bead is a small docs and command-doc alignment pass with a tight file set and no risky source-code change.

## Slice order

### Slice 1: Align top-level workflow surfaces

Files:

- `AGENTS.md`
- `CLAUDE.md`
- `.claude/CRISPY.md`

Verification:

- Read all three and confirm they describe the same ordered non-trivial workflow.
- Confirm the review step is described consistently.
- Confirm the first step around bead claim/create is either consistently stated or intentionally delegated.

Stop condition:

- A reader can start from any of the three files without encountering conflicting stage order or review wording.

### Slice 2: Align command surfaces with the repo workflow contract

Files:

- `.claude/commands/crispy-qr.md`
- `.claude/commands/crispy-dspw.md`
- `.claude/commands/verify.md`
- `.claude/commands/review.md`
- `.claude/commands/done.md`
- `.claude/commands/handoff.md`

Verification:

- Read the command docs and confirm they match the top-level workflow wording from slice 1.
- Confirm `/done` no longer leaves landing semantics ambiguous.
- Confirm `/verify` still defines a truthful docs-only verification path.

Stop condition:

- Command docs support the same CRISPY story and do not contradict repo-level guidance on stage boundaries, review mode, or landing.

### Slice 3: Patch contributor-facing summary without creating a fourth workflow spec

Files:

- `README.md`

Verification:

- Confirm contributor guidance stays brief.
- Confirm it points at the canonical workflow surfaces instead of restating the workflow in full.
- Confirm referenced files and commands exist.

Stop condition:

- `README.md` contains only an accurate pointer and introduces no new workflow drift.

## Global stop conditions

- Stop if landing semantics between `AGENTS.md` and `.claude/commands/done.md` cannot be reconciled cleanly without broader process input.
- Stop if fixing one workflow surface would require broad unrelated doc rewrites.
- Stop after the docs are aligned and docs-only verification passes; do not expand into source-code changes.
