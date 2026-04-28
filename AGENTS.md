# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready --json           # Find available work
bd show <id>              # View issue details
bd update <id> --claim    # Claim work atomically
bd close <id>             # Complete work
bd dolt push              # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags to avoid hanging on confirmation prompts.**

```bash
cp -f source dest         # NOT: cp source dest
mv -f source dest         # NOT: mv source dest
rm -f file                # NOT: rm file
rm -rf directory          # NOT: rm -r directory
cp -rf source dest        # NOT: cp -r source dest
```

`scp`/`ssh` → `-o BatchMode=yes`; `apt-get` → `-y`; `brew` → `HOMEBREW_NO_AUTO_UPDATE=1`.

## Issue Tracking with bd (beads)

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:f65d5d33 -->

**Use bd for ALL task tracking. Never use markdown TODOs or external trackers.**

### Core Workflow

1. Check `bd ready --json` for unblocked issues
2. Claim: `bd update <id> --claim --json`
3. Work: implement, test, document
4. Discover new work? `bd create "title" --description="..." -t <type> -p <0-4> --deps discovered-from:<parent-id> --json`
5. Complete: `bd close <id> --reason "Done" --json`

### Issue Types & Priorities

| Type | Priority | Meaning |
|------|----------|---------|
| `bug` | `0` | Critical: security, data loss, broken builds |
| `feature` | `1` | High: major features, important bugs |
| `task` | `2` | Medium (default): tests, docs, refactoring |
| `epic` | `3` | Low: polish, optimization |
| `chore` | `4` | Backlog: future ideas, maintenance |

### Issue Management

- Use `--acceptance` and `--design` fields; validate with `--validate`
- Lifecycle: `bd defer <id>` / `bd supersede <id>` / `bd stale` / `bd orphans` / `bd lint`
- Flag human decisions: `bd human <id>`
- Structured workflows: `bd formula list` / `bd mol pour <name>`

### Sync

Each write auto-commits to Dolt. Use `bd dolt push`/`bd dolt pull` for remote sync.

<!-- END BEADS INTEGRATION -->

## Session Completion

Work is NOT complete until `git push` succeeds.

**MANDATORY:**

1. File follow-up issues with `bd create`
2. Run quality gates (tests, linters, builds) if code changed
3. Update issues: close finished, update in-progress
4. Push:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. Clean up (stashes, pruned branches), verify all changes pushed

**RULES:** Never stop before pushing. Never say "ready to push" — YOU must push. Resolve and retry until it succeeds.
