---
description: CRISPY stage for design, structure, plan, and worktree decision
argument-hint: <bead-id>
---

# CRISPY D/S/P/W

Arguments: $ARGUMENTS

Use this command after research is complete.

## Goal

Turn the research artifact into a small, reviewable implementation packet before coding starts.

## Preconditions

- `.beads/<bead-id>/research.md` exists
- blocking questions are resolved or explicitly recorded

## Steps

1. Read the bead, the research artifact, and only the files required for design decisions.
2. Write `.beads/<bead-id>/design.md` with invariants, failure modes, boundary changes, and acceptance criteria.
3. Write `.beads/<bead-id>/structure.md` with the target files, types, functions, and tests.
4. Write `.beads/<bead-id>/plan.md` as vertical slices only.
5. Record the worktree or branch decision inside `plan.md`.

## Rules

- Keep `design.md` short enough for a fast human review.
- Do not produce a giant implementation essay.
- Do not switch to horizontal planning.
- Each plan slice must name its verification step.
- If subagents are used here, give each one a single bounded review problem.

## Required `plan.md` content

- slice order
- files per slice
- verification per slice
- stop condition per slice
- worktree decision

## Stop Condition

Stop after the design, structure, and plan artifacts are written.
Implementation begins only after the packet is reviewable.
