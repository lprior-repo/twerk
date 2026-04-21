# Project Twerk - Claude AI Instructions

Use these files as the source of truth:

- `AGENTS.md` for repo rules, `bd` workflow, and landing constraints
- `.claude/CRISPY.md` for the staged low-instruction workflow
- `improvements.md` for the detailed repo-wide remediation plan; read only the relevant section for the current bead

Preferred flow for non-trivial work:

1. claim or create the bead in `bd`
2. `/crispy-qr <bead-id>`
3. `/crispy-dspw <bead-id>`
4. implement vertical slices
5. `/verify`
6. `/review --staged` or `/review --branch`
7. `/done`

## Subagent Contract

Subagents should receive one bounded task only.

Every subagent response should include:

- files read
- files changed, if any
- commands run
- artifacts produced
- open questions or risks

Only the primary agent should integrate outputs, close beads, and land work.
