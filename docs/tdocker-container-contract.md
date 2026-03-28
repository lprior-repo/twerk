# Docker TContainer Contract Specification

## Context

- **Feature**: Docker-based task container lifecycle management
- **Domain Terms**:
  - `tcontainer` - A Docker container wrapper that manages task execution
  - `torkdir` - The `/tork` directory mounted into containers for stdout, progress, and entrypoint
  - `workdir` - The working directory for task files
  - `probe` - HTTP health check for container readiness
  - `Mount` - Volume/bind/tmpfs mounts attached to the container
- **Assumptions**:
  - Docker client is already authenticated and connected
  - The image exists locally or can be pulled from the registry
  - The broker is available for progress publishing
  - The mounter can create and mount volumes
- **Open Questions**:
  - What is the maximum retry count for image pulls?
  - Is there a maximum timeout for container creation?
  - Should GPU allocation failures be retried?

---

## Preconditions

1. **For `createTaskContainer`**:
   - `task.ID` must be non-empty
   - `task.Image` must be non-empty and a valid Docker image reference
   - If `task.Mounts` contains a bind mount, both `Source` and `Target` must be non-empty
   - If `task.Mounts` contains a volume mount, `Target` must be non-empty
   - `task.Limits.CPU` and `task.Limits.Memory` must be valid (or empty)
   - `task.GPUs` must be valid GPU option string (or empty)
   - The runtime's mounter must be operational
   - The runtime's broker must be operational

2. **For `Start`**:
   - The container must have been successfully created via `createTaskContainer`
   - The container must be in a non-running state (created/stopped)

3. **For `Remove`**:
   - The container must exist (created, running, or stopped)
   - The torkdir mount must exist

4. **For `Wait`**:
   - The container must be in a running state
   - The `/tork/stdout` file must exist in the container

5. **For `probeContainer`**:
   - `task.Probe` must be non-nil
   - `task.Probe.Port` must be a valid port number

---

## Postconditions

1. **After `createTaskContainer` (success)**:
   - A Docker container exists with the given image and configuration
   - The torkdir volume is mounted at `/tork` inside the container
   - `/tork/stdout` and `/tork/progress` files exist inside the container
   - If `task.Run` is set, `/tork/entrypoint` contains the run script
   - If `task.Files` is non-empty and `task.Workdir` is set, files exist in the workdir
   - Container is in `created` state (not running)
   - The returned `tcontainer` has a valid `id` field

2. **After `Start` (success)**:
   - The container is in a `running` state
   - If a probe is configured, the probe endpoint returns HTTP 200
   - Container logs are being streamed to the logger

3. **After `Remove` (success)**:
   - The container is removed from Docker
   - The torkdir volume is unmounted
   - All resources are cleaned up

4. **After `Wait` (success)**:
   - The container is in an `exited` state
   - The exit code is 0
   - The stdout output from `/tork/stdout` is returned
   - Progress has been published to the broker

5. **After `Wait` (failure - non-zero exit)**:
   - The container is in an `exited` state
   - An error with exit code information is returned
   - The last 10 lines of logs are included in the error

---

## Invariants

1. **Container Identity**: Once a `tcontainer` is created, its `id` field never changes
2. **Task Reference**: The `task` field always points to the original task definition
3. **Logger**: The `logger` field is always non-nil (even if a no-op writer)
4. **Torkdir Lifecycle**: If a container is successfully created, its torkdir is mountable; after `Remove` it is unmounted
5. **State Consistency**: The container's Docker state corresponds to the internal state management
6. **Resource Cleanup**: `Remove` is idempotent - calling it on an already-removed container returns an error but does not panic
7. **Progress Reporting**: Progress reporting runs in a goroutine and does not block the main operation

---

## Error Taxonomy

### Validation Errors (InvalidInput)

| Error | Condition |
|-------|-----------|
| `ErrTaskIDRequired` | `task.ID` is empty |
| `ErrImageRequired` | `task.Image` is empty |
| `ErrBindSourceRequired` | Bind mount has empty `Source` |
| `ErrBindTargetRequired` | Bind mount has empty `Target` |
| `ErrVolumeTargetRequired` | Volume mount has empty `Target` |
| `ErrUnknownMountType` | Mount type is not Volume/Bind/Tmpfs |
| `ErrInvalidCPUValue` | CPU limit string is malformed |
| `ErrInvalidMemoryValue` | Memory limit string is malformed |
| `ErrInvalidGPUOptions` | GPU options string is malformed |
| `ErrInvalidProbeTimeout` | Probe timeout string is malformed |
| `ErrInvalidProbePort` | Probe port is not a valid number |
| `ErrWorkdirRequired` | Files exist but `Workdir` is empty |

### Resource Errors (NotFound / Unavailable)

| Error | Condition |
|-------|-----------|
| `ErrImagePullFailed` | Failed to pull the Docker image |
| `ErrContainerCreateFailed` | Docker container creation failed |
| `ErrContainerStartFailed` | Docker container start failed |
| `ErrContainerRemoveFailed` | Docker container removal failed |
| `ErrContainerInspectFailed` | Cannot inspect container state |
| `ErrMountFailed` | Failed to mount the torkdir volume |
| `ErrUnmountFailed` | Failed to unmount the torkdir volume |
| `ErrVolumeNotFound` | Port binding not found for probed container |
| `ErrOutputCopyFailed` | Failed to copy output from container |
| `ErrProgressCopyFailed` | Failed to copy progress from container |
| `ErrLogStreamFailed` | Failed to stream container logs |
| `ErrArchiveCreateFailed` | Failed to create temporary archive |

### Wait/Eecution Errors (ExecutionFailure)

| Error | Condition |
|-------|-----------|
| `ErrContainerWaitFailed` | Error waiting for container to finish |
| `ErrNonZeroExit` | Container exited with non-zero status code |
| `ErrProbeTimeout` | Probe timed out before succeeding |
| `ErrProbeHTTPFailed` | Probe HTTP request failed |
| `ErrOutputReadFailed` | Failed to read stdout from container |
| `ErrProgressParseFailed` | Failed to parse progress value |

### Context Errors

| Error | Condition |
|-------|-----------|
| `ErrContextCanceled` | Operation was canceled via context |
| `ErrContextTimeout` | Operation exceeded timeout (e.g., 30s container create) |

---

## Contract Signatures

All fallible operations return `Result<T, Error>` equivalents in idiomatic Go error handling.

| Operation | Signature | Error Types |
|-----------|-----------|-------------|
| `createTaskContainer` | `func(ctx, rt, task, logger) (*tcontainer, error)` | `ErrTaskIDRequired`, `ErrImageRequired`, `ErrImagePullFailed`, `ErrBindSourceRequired`, `ErrBindTargetRequired`, `ErrVolumeTargetRequired`, `ErrUnknownMountType`, `ErrInvalidCPUValue`, `ErrInvalidMemoryValue`, `ErrInvalidGPUOptions`, `ErrMountFailed`, `ErrContainerCreateFailed`, `ErrArchiveCreateFailed`, `ErrContextTimeout` |
| `Start` | `func(ctx) error` | `ErrContainerStartFailed`, `ErrContainerInspectFailed`, `ErrProbeTimeout`, `ErrProbeHTTPFailed`, `ErrLogStreamFailed`, `ErrContextCanceled` |
| `Remove` | `func(ctx) error` | `ErrContainerRemoveFailed`, `ErrUnmountFailed`, `ErrContextCanceled` |
| `Wait` | `func(ctx) (string, error)` | `ErrLogStreamFailed`, `ErrContainerWaitFailed`, `ErrNonZeroExit`, `ErrOutputReadFailed`, `ErrContextCanceled` |
| `probeContainer` | `func(ctx) error` | `ErrContainerInspectFailed`, `ErrVolumeNotFound`, `ErrInvalidProbeTimeout`, `ErrProbeTimeout`, `ErrProbeHTTPFailed`, `ErrLogStreamFailed`, `ErrContextCanceled` |
| `readOutput` | `func(ctx) (string, error)` | `ErrOutputCopyFailed`, `ErrContextCanceled` |
| `readProgress` | `func(ctx) (float64, error)` | `ErrProgressCopyFailed`, `ErrProgressParseFailed`, `ErrContextCanceled` |
| `initTorkdir` | `func(ctx) error` | `ErrArchiveCreateFailed`, `ErrContextCanceled` |
| `initWorkDir` | `func(ctx) error` | `ErrWorkdirRequired`, `ErrArchiveCreateFailed`, `ErrContextCanceled` |

---

## Non-goals

- Container networking beyond basic network attachment and port publishing
- Multi-container orchestration or composition
- Direct container logins or registry authentication management
- Container checkpointing or pausing
- Resource limit enforcement beyond Docker's native mechanisms
- Automatic retry logic for transient failures (left to caller)
