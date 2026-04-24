# tw-j7c6 Findings

## Task
Create script to bulk-delete stale remote branches. Make idempotent and safe. Add to bd config.

## What Was Done
1. Created `scripts/cleanup-branches.sh` — a bash script for bulk-deleting stale remote branches
2. Created `.beads/formulas/branch-cleanup.formula.toml` — bd formula for triggering cleanup via `bd formula`

## Script: `scripts/cleanup-branches.sh`

### Patterns (default)
- `polecat/*` — Polecat worktree branches
- `tw-polecat/*` — Tw-era polecat branches
- `fix/*` — Fix branches
- `temp-*`, `test-*`, `final-*`, `merge-*` — Temporary branches

### Modes
- **Dry-run** (default): Lists what would be deleted, skips nothing
- **`--run`**: Actually deletes branches
- **`--force`**: Deletes unmerged branches too (default: merged-only)

### Safety Features
- Dry-run by default — no accidental deletions
- Only deletes branches merged into main (unless --force)
- Idempotent — safe to run multiple times
- Prunes stale tracking refs after deletion
- Custom patterns supported as positional args

### Current Branch State
- 42 remote branches total
- 7 merged into main (safe to delete)
- 32 unmerged (need --force to delete)
- 3 other (origin, origin/main, origin/pushfix-trigger-contract)

### Test Results
- Dry-run: Correctly identified 39 matching branches
- 7 merged branches listed for deletion
- 32 unmerged branches correctly skipped
- Script syntax validated with `bash -n`

## Formula: `branch-cleanup`
- Tier: project (lives in repo at `.beads/formulas/`)
- Steps: dry-run → review → execute → optional force-execute
- Invoke via: `bd formula show branch-cleanup`
