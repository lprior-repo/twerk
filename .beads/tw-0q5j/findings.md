# Findings: tw-0q5j - Test output formatting respects --format flag

## Summary
Wrote CLI tests for `--format json|table|quiet` flag behavior as specified in bead description.

## Tests Written
Created `crates/twerk-cli/tests/format_output_test.rs` with tests for:

1. **`--format json`** → valid JSON output
   - `queue_list_format_json_outputs_valid_json`
   - `trigger_list_format_json_outputs_valid_json`
   - `node_list_format_json_outputs_valid_json`
   - `task_get_format_json_outputs_valid_json`

2. **`--format table`** → aligned columns with headers
   - `queue_list_format_table_outputs_aligned_columns_with_headers`
   - `trigger_list_format_table_outputs_aligned_columns_with_headers`
   - `node_list_format_table_outputs_aligned_columns_with_headers`
   - `task_get_format_table_outputs_human_readable_format`

3. **`--format quiet`** → only IDs, one per line
   - `queue_list_format_quiet_outputs_only_ids_one_per_line`
   - `trigger_list_format_quiet_outputs_only_ids_one_per_line`
   - `node_list_format_quiet_outputs_only_ids_one_per_line`

4. **Invalid format** → error message
   - `queue_list_invalid_format_returns_error`
   - `trigger_list_invalid_format_returns_error`
   - `node_list_invalid_format_returns_error`
   - `task_get_invalid_format_returns_error`

## Current Implementation State
The `--format` flag described in the bead **does not exist** in the current codebase.
- Current CLI uses `--json` boolean flag (global) for JSON output
- Handlers accept `json_mode: bool` parameter
- No `--format` flag with `json|table|quiet` options exists in `crates/twerk-cli/src/commands.rs`

## Pre-existing Compilation Error
**Cannot run tests** - `twerk-common` has a missing `slot` module:
```
error[E0583]: file not found for module `slot`
  --> crates/twerk-common/src/lib.rs:12:1
   |
12 | pub mod slot;
```

This error exists in `origin/main` and is unrelated to the tests written.

## Test Pattern
Tests follow existing pattern in `e2e_cli_test.rs`:
- `cli_binary()` helper to locate compiled binary
- `run_cli()` helper to execute CLI with args
- `parse_json_output()` for JSON validation
- `is_valid_json()` for raw JSON string validation

## Recommendations
1. Implement `--format` flag in `Cli` struct (`commands.rs`) with `json|table|quiet` ValueEnum
2. Pass format option through to handlers alongside `json_mode`
3. Add "quiet" output mode handler (currently not implemented - only json/table)
4. Fix `twerk-common` missing `slot` module to enable test execution
