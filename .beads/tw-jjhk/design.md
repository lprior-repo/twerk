# Design: tw-jjhk

## Goal

Align the repo's CRISPY workflow surfaces so a reader gets one consistent staged workflow for non-trivial work without having to reconcile conflicting instructions.

## Invariants

- `AGENTS.md` and `.claude/CRISPY.md` remain the canonical workflow sources.
- `bd` remains the only durable task tracker.
- `.beads/<bead-id>/` artifacts remain bead-local working docs, not a second tracker.
- `research.md` stays fact-only and always precedes design and plan.
- `/verify` requires real command output before review or landing.
- The staged flow for non-trivial work is ordered and readable end-to-end.

## Boundary changes

- This bead is docs and command-doc alignment only.
- No product behavior, runtime behavior, or source-code semantics should change.
- `README.md` should stay a short pointer, not become another full workflow spec.

## Failure modes to avoid

- Top-level docs describe different first steps for the same workflow.
- `CLAUDE.md` stays broader than the canonical review step and reintroduces drift.
- Landing remains ambiguous between raw git push guidance and `gt done` guidance.
- Command docs drift from the repo-level CRISPY story.
- Bead-local artifacts are described as durable tracking instead of working docs.
- Verification language becomes aspirational instead of command-backed.

## Acceptance criteria

- A reader starting in `AGENTS.md`, `CLAUDE.md`, or `.claude/CRISPY.md` sees the same stage order for non-trivial work.
- Review guidance is consistent across the top-level workflow surfaces.
- Landing guidance is explicit and no longer ambiguous.
- The roles of `research.md`, `design.md`, `structure.md`, and `plan.md` are unambiguous.
- Docs-only verification can confirm that referenced files and commands exist and that the workflow docs do not contradict each other.

## Non-goals

- Redesigning the repo's broader contributor documentation.
- Changing build, test, or runtime behavior.
- Expanding CRISPY into a larger process than the current staged workflow requires.
