# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN BEADS INTEGRATION profile:full hash:d4f96305 -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

## CRISPY Workflow

For non-trivial work, use the staged CRISPY flow in `.claude/CRISPY.md`.

Before entering the stages below, claim or create the bead in `bd` using the workflow above.

Repo mapping:

1. Claim or create the bead in `bd`
2. `/crispy-qr <bead-id>` for blocking questions and fact-only research
3. `/crispy-dspw <bead-id>` for design, structure, plan, and worktree decision
4. Implement one vertical slice at a time
5. `/verify`
6. `/review --staged` or `/review --branch`
7. `/done`
8. `/handoff` for session continuity

Rules:

- Durable task tracking stays in `bd`
- CRISPY markdown artifacts live under `.beads/<bead-id>/`; they are bead-local working artifacts, not a second issue tracker
- Keep stage prompts under roughly 40 discrete instructions
- Do not mix requested intent into research until a fact-based `research.md` exists
- `improvements.md` is the detailed repo-wide remediation plan; when it is relevant, pull only the section needed for the current bead
- Do not load large repo plans like `improvements.md` wholesale when one relevant section will do

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Dolt Remote Configuration

bd stores its Dolt database in `.beads/dolt/`. The remote is **DoltHub**: `https://doltremoteapi.dolthub.com/priorlewis43/twerk-database`

**Check current remote:**
```bash
cd .beads/dolt && dolt remote -v
# Expected: origin https://doltremoteapi.dolthub.com/priorlewis43/twerk-database {}
```

**If `bd dolt push` or `dolt push origin main` fails:**

1. **"permission denied"**: Credentials issue. Check DoltHub login or use force:
   ```bash
   cd .beads/dolt
   dolt push origin main --force
   ```

2. **"no common ancestor"**: History mismatch. Force push:
   ```bash
   dolt push origin main --force
   ```

3. **"database not found"**: Dolt server isn't running or metadata.json is wrong. Check:
   ```bash
   cat .beads/metadata.json  # should have database: "twerk"
   dolt sql -q "SHOW DATABASES;"  # should list twerk
   ```

4. **Remote wrong**: Fix with:
   ```bash
   cd .beads/dolt
   dolt remote remove origin
   dolt remote add origin https://doltremoteapi.dolthub.com/priorlewis43/twerk-database
   dolt push origin main
   ```

**Standard push workflow:**
```bash
git pull --rebase
cd .beads/dolt && dolt push origin main
git push
```

### Closing Go-Portage Beads

When closing beads related to porting from Go (e.g., "Implement X from Go's internal/..."):

1. **Verify implementation exists** - Check if the Rust code was actually implemented:
   ```bash
   # Look for the implementation file
   ls -la crates/*/src/<implementation>.rs
   
   # Or search for the feature
   grep -r "function_name" crates/ --include="*.rs"
   ```

2. **Close with verification reason**:
   ```bash
   bd close <id> --reason "Implemented - found in crates/..." --json
   ```

3. **Push changes**:
   ```bash
   bd dolt push
   git push
   ```

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Landing the Plane (Session Completion)

The `/done` command is the workflow wrapper for this stage, but the guarantees below still apply.

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
