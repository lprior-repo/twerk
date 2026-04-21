# CRISPY Workflow

Use CRISPY for non-trivial work in this repo.

CRISPY means:

- `Q` Questions
- `R` Research
- `D` Design
- `S` Structure
- `P` Plan
- `W` Work tree
- `I` Implement
- `PR` Verify, review, and land

## Hard Rules

- Keep each prompt under roughly 40 discrete instructions.
- Do not use one mega-prompt to drive the whole task.
- Do not mix current-state research with requested intent until the research artifact exists.
- Treat every generated plan or patch as a hypothesis that must be verified.
- Durable tracking stays in `bd`, not ad hoc markdown task lists.
- CRISPY artifacts belong under `.beads/<bead-id>/`.
- Implement vertical slices, not broad horizontal layers.
- `improvements.md` is the repo-wide remediation plan when a bead comes from the audit backlog.
- Pull only the relevant section of large docs such as `improvements.md`, never the whole file by default.

## Repo Mapping

1. Claim or create the bead in `bd`.
2. Run `/crispy-qr <bead-id>`.
3. Run `/crispy-dspw <bead-id>`.
4. Implement one vertical slice at a time.
5. Run `/verify`.
6. Run `/review --staged` or `/review --branch`.
7. Run `/done`.
8. Use `/handoff` instead of guessing at session continuity.

## Stage Outputs

### Q: Questions

- Ask only questions that block execution.
- If there are no blockers, state that and continue.

### R: Research

Write `.beads/<bead-id>/research.md`.

The file should contain only:

- directly observed facts
- concrete file paths
- current behavior
- known gaps
- unanswered questions

Do not put proposed architecture or implementation steps in `research.md`.

### D: Design

Write `.beads/<bead-id>/design.md`.

Keep it short enough to review quickly. Target about 200 lines or less.

Include:

- problem statement
- invariants
- failure modes
- interface or contract changes
- acceptance criteria

### S: Structure

Write `.beads/<bead-id>/structure.md`.

This is the header-file view of the change:

- files to touch
- new or changed types
- new or changed functions
- tests to add or update
- boundaries that must stay unchanged

### P: Plan

Write `.beads/<bead-id>/plan.md`.

The plan must be vertical slices only. Each slice must:

- be independently testable
- state its verification step
- have an explicit stop condition

### W: Work tree

Record the isolation decision in `plan.md`:

- stay in the current tree, or
- use a dedicated branch or worktree

Only create extra isolation when concurrency or risk justifies it.

### I: Implement

- Implement one slice.
- Verify it.
- Then move to the next slice.
- If new work is discovered, file it in `bd`.

### PR: Verify, Review, Land

- `/verify` proves the change, with real commands.
- `/review` inspects the diff.
- `/done` is the only landing command.

## Subagent Contract

Subagents should get one bounded job only: one research area, one design check, or one implementation slice.

Every subagent handoff must include:

- files read
- files changed, if any
- commands run
- artifacts produced
- risks or open questions

The primary agent owns final integration, verification, bead closure, and landing.
