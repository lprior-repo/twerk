# Research: tw-jjhk

## Objective

Adopt a repo-native CRISPY workflow for subagent-driven work with strict instruction budgets, staged artifacts, and integration with the existing repo guidance surfaces.

Directly observed source for the objective:

- `bd show tw-jjhk --json`

## Scope boundaries

This research pass is limited to the workflow and command-doc surfaces that directly govern staged agent work:

- `AGENTS.md`
- `CLAUDE.md`
- `.claude/CRISPY.md`
- `.claude/commands/crispy-qr.md`
- `.claude/commands/crispy-dspw.md`
- `.claude/commands/verify.md`
- `.claude/commands/review.md`
- `.claude/commands/done.md`
- `.claude/commands/handoff.md`
- `README.md`

This research does not yet propose changes to the workflow. It records current behavior, constraints, and drift only.

## Directly observed behavior

- `AGENTS.md:104-123` defines a staged CRISPY flow for non-trivial work and states that durable tracking stays in `bd`, CRISPY artifacts live under `.beads/<bead-id>/`, prompts should stay under roughly 40 instructions, research must stay fact-based, and large planning docs should not be loaded wholesale.
- `CLAUDE.md:3-15` points to `AGENTS.md` and `.claude/CRISPY.md` as the sources of truth and lists the preferred non-trivial flow as `/crispy-qr`, `/crispy-dspw`, implement vertical slices, `/verify`, `/review`, `/done`.
- `.claude/CRISPY.md:27-36` maps the repo flow as claim/create bead, `/crispy-qr`, `/crispy-dspw`, implement one vertical slice at a time, `/verify`, `/review --staged` or `/review --branch`, `/done`, and `/handoff` for session continuity.
- `.claude/CRISPY.md:45-57` states that `research.md` must contain directly observed facts, concrete file paths, current behavior, known gaps, and unanswered questions, and must not contain proposed architecture or implementation steps.
- `.claude/commands/crispy-qr.md:10-44` defines the Q/R stage as blocking questions plus fact-only research and requires `.beads/<bead-id>/research.md` with sections for objective, scope boundaries, directly observed behavior, concrete file references, open questions, and things not yet claimed as facts.
- `.claude/commands/crispy-dspw.md:10-48` defines the next stage as writing `design.md`, `structure.md`, and `plan.md` after research exists and blocking questions are resolved or recorded.
- `.claude/commands/verify.md:10-31` defines `/verify` as the verification gate and requires real command output, explicit pass/fail reporting, and no move to `/done` before verification and review are complete.
- `.claude/commands/review.md:11-24` defines multiple review modes, including `--staged` and `--branch`.
- `.claude/commands/done.md:13-49` defines `/done` as a `gt done` based landing step that expects a clean git tree and at least one local commit, then pushes/submits work.
- `.claude/commands/handoff.md:11-24` defines handoff as a session continuity command rather than an implementation or verification stage.
- `README.md:74-79` already gives contributor guidance for build and verification commands, but it does not define a staged CRISPY execution flow.

## Concrete file references

- `AGENTS.md:95-123`
- `CLAUDE.md:3-29`
- `.claude/CRISPY.md:27-57`
- `.claude/commands/crispy-qr.md:10-44`
- `.claude/commands/crispy-dspw.md:10-48`
- `.claude/commands/verify.md:10-40`
- `.claude/commands/review.md:11-24`
- `.claude/commands/done.md:13-49`
- `.claude/commands/handoff.md:11-24`
- `README.md:74-79`

## Open questions

- `AGENTS.md:213-237` still describes session completion as `git pull --rebase`, `bd dolt push`, and `git push`, while `.claude/commands/done.md:30-49` describes landing through `gt done`. It is not yet resolved which landing mechanism is canonical for CRISPY completion in this repo.
- `CLAUDE.md:10-15` says `/review`, while `AGENTS.md:110-115` and `.claude/CRISPY.md:33-35` explicitly call out `/review --staged` or `/review --branch`. It is not yet resolved whether `CLAUDE.md` should stay broader or match the more specific review variants.
- `.claude/CRISPY.md:29-30` includes explicit claim/create of the bead before `/crispy-qr`, while `AGENTS.md:108-115` and `CLAUDE.md:8-15` start at `/crispy-qr <bead-id>`. It is not yet resolved whether those surfaces should all describe the same first step.

## Things not yet claimed as facts

- No claim is made yet that the current CRISPY docs are fully aligned; only the specific drift listed above has been directly observed.
- No claim is made yet that additional command files or workflow changes are necessary; that belongs in design and plan, not research.
- No claim is made yet that the repo's landing flow should prefer `gt done` or raw git push semantics; the current surfaces differ and that conflict is only recorded here.
- No claim is made yet that `README.md` should include CRISPY workflow content; this research only records that the current README remains high-level contributor guidance.
