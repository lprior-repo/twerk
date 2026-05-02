# Findings: tw-g7yn - vo-executor SubprocessError missing variants

## Issue Summary
- **Bead**: tw-g7yn
- **Title**: twerk: Fix vo-executor SubprocessError missing variants
- **Description**: CRATE: vo-executor depends on vo-ipc
- **Priority**: P0

## Analysis

### Problem Identified
In `/home/lewis/gt/veloxide/polecats/brahmin/veloxide/crates/vo-executor/src/dispatch.rs`, the code calls `run_subprocess()` from vo-ipc which returns `Result<SubprocessOutput, IpcError>`. The match statement on line 110-122 only explicitly handles two IpcError variants:

1. `IpcError::Timeout` -> `ExecuteNodeError::TimeoutExceeded`
2. `IpcError::ProcessFailed` -> `ExecuteNodeError::TransientError`

All other IpcError variants (WaitFailed, StderrReadFailed, SignalFailed, IoError, etc.) fell through to the generic `other` case which just formats them as a string, losing important error information.

### IpcError Variants Missing Explicit Handling
The following IpcError variants were not being properly distinguished:
- `WaitFailed` - failed to wait for subprocess
- `StderrReadFailed` - failed to capture stderr
- `SignalFailed` - failed to signal subprocess (e.g., SIGTERM, SIGKILL)
- `IoError` - IO errors during subprocess operations

## Changes Made

### File 1: errors.rs
Added new variants to `ExecuteNodeError` enum in `/home/lewis/gt/veloxide/polecats/brahmin/veloxide/crates/vo-executor/src/errors.rs`:

```rust
/// Failed to wait for subprocess.
#[error("Wait failed: {detail}")]
WaitFailed { detail: String },

/// Failed to capture stderr from subprocess.
#[error("Stderr read failed: {detail}")]
StderrReadFailed { detail: String },

/// Failed to signal subprocess (e.g., SIGTERM, SIGKILL).
#[error("Signal failed: {detail}")]
SignalFailed { detail: String },

/// IO error during subprocess operation.
#[error("IO error: {detail}")]
IoError { detail: String },
```

### File 2: dispatch.rs
Updated the match statement in `/home/lewis/gt/veloxide/polecats/brahmin/veloxide/crates/vo-executor/src/dispatch.rs` to explicitly handle the new error variants:

```rust
IpcError::WaitFailed { detail } => ExecuteNodeError::WaitFailed {
    detail: detail.clone(),
},
IpcError::StderrReadFailed { detail } => ExecuteNodeError::StderrReadFailed {
    detail: detail.clone(),
},
IpcError::SignalFailed { detail } => ExecuteNodeError::SignalFailed {
    detail: detail.clone(),
},
IpcError::IoError(ref e) => ExecuteNodeError::IoError {
    detail: e.to_string(),
},
```

### File 3: errors.rs test
Updated `all_execute_node_error_variants_construct` test to include construction of the new variants.

## Build Status
The vo-executor crate has pre-existing compilation errors unrelated to these changes:
- `NodeKind::Router` not covered in match (line 49)
- Field access issues with `effect_kind` and `params` (lines 167-168)

These are pre-existing issues in the veloxide codebase, not caused by the changes made for this bead.

## Code Location
The changes are in the veloxide repo at:
`/home/lewis/gt/veloxide/polecats/brahmin/veloxide/crates/vo-executor/`

Note: This is a cross-repo issue - the bead is filed in twerk's issue tracker but the code is in veloxide.

## Completion Status
- [x] Analyzed issue and identified missing variants
- [x] Added WaitFailed, StderrReadFailed, SignalFailed, IoError to ExecuteNodeError
- [x] Updated dispatch.rs to handle new variants
- [x] Updated test to cover new variants
