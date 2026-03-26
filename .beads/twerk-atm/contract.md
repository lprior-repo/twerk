# Contract Specification: Graceful Shutdown for Runtime Adapters

## Context

- **Feature**: Implement proper graceful shutdown for shell, podman, and mock runtime adapters
- **Bead ID**: twerk-atm
- **Priority**: 0 (Critical)
- **Domain Terms**:
  - `RuntimeAdapter`: Interface for executing tasks in different environments (shell, podman, docker)
  - `Task`: Unit of work with lifecycle state (CREATED, PENDING, RUNNING, STOPPED, COMPLETED, FAILED)
  - `BoxedFuture`: Async operation returning `Result<T>` for railway-oriented error handling
  - `Graceful Shutdown`: Controlled termination with signal propagation, resource cleanup, and timeout handling

- **Current State**:
  - `shell.rs:72` - `stop()` returns `Ok(())` without terminating processes
  - `podman.rs:46` - `stop()` returns `Ok(())` without stopping containers
  - `runtime_adapter.rs:67` - `MockRuntime::stop()` returns `Ok(())` without cleanup

- **Assumptions**:
  1. Each runtime adapter tracks running processes/containers via internal state
  2. Shell adapter spawns child processes that must be terminated
  3. Podman adapter creates containers that must be stopped and removed
  4. Timeout should be configurable (default: 30s for graceful, 5s for force)
  5. Signal propagation: SIGTERM first, SIGKILL on timeout
  6. Resource cleanup: temporary files, mounted volumes, container networks

- **Open Questions**:
  1. Q1: Should `stop()` return the exit code of the terminated process?
     - **Answer**: Yes, for observability (see Error Taxonomy: `ExitCode(i32)`)
  2. Q2: What is the default timeout for graceful shutdown?
     - **Answer**: 30 seconds (configurable via `TASK_STOP_TIMEOUT` env var)
  3. Q3: Should `stop()` be idempotent (safe to call multiple times)?
     - **Answer**: Yes - calling `stop()` on a non-running task should return `Ok(())`

## Preconditions

- [ ] **PRE-01**: Task must be in an active state (CREATED, PENDING, SCHEDULED, RUNNING)
  - Rationale: Cannot stop a task that has already completed or failed
  - Violation: Return `Error::TaskNotRunning`
  
- [ ] **PRE-02**: Runtime adapter must have tracked the process/container ID during `run()`
  - Rationale: Cannot stop what we don't know about
  - Violation: Return `Error::ProcessNotFound`
  
- [ ] **PRE-03**: Task ID must be non-empty and valid
  - Rationale: Empty IDs cannot be used for process/container lookup
  - Violation: Return `Error::InvalidTaskId`

## Postconditions

- [ ] **POST-01**: Process/Container must be terminated (exit code retrieved)
  - Rationale: Primary goal of `stop()` is to terminate the running task
  - Verification: Process PID does not exist, container state is `exited`
  
- [ ] **POST-02**: Internal state must be cleaned up
  - Rationale: Prevent memory leaks and stale references
  - Verification: Task ID removed from `active_tasks` map
  
- [ ] **POST-03**: Temporary resources must be cleaned up
  - Rationale: Prevent disk space leaks (temp files, volumes, mounts)
  - Verification: Temp directories removed, volumes unmounted
  
- [ ] **POST-04**: Task state must be updated to STOPPED
  - Rationale: Consistency with task lifecycle management
  - Verification: Task state transitions from RUNNING → STOPPED

## Invariants

- [ ] **INV-01**: No zombie processes/containers left after `stop()` completes
  - Rationale: Resource leak prevention
  - Violation: Process still exists in process table, container still running
  
- [ ] **INV-02**: Process termination is atomic (all child processes terminated)
  - Rationale: Prevent orphaned processes
  - Violation: Child processes still running after parent stopped
  
- [ ] **INV-03**: Timeout enforcement is strict (no indefinite hangs)
  - Rationale: Prevent hangs that block shutdown sequence
  - Violation: `stop()` takes longer than configured timeout
  
- [ ] **INV-04**: Signal propagation order is preserved (SIGTERM → SIGKILL)
  - Rationale: Graceful shutdown best practices
  - Violation: SIGKILL sent before timeout period elapsed

## Error Taxonomy

All errors must implement `std::error::Error`, `std::fmt::Display`, and `thiserror::Error`.

```rust
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ShutdownError {
    #[error("task is not in a running state: {0}")]
    TaskNotRunning(String),
    
    #[error("process not found: task_id={0}")]
    ProcessNotFound(String),
    
    #[error("invalid task ID: {0}")]
    InvalidTaskId(String),
    
    #[error("timeout waiting for graceful shutdown: {0}s elapsed")]
    ShutdownTimeout(u64),
    
    #[error("failed to send termination signal: {0}")]
    SignalError(String),
    
    #[error("failed to terminate process: {0}")]
    TerminationFailed(String),
    
    #[error("cleanup failed: {0}")]
    CleanupFailed(String),
    
    #[error("resource not available: {0}")]
    ResourceUnavailable(String),
    
    #[error("exit code: {0}")]
    ExitCode(i32),
    
    #[error("runtime error: {0}")]
    RuntimeError(String),
}

// Railway-oriented wrapper for Result types
pub type ShutdownResult<T> = Result<T, ShutdownError>;
```

## Contract Signatures

### Runtime Trait Extension

```rust
pub trait Runtime: Send + Sync {
    /// Execute a task in the runtime environment
    fn run(&self, task: &Task) -> BoxedFuture<ProcessInfo>;
    
    /// Gracefully stop a running task
    /// 
    /// # Preconditions
    /// - Task must be in RUNNING state
    /// - Runtime must have tracked the process/container ID
    /// - Task ID must be non-empty
    /// 
    /// # Postconditions
    /// - Process/container terminated
    /// - Internal state cleaned up
    /// - Temporary resources removed
    /// - Exit code recorded
    /// 
    /// # Errors
    /// - TaskNotRunning: Task is not in a running state
    /// - ProcessNotFound: Runtime doesn't have record of this task
    /// - InvalidTaskId: Task ID is empty or malformed
    /// - ShutdownTimeout: Graceful shutdown exceeded timeout
    /// - SignalError: Failed to send termination signal
    /// - TerminationFailed: Process could not be terminated
    /// - CleanupFailed: Post-termination cleanup failed
    /// - ExitCode: Process exited with non-zero code (informational)
    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>>;
    
    /// Check runtime health
    fn health_check(&self) -> BoxedFuture<Result<()>>;
}
```

### ShellRuntimeAdapter Specifics

```rust
pub struct ShellRuntimeAdapter {
    cmd: Vec<String>,
    uid: String,
    gid: String,
    active_processes: Arc<DashMap<TaskId, ProcessHandle>>,
}

impl Runtime for ShellRuntimeAdapter {
    /// Returns: ProcessInfo with PID, start time, and ProcessHandle
    fn run(&self, task: &Task) -> BoxedFuture<ProcessInfo>;
    
    /// Terminates shell process and cleanup temp files
    /// - Sends SIGTERM to process group
    /// - Waits for graceful exit (default 30s)
    /// - Falls back to SIGKILL if timeout exceeded
    /// - Removes temporary script file
    /// - Returns ExitCode
    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>>;
}
```

### PodmanRuntimeAdapter Specifics

```rust
pub struct PodmanRuntimeAdapter {
    privileged: bool,
    host_network: bool,
    active_containers: Arc<DashMap<TaskId, ContainerId>>,
}

impl Runtime for PodmanRuntimeAdapter {
    /// Returns: ProcessInfo with container ID and start time
    fn run(&self, task: &Task) -> BoxedFuture<ProcessInfo>;
    
    /// Stops podman container and cleanup resources
    /// - Sends SIGTERM via container stop
    /// - Waits for graceful exit (default 30s)
    /// - Falls back to SIGKILL if timeout exceeded
    /// - Removes container and associated volumes
    /// - Returns ExitCode
    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>>;
}
```

### MockRuntime Specifics

```rust
pub struct MockRuntime;

impl Runtime for MockRuntime {
    /// Mock run (no-op)
    fn run(&self, task: &Task) -> BoxedFuture<ProcessInfo>;
    
    /// Mock stop (idempotent, no-op)
    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>>;
}
```

## Non-goals

- [ ] **NG-01**: Do not implement process monitoring/heartbeats
  - Rationale: Out of scope for shutdown implementation
  - Future work: Separate bead for process health monitoring
  
- [ ] **NG-02**: Do not implement rolling restarts
  - Rationale: Different feature, separate concern
  - Future work: Separate bead for restart logic
  
- [ ] **NG-03**: Do not implement process group management for nested processes
  - Rationale: Shell adapter runs single command, not process trees
  - Future work: If needed, use `process_group_id` and terminate group
  
- [ ] **NG-04**: Do not implement signal customization (SIGUSR1, etc.)
  - Rationale: Standard SIGTERM/SIGKILL sufficient for now
  - Future work: Configurable signal types if needed

## Type-First Enforcement

All runtime adapters must:
1. Use `Arc<DashMap<TaskId, ProcessHandle>>` for tracking active processes
2. Return `Result<ExitCode, ShutdownError>` from `stop()`
3. Use `tokio::time::timeout()` for timeout enforcement
4. Implement signal propagation with `tokio::process::Child::kill()`
5. Clean up resources in `drop()` or explicit cleanup methods

## Configuration

Environment variables for shutdown behavior:

```rust
pub const ENV_TASK_STOP_GRACEFUL_TIMEOUT: &str = "TASK_STOP_GRACEFUL_TIMEOUT"; // seconds, default: 30
pub const ENV_TASK_STOP_FORCE_TIMEOUT: &str = "TASK_STOP_FORCE_TIMEOUT";       // seconds, default: 5
pub const ENV_TASK_STOP_ENABLE_CLEANUP: &str = "TASK_STOP_ENABLE_CLEANUP";     // bool, default: true
```
