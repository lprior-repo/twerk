# Project Twerk - Claude AI Instructions

This is a Rust port of a Go distributed task execution system. Key things for AI agents:

## Beads (Issue Tracking)

This project uses **bd** with Dolt database at `.beads/dolt/` and a local Dolt database at `./twerk-database`.

**IMPORTANT**: If `bd dolt push` fails, use dolt CLI directly from `.beads/dolt/`:

```bash
cd .beads/dolt
dolt push origin main
```

## Closing Go-Portage Beads

When asked to close beads about porting from Go:

1. Search for the implementation in `crates/`
2. If found: `bd close <id> --reason "Implemented - found in crates/..." --json`
3. Push: `cd .beads/dolt && dolt push origin main && git push`

## Non-Interactive Shell

Always use `-f` flag for `cp`, `mv`, `rm` to avoid prompts. See AGENTS.md for details.
