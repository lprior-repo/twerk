---
description: Run the verification gate before review or landing
argument-hint: [scope]
---

# Verify

Arguments: $ARGUMENTS

Run the smallest honest verification set for the current change.

## Steps

1. Determine the change class:
   - source code
   - docs or command docs only
   - mixed
2. If source code changed, prefer the repo's standard gate:
   - `moon run :ci-source`
3. If that is too broad for the current loop, run the narrowest truthful check set and explain why.
4. If only docs or command docs changed, verify:
   - referenced files exist
   - referenced commands exist
   - updated docs do not conflict with `AGENTS.md`, `CLAUDE.md`, or repo-local workflow docs
5. Report exact commands run, exit codes, and blockers.

## Rules

- Never claim verification without real command output.
- Do not skip failing checks without documenting the reason.
- Do not move to `/done` before verification and review are complete.

## Output

Report:

- commands run
- pass or fail status
- remaining blockers
- whether the change is ready for `/review`
