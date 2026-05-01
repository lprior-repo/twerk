# Findings: tw-z92p - Test check command detects circular dependencies

## Task Summary
Test cycle detection in workflow definitions. Write tests for:
1. workflow with step A->B->C->A -> exit 1, cycle reported
2. workflow with no cycles -> exit 0
3. self-referencing step -> exit 1
4. disconnected subgraph with cycle -> exit 1

## Codebase Analysis

### Location of Cycle Detection
- **Validation module**: `crates/twerk-core/src/asl/validation.rs`
- **Validation tests**: `crates/twerk-core/tests/asl_validation_test.rs`
- **Note**: The bead description references `check()` in `crates/twerk-cli/src/commands.rs`, but no such function exists there. The CLI has no `check` subcommand.

### Existing Tests Found
1. `linear_chain_is_clean` - verifies no cycles in A→B→C chain (covers case 2)
2. `simple_cycle_detected` - A→B→A cycle (covers case 1 partially - 2-step not 3-step)
3. `self_loop_detected` - A→A self-reference (covers case 3)

### Tests Added
1. `three_step_cycle_detected` - Added test for A→B→C→A 3-step cycle
2. `disconnected_subgraph_with_cycle_detected` - Added test for disconnected C→D→C cycle with main path A→B

### Build Issue
The codebase does NOT compile due to a missing module:
```
error[E0583]: file not found for module `slot`
  --> crates/twerk-common/src/lib.rs:12:1
   |
12 | pub mod slot;
```

The `slot` module is declared in `twerk-common/src/lib.rs` but the file `crates/twerk-common/src/slot.rs` (or `mod.rs`) does not exist.

### Test Execution Status
Cannot run tests due to build failure in `twerk-common`.

## Verification Commands
```bash
cargo test -p twerk-core --test asl_validation_test
```

## Files Modified
- `crates/twerk-core/tests/asl_validation_test.rs`: Added 2 new tests

## Recommendations
1. Fix missing `slot` module in `twerk-common` crate to enable test execution
2. Consider adding a `check` CLI subcommand that invokes the validation analysis
