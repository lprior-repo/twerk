---
description: CRISPY stage for blocking questions and fact-only research
argument-hint: <bead-id>
---

# CRISPY Q/R

Arguments: $ARGUMENTS

Use this command for the Questions and Research stages only.

## Goal

Produce a clean research artifact without jumping ahead to design or implementation.

## Steps

1. Resolve the bead from `$ARGUMENTS`.
2. Read the bead context and only the files needed to understand current behavior.
3. Ask only blocking questions. If none exist, say so and continue.
4. If subagents are useful, split by bounded research area and ask for facts only.
5. Keep every prompt small. Do not exceed the repo's CRISPY instruction budget.
6. If the bead comes from `improvements.md`, pull only the relevant section.
7. Write or update `.beads/<bead-id>/research.md`.

## Required `research.md` sections

- Objective
- Scope boundaries
- Directly observed behavior
- Concrete file references
- Open questions
- Things not yet claimed as facts

## Rules

- No architecture proposals yet.
- No implementation plan yet.
- No broad repo summary if the bead only touches one area.
- Do not load large planning docs wholesale when one section will do.

## Stop Condition

Stop after `research.md` is complete and fact-based.
The next step is `/crispy-dspw <bead-id>`.
