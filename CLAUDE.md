# Project Twerk - Claude AI Instructions

This is a Rust port of a Go distributed task execution system. Key things for AI agents:

## Beads (Issue Tracking)

This project uses **bd** with a local Dolt database at `./twerk-database`.

**Dolt remote is already configured** - just run `bd dolt push` to sync.

## Closing Go-Portage Beads

When asked to close beads about porting from Go:

1. Search for the implementation in `crates/`
2. If found: `bd close <id> --reason "Implemented - found in crates/..." --json`
3. Push: `bd dolt push && git push`

## Non-Interactive Shell

Always use `-f` flag for `cp`, `mv`, `rm` to avoid prompts. See AGENTS.md for details.
