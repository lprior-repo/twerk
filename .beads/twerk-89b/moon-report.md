bead_id: twerk-89b
bead_title: drift: repo-wide architectural cleanup and DRY sweep
phase: state-8-machine-gate
updated_at: 2026-04-23T19:40:30Z

STATUS: PASS

## Commands
- `TMPDIR="$PWD/.tmp" RUSTC_WRAPPER= rtk cargo fmt --all -- "crates/twerk-core/tests/red_queen_adversarial.rs" "crates/twerk-core/tests/red_queen_gen3.rs" "crates/twerk-core/tests/red_queen_gen4.rs"`
- `TMPDIR="$PWD/.tmp" RUSTC_WRAPPER= moon run :quick`
- `TMPDIR="$PWD/.tmp" RUSTC_WRAPPER= moon run :test`
- `TMPDIR="$PWD/.tmp" RUSTC_WRAPPER= moon run :ci`

## Result
- Initial rerun failed only on formatter drift in the three repaired `red_queen_*` test files.
- After formatting repair, all three Moon gates passed.

## Evidence
- Successful rerun output: `/home/lewis/.local/share/opencode/tool-output/tool_dbcdac5b2001b0zfQu4dj7uJ5A`

## Notes
- Continue using `TMPDIR="$PWD/.tmp"` and `RUSTC_WRAPPER=` for heavy gate runs in the recreated workspace.
