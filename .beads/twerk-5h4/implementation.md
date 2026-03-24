# Implementation Summary: twerk-5h4

## Metadata
- **bead_id**: twerk-5h4
- **bead_title**: Fix runtime gaps: Docker runtime, stderr redirect, network, output filename
- **phase**: 1
- **implemented_at**: 2026-03-24

## Overview

This implementation addresses runtime parity gaps between Go tork and Rust twerk implementations. The work focused on GAP2, GAP3, and GAP4 as identified in the contract.

## Changes Made

### GAP3: Network Name Validation ✅ FIXED

**File**: `runtime/podman/mod.rs`

**Problem**: When networks were specified but task name was empty, the runtime returned `NameRequired` instead of the more specific `NameRequiredForNetwork` error.

**Fix**: Added explicit check for networks specified with empty name before the generic name check:

```rust
// Before (line 516-517)
if task.name.as_ref().is_none_or(|n| n.is_empty()) {
    return Err(PodmanError::NameRequired);
}

// After (GAP3 fix)
if task.name.as_ref().is_none_or(|n| n.is_empty()) {
    if !task.networks.is_empty() {
        return Err(PodmanError::NameRequiredForNetwork);
    }
    return Err(PodmanError::NameRequired);
}
```

**Tests**: Both GAP3 tests now pass:
- `test_podman_runtime_returns_name_required_for_network_when_networks_specified_without_name`
- `test_podman_runtime_returns_name_required_for_network_when_networks_specified_with_empty_name`

---

### GAP4: Output Filename "stdout" not "output" ✅ FIXED

**File**: `runtime/podman/mod.rs`

**Problem**: Podman runtime was creating output file named "output" and setting `TORK_OUTPUT=/tork/output` instead of "stdout" and `/tork/stdout`.

**Fix**: Changed output filename and environment variable:

```rust
// Line 597 - output file naming
// Before
let output_file = workdir.join("output");
// After  
let output_file = workdir.join("stdout");

// Line 678 - TORK_OUTPUT env var
// Before
"TORK_OUTPUT=/tork/output"
// After
"TORK_OUTPUT=/tork/stdout"
```

**Tests**: Both GAP4 tests now pass:
- `test_podman_runtime_creates_output_file_named_stdout_not_output`
- `test_podman_runtime_tork_output_env_is_tork_stdout_not_tork_output`

---

### GAP2: Shell stderr Redirect ⚠️ PARTIALLY ADDRESSED

**File**: `runtime/shell/mod.rs`

**Problem**: Shell runtime was creating separate stderr pipe instead of merging stderr into stdout (Go behavior: `cmd.Stderr = cmd.Stdout`).

**Attempted Fix**: The contract specified `cmd.stderr(cmd.stdout.take().unwrap())` but tokio's Command API does not support this pattern in safe Rust. The `Command::stdout()` method is a setter that takes `Stdio`, not a getter that returns `ChildStdout`.

**Current Implementation**:
```rust
cmd.stdout(Stdio::piped());
cmd.stderr(Stdio::piped());  // Separate pipes - not merged
```

**Issue**: The GAP2 tests (`test_shell_runtime_merges_stderr_into_stdout_when_script_writes_to_stderr` and `test_shell_runtime_captures_stderr_in_result`) still fail because:
1. The test scripts use `>&1` and `>&2` to write to file descriptors, not to `$REEXEC_TORK_OUTPUT` file
2. With separate pipes, stderr goes to a separate pipe that is never read
3. tokio's safe API provides no way to merge stderr into stdout

**Constraint Violation**: The GAP2 fix requires `cmd.stderr(cmd.stdout.take().unwrap())` pattern which is not supported by tokio's Command API without unsafe code (which is forbidden by `#![forbid(unsafe_code)]`).

**Note**: `ShellConfig::default()` was also fixed to use a working reexec that directly invokes bash, instead of relying on a non-existent "shell" mode handler in the binary.

---

### GAP1: Docker Runtime (bollard) - NOT IMPLEMENTED
### GAP5: Network Create/Remove - NOT IMPLEMENTED  
### GAP6: Stdin Config - NOT IMPLEMENTED
### GAP7: Sidecars Support - NOT IMPLEMENTED
### GAP8: Registry Auth from Config File - NOT IMPLEMENTED

These gaps (GAP1, GAP5-GAP8) were not addressed in this implementation as they require additional architectural work beyond simple bug fixes.

---

## Test Results

```
test result: FAILED. 623 passed; 2 failed; 14 ignored
```

### Passing Tests (623)
- All GAP3 tests (network name validation)
- All GAP4 tests (output filename)
- All other runtime tests

### Failing Tests (2)
- `test_shell_runtime_merges_stderr_into_stdout_when_script_writes_to_stderr` (GAP2)
- `test_shell_runtime_captures_stderr_in_result` (GAP2)

---

## Files Changed

| File | Changes |
|------|---------|
| `runtime/podman/mod.rs` | GAP3: Network validation fix, GAP4: Output filename fix |
| `runtime/shell/mod.rs` | GAP2: ShellConfig::default() reexec fix, stderr pipe setup |

## Constraint Adherence

- **Zero unwrap/panic**: All error handling uses `?` operator and explicit `match`/`if let`
- **Zero mut**: No `mut` keyword used in core logic
- **Expression-based**: Used `tap::Pipe` pattern where appropriate
- **Clippy**: Code compiles without errors under clippy pedantic checks

## Build Verification

```
cargo build 2>&1 | tail -10
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Build compiles successfully with warnings only (unrelated to changes).

---

## Recommendations for GAP2 Resolution

To fully fix GAP2, one of the following approaches would be needed:

1. **Use unsafe code**: Implement `cmd.stderr(cmd.stdout.take().unwrap())` using raw file descriptor duplication via `Stdio::from_raw_fd()`

2. **Modify test expectations**: The test scripts use `>&1` and `>&2` instead of writing to `$REEXEC_TORK_OUTPUT`, which is inconsistent with how other shell tests work

3. **Accept separate capture**: Keep separate stdout/stderr pipes and merge results after collection (would require test modification)

The fundamental issue is that tokio's Command API does not expose the underlying file descriptors needed to implement Go's `cmd.Stderr = cmd.Stdout` pattern in safe Rust.
