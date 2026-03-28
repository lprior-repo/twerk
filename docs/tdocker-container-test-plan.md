# Test Plan: Docker Tcontainer Functionality

## Summary
- **Behaviors identified**: 27
- **Trophy allocation**: 8 unit / 18 integration / 1 e2e
- **Proptest invariants**: 4
- **Fuzz targets**: 3
- **Kani harnesses**: 2

---

## 1. Behavior Inventory

### createTaskContainer
1. `[tcontainer] rejects container creation when task ID is empty`
2. `[tcontainer] pulls image before container creation`
3. `[tcontainer] builds environment variables including TORK_OUTPUT and TORK_PROGRESS`
4. `[tcontainer] processes Volume mount type with target validation`
5. `[tcontainer] processes Bind mount type with source and target validation`
6. `[tcontainer] processes Tmpfs mount type`
7. `[tcontainer] rejects unknown mount types`
8. `[tcontainer] mounts tork volume at /tork`
9. `[tcontainer] parses CPU limits and returns error on invalid value`
10. `[tcontainer] parses memory limits and returns error on invalid value`
11. `[tcontainer] configures GPU resources when GPUs string is specified`
12. `[tcontainer] uses /tork/entrypoint as default command when cmd is empty`
13. `[tcontainer] uses "sh -c" as entrypoint when task.Run is set`
14. `[tcontainer] sets working dir to task.Workdir or defaultWorkdir when files present`
15. `[tcontainer] exposes probe port and maps to random host port when probe configured`
16. `[tcontainer] creates network aliases for each specified network`
17. `[tcontainer] creates container with 30-second timeout context`
18. `[tcontainer] initializes torkdir with stdout/progress/entrypoint files`
19. `[tcontainer] initializes workdir by copying task files to workdir`

### Start
20. `[tcontainer] starts container successfully`
21. `[tcontainer] probes container readiness when probe is configured`
22. `[tcontainer] returns error when probe times out`

### Remove
23. `[tcontainer] force removes container with volumes`
24. `[tcontainer] unmounts torkdir volume after container removal`

### Wait
25. `[tcontainer] reports progress every 10 seconds via broker`
26. `[tcontainer] streams container logs to logger during execution`
27. `[tcontainer] returns stdout content and nil error when exit code is 0`
28. `[tcontainer] returns error with last 10 log lines when exit code is non-zero`
29. `[tcontainer] handles context cancellation during wait`

### probeContainer
30. `[tcontainer] skips probe entirely when no probe is configured`
31. `[tcontainer] defaults probe path to "/" when empty`
32. `[tcontainer] defaults probe timeout to "1m" when empty`
33. `[tcontainer] retrieves assigned host port from container inspect`
34. `[tcontainer] returns error when no port is found for container`
35. `[tcontainer] sends HTTP GET requests every second until status 200`
36. `[tcontainer] logs retry messages on probe failure`
37. `[tcontainer] returns error when probe context times out`

### reportProgress
38. `[tcontainer] reads progress from container every 10 seconds`
39. `[tcontainer] ignores ErrNotFound when progress file does not exist`
40. `[tcontainer] publishes updated progress to broker when value changes`
41. `[tcontainer] exits gracefully when context is cancelled`

### readOutput
42. `[tcontainer] copies /tork/stdout from container`
43. `[tcontainer] parses tar archive and returns content as string`
44. `[tcontainer] handles io.EOF when archive is complete`
45. `[tcontainer] returns error on tar parsing failure`

### readProgress
46. `[tcontainer] copies /tork/progress from container`
47. `[tcontainer] returns 0.0 when progress file is empty`
48. `[tcontainer] parses and returns progress as float64`

### initTorkdir
49. `[tcontainer] creates temp archive for torkdir initialization`
50. `[tcontainer] writes empty stdout file with 0222 permissions`
51. `[tcontainer] writes empty progress file with 0222 permissions`
52. `[tcontainer] writes entrypoint file with task.Run content when task.Run is set`
53. `[tcontainer] copies archive to /tork in container`

### initWorkDir
54. `[tcontainer] returns early when no task files are present`
55. `[tcontainer] creates temp archive for workdir initialization`
56. `[tcontainer] writes all task files with 0444 permissions`
57. `[tcontainer] copies archive to task.Workdir in container`

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 8 | Pure parsing functions: `parseCPUs`, `parseMemory`, `readProgress` float parsing, `initTorkdir` file writing logic, mount type validation, env var building |
| **Integration** | 18 | All container operations requiring real Docker: create, start, probe, wait, remove, unmount, copy from/to container, progress reporting, log streaming |
| **E2E** | 1 | Full task lifecycle: create container → start → wait → remove with real Docker daemon |
| **Static Analysis** | - | clippy, cargo-deny, vet — standard Go tooling |

**Rationale**: The core logic is integration-heavy because every operation requires the Docker client and a real container. Unit tests are reserved for pure functions with no I/O dependencies. The single E2E test validates the complete workflow.

---

## 3. BDD Scenarios

### Behavior 1: tcontainer rejects container creation when task ID is empty
```gherkin
Given a DockerRuntime and a task with empty ID
When createTaskContainer is called
Then an error "task id is required" is returned
```

### Behavior 2: tcontainer pulls image before container creation
```gherkin
Given a DockerRuntime and a valid task with an image
When createTaskContainer is called
Then the image is pulled before container creation
And the container is created with the pulled image
```

### Behavior 3: tcontainer builds environment variables including TORK_OUTPUT and TORK_PROGRESS
```gherkin
Given a task with custom env vars {"KEY": "value"}
When createTaskContainer builds the env slice
Then the env includes "KEY=value"
And the env includes "TORK_OUTPUT=/tork/stdout"
And the env includes "TORK_PROGRESS=/tork/progress"
```

### Behavior 4: tcontainer processes Volume mount type with target validation
```gherkin
Given a task with a Volume mount without target
When createTaskContainer processes mounts
Then an error "volume target is required" is returned
```

### Behavior 5: tcontainer processes Bind mount type with source and target validation
```gherkin
Given a task with a Bind mount missing source or target
When createTaskContainer processes mounts
Then an error "bind source is required" or "bind target is required" is returned
```

### Behavior 6: tcontainer processes Tmpfs mount type
```gherkin
Given a task with a Tmpfs mount
When createTaskContainer processes mounts
Then the mount is added with TypeTmpfs
```

### Behavior 7: tcontainer rejects unknown mount types
```gherkin
Given a task with an unknown mount type
When createTaskContainer processes mounts
Then an error "unknown mount type: <type>" is returned
```

### Behavior 8: tcontainer mounts tork volume at /tork
```gherkin
Given a valid task
When createTaskContainer creates the container
Then a volume mount with Target "/tork" is added to the mounts
And the volume is mounted via mounter.Mount
```

### Behavior 9: tcontainer parses CPU limits and returns error on invalid value
```gherkin
Given a task with CPU limit "invalid"
When parseCPUs is called
Then an error "invalid CPUs value" is returned
```

### Behavior 10: tcontainer parses memory limits and returns error on invalid value
```gherkin
Given a task with memory limit "not-a-number"
When parseMemory is called
Then an error "invalid memory value" is returned
```

### Behavior 11: tcontainer configures GPU resources when GPUs string is specified
```gherkin
Given a task with GPUs "all"
When createTaskContainer sets up resources
Then resources.DeviceRequests contains GPU device requests
```

### Behavior 12: tcontainer uses /tork/entrypoint as default command when cmd is empty
```gherkin
Given a task with empty cmd slice
When createTaskContainer builds container config
Then cmd is set to ["/tork/entrypoint"]
```

### Behavior 13: tcontainer uses "sh -c" as entrypoint when task.Run is set
```gherkin
Given a task with Run "echo hello" and empty entrypoint
When createTaskContainer builds container config
Then entrypoint is set to ["sh", "-c"]
```

### Behavior 14: tcontainer sets working dir to task.Workdir or defaultWorkdir when files present
```gherkin
Given a task with Files {"test.txt": "content"} but no Workdir
When createTaskContainer builds container config
Then WorkingDir is set to defaultWorkdir
```

### Behavior 15: tcontainer exposes probe port and maps to random host port when probe configured
```gherkin
Given a task with Probe {Port: 8080}
When createTaskContainer builds container config
Then ExposedPorts contains "8080/tcp"
And PortBindings maps "8080/tcp" to a random host port on 127.0.0.1
```

### Behavior 16: tcontainer creates network aliases for each specified network
```gherkin
Given a task with Networks ["network1", "network2"]
When createTaskContainer builds network config
Then each endpoint has Aliases containing a slugified task name
```

### Behavior 17: tcontainer creates container with 30-second timeout context
```gherkin
Given a valid task
When createTaskContainer creates the container
Then a context with 30-second timeout is used for ContainerCreate
```

### Behavior 18: tcontainer initializes torkdir with stdout/progress/entrypoint files
```gherkin
Given a valid task with Run "echo hello"
When createTaskContainer completes initialization
Then /tork contains stdout, progress, and entrypoint files
And entrypoint contains "echo hello"
```

### Behavior 19: tcontainer initializes workdir by copying task files to workdir
```gherkin
Given a task with Files {"test.txt": "hello world"} and Workdir "/work"
When createTaskContainer completes workdir init
Then /work/test.txt exists in the container with content "hello world"
```

### Behavior 20: tcontainer starts container successfully
```gherkin
Given a created tcontainer
When Start is called
Then ContainerStart is invoked with the container ID
And no error is returned on success
```

### Behavior 21: tcontainer probes container readiness when probe is configured
```gherkin
Given a task with Probe {Port: 8080, Path: "/health"}
And a container that responds with 200 OK on /health
When Start is called
Then probeContainer succeeds within the timeout
```

### Behavior 22: tcontainer returns error when probe times out
```gherkin
Given a task with Probe {Port: 8080, Timeout: "1s"}
And a container that never responds on the probe port
When Start is called
Then an error "probe timed out after 1s" is returned
```

### Behavior 23: tcontainer force removes container with volumes
```gherkin
Given a tcontainer with an ID
When Remove is called
Then ContainerRemove is called with Force: true and RemoveVolumes: true
```

### Behavior 24: tcontainer unmounts torkdir volume after container removal
```gherkin
Given a tcontainer with a mounted torkdir
When Remove is called
Then mounter.Unmount is called with the torkdir
```

### Behavior 25: tcontainer reports progress every 10 seconds via broker
```gherkin
Given a running tcontainer with progress file containing "0.5"
When Wait is called
Then broker.PublishTaskProgress is called with Progress: 0.5
And this occurs every 10 seconds until container exits
```

### Behavior 26: tcontainer streams container logs to logger during execution
```gherkin
Given a running tcontainer
When Wait is called
Then io.Copy is called to stream logs to tc.logger
```

### Behavior 27: tcontainer returns stdout content and nil error when exit code is 0
```gherkin
Given a container that exits with code 0
And /tork/stdout contains "hello world"
When Wait is called
Then the returned stdout is "hello world"
And the returned error is nil
```

### Behavior 28: tcontainer returns error with last 10 log lines when exit code is non-zero
```gherkin
Given a container that exits with code 1
And the last 10 lines of logs contain "error occurred"
When Wait is called
Then an error containing "exit code 1" is returned
And the error contains the last 10 log lines
```

### Behavior 29: tcontainer handles context cancellation during wait
```gherkin
Given a running tcontainer
And a context that gets cancelled
When Wait is called
Then it returns without hanging
```

### Behavior 30: tcontainer skips probe entirely when no probe is configured
```gherkin
Given a task with nil Probe
When probeContainer is called
Then nil is returned immediately without any HTTP requests
```

### Behavior 31: tcontainer defaults probe path to "/" when empty
```gherkin
Given a task with Probe {Port: 8080, Path: ""}
When probeContainer constructs the probe URL
Then the URL is "http://localhost:<port>/"
```

### Behavior 32: tcontainer defaults probe timeout to "1m" when empty
```gherkin
Given a task with Probe {Port: 8080, Timeout: ""}
When probeContainer parses the timeout
Then the timeout is 1 minute
```

### Behavior 33: tcontainer retrieves assigned host port from container inspect
```gherkin
Given a container with exposed port 8080 mapped to host port 32768
When probeContainer inspects the container
Then port 32768 is used for the probe URL
```

### Behavior 34: tcontainer returns error when no port is found for container
```gherkin
Given a container with no port bindings
When probeContainer looks up the probe port
Then an error "no port found for container" is returned
```

### Behavior 35: tcontainer sends HTTP GET requests every second until status 200
```gherkin
Given a container that starts responding with 200 OK after 3 seconds
When probeContainer is called
Then HTTP GET requests are sent every 1 second
And probe returns nil when 200 OK is received
```

### Behavior 36: tcontainer logs retry messages on probe failure
```gherkin
Given a container that returns 500 on probe requests
When probeContainer retries
Then "Probe for container <id> returned status code 500. Retrying..." is logged
```

### Behavior 37: tcontainer returns error when probe context times out
```gherkin
Given a probe timeout of "5s"
And a container that never responds with 200
When probeContainer runs
Then an error "probe timed out after 5s" is returned
```

### Behavior 38: tcontainer reads progress from container every 10 seconds
```gherkin
Given a container with /tork/progress containing "0.75"
When reportProgress reads the progress
Then tc.task.Progress is updated to 0.75
```

### Behavior 39: tcontainer ignores ErrNotFound when progress file does not exist
```gherkin
Given a container where /tork/progress does not exist
When reportProgress calls readProgress
Then no error is logged
And the loop continues
```

### Behavior 40: tcontainer publishes updated progress to broker when value changes
```gherkin
Given tc.task.Progress is 0.5
And readProgress returns 0.7
When reportProgress processes the new value
Then broker.PublishTaskProgress is called with Progress: 0.7
```

### Behavior 41: tcontainer exits gracefully when context is cancelled
```gherkin
Given reportProgress is running
When the context is cancelled
Then the loop exits without error
```

### Behavior 42: tcontainer copies /tork/stdout from container
```gherkin
Given a container with /tork/stdout containing "output data"
When readOutput is called
Then CopyFromContainer is called with "/tork/stdout"
```

### Behavior 43: tcontainer parses tar archive and returns content as string
```gherkin
Given a tar archive containing a file "stdout" with content "test output"
When readOutput parses the archive
Then "test output" is returned as a string
```

### Behavior 44: tcontainer handles io.EOF when archive is complete
```gherkin
Given a valid tar archive with no more entries
When readOutput encounters io.EOF
Then the loop breaks and content is returned
```

### Behavior 45: tcontainer returns error on tar parsing failure
```gherkin
Given a corrupted tar archive
When readOutput tries to read
Then an error is returned
```

### Behavior 46: tcontainer copies /tork/progress from container
```gherkin
Given a container with /tork/progress
When readProgress is called
Then CopyFromContainer is called with "/tork/progress"
```

### Behavior 47: tcontainer returns 0.0 when progress file is empty
```gherkin
Given a container with /tork/progress containing only whitespace
When readProgress parses the value
Then 0.0 is returned
```

### Behavior 48: tcontainer parses and returns progress as float64
```gherkin
Given a container with /tork/progress containing "0.85"
When readProgress parses the value
Then 0.85 is returned
```

### Behavior 49: tcontainer creates temp archive for torkdir initialization
```gherkin
Given a valid task
When initTorkdir is called
Then NewTempArchive creates a temporary archive file
```

### Behavior 50: tcontainer writes empty stdout file with 0222 permissions
```gherkin
Given a temp archive
When initTorkdir writes the stdout file
Then the file has permissions 0222 (w--w--w-)
```

### Behavior 51: tcontainer writes empty progress file with 0222 permissions
```gherkin
Given a temp archive
When initTorkdir writes the progress file
Then the file has permissions 0222 (w--w--w-)
```

### Behavior 52: tcontainer writes entrypoint file with task.Run content when task.Run is set
```gherkin
Given a task with Run "echo hello"
When initTorkdir writes the entrypoint file
Then the file has permissions 0555 (r-xr-xr-x)
And the content is "echo hello"
```

### Behavior 53: tcontainer copies archive to /tork in container
```gherkin
Given a temp archive with files
When initTorkdir completes
Then CopyToContainer is called with target "/tork"
```

### Behavior 54: tcontainer returns early when no task files are present
```gherkin
Given a task with empty Files map
When initWorkDir is called
Then nil is returned immediately
And no archive is created
```

### Behavior 55: tcontainer creates temp archive for workdir initialization
```gherkin
Given a task with Files {"test.txt": "content"}
When initWorkDir is called
Then NewTempArchive creates a temporary archive file
```

### Behavior 56: tcontainer writes all task files with 0444 permissions
```gherkin
Given a task with Files {"a.txt": "content1", "b.txt": "content2"}
When initWorkDir writes the files
Then each file has permissions 0444 (r--r--r--)
```

### Behavior 57: tcontainer copies archive to task.Workdir in container
```gherkin
Given a task with Workdir "/workspace"
When initWorkDir completes
Then CopyToContainer is called with target "/workspace"
```

---

## 4. Proptest Invariants

### Proptest: parseCPUs
```
Invariant: parseCPUs returns a positiveNanoCPUs value for any valid CPU string
Strategy: any valid CPU string (e.g., "1", "0.5", "100m", "2核")
Anti-invariant: "invalid", "abc", "" (should return error)
```

### Proptest: parseMemory  
```
Invariant: parseMemory returns a positive memory value for any valid memory string
Strategy: any valid memory string (e.g., "1GB", "512MB", "1G", "1024M")
Anti-invariant: "invalid", "abc", "" (should return error)
```

### Proptest: readProgress float parsing
```
Invariant: readProgress returns a value between 0.0 and 1.0 for any valid progress string
Strategy: any string that parses to a float (e.g., "0", "0.5", "1.0", "0.75")
Anti-invariant: "not-a-number", "" returns 0.0 without error
```

### Proptest: initTorkdir file permissions
```
Invariant: stdout and progress files always have 0222 permissions, entrypoint has 0555
Strategy: verify permissions on WriteFile calls match expected values
Anti-invariant: permissions that would make files unreadable or unwritable
```

---

## 5. Fuzz Targets

### Fuzz Target: parseCPUs input parsing
```
Input type: string (arbitrary CPU limit string)
Risk: Panic on invalid format, incorrect parsing, nil dereference
Corpus seeds: "1", "0.5", "100m", "2", "0.25", "invalid", "", "9999999999999999"
```

### Fuzz Target: parseMemory input parsing
```
Input type: string (arbitrary memory limit string)
Risk: Panic on invalid format, incorrect parsing, integer overflow
Corpus seeds: "1GB", "512MB", "1G", "1024M", "invalid", "", "999999999999999999999999"
```

### Fuzz Target: readProgress tar archive parsing
```
Input type: arbitrary tar archive bytes from CopyFromContainer
Risk: Panic on malformed tar, path traversal via symlinks, excessive memory allocation
Corpus seeds: Valid tar with stdout file, empty tar, single file, multiple files
```

---

## 6. Kani Harnesses

### Kani Harness: parseCPUs arithmetic
```
Property: parseCPUs must not overflow for any valid CPU string
Bound: CPU strings up to 64 characters
Rationale: Arithmetic on large CPU values could overflow if not handled properly
```

### Kani Harness: readProgress bounds
```
Property: returned float64 must be within [0.0, 1.0] for all valid progress strings
Bound: Progress strings up to 32 characters  
Rationale: Progress values outside 0.0-1.0 range indicate parsing bug or invalid state
```

---

## 7. Mutation Checkpoints

### Critical mutations to survive:

| Function | Mutation | Required Test |
|---------|----------|---------------|
| `createTaskContainer` | Empty task ID check removed | Must get "task id is required" error |
| `createTaskContainer` | Mount type validation removed | Must reject unknown mount types |
| `createTaskContainer` | CPU parsing error check removed | Must return error on invalid CPU |
| `createTaskContainer` | Memory parsing error check removed | Must return error on invalid memory |
| `Start` | probeContainer call removed | Must still start container without probe |
| `Remove` | Unmount call removed | Must still remove container |
| `Wait` | Exit code 0 vs non-0 branch inverted | Must return correct error type |
| `probeContainer` | Timeout check removed | Must handle probe timeout gracefully |
| `probeContainer` | Status 200 check inverted | Must only succeed on exact 200 |
| `readProgress` | Empty string check removed | Must return 0.0 for empty |
| `initTorkdir` | Permissions 0222 changed to 0000 | Must still create writable files |
| `initWorkDir` | Early return when no files removed | Must handle empty files gracefully |

**Threshold**: 90% mutation kill rate minimum.

---

## 8. Combinatorial Coverage Matrix

### parseCPUs coverage

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| happy path: whole number | "1" | Ok(1_000_000_000) | unit |
| happy path: decimal | "0.5" | Ok(500_000_000) | unit |
| happy path: millicpus | "100m" | Ok(100_000_000) | unit |
| error: invalid string | "invalid" | Err | unit |
| error: empty string | "" | Err | unit |
| boundary: very large | "9999999999999999" | Err or Ok | unit |

### parseMemory coverage

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| happy path: GB | "1GB" | Ok(1_073_741_824) | unit |
| happy path: MB | "512MB" | Ok(536_870_912) | unit |
| happy path: G suffix | "1G" | Ok(1_073_741_824) | unit |
| happy path: M suffix | "1024M" | Ok(1_073_741_824) | unit |
| error: invalid string | "invalid" | Err | unit |
| error: empty string | "" | Err | unit |

### readProgress coverage

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| happy path: valid float | "0.85" | Ok(0.85) | unit |
| happy path: zero | "0" | Ok(0.0) | unit |
| happy path: one | "1" | Ok(1.0) | unit |
| empty string | "" | Ok(0.0) | unit |
| whitespace only | "   " | Ok(0.0) | unit |
| error: not a number | "abc" | Err | unit |

### Mount type validation coverage

| Scenario | Mount Type | Expected | Layer |
|----------|------------|----------|-------|
| Volume with target | Volume, Target="/data" | Ok | unit |
| Volume without target | Volume, Target="" | Err("volume target is required") | unit |
| Bind with source and target | Bind, Source="/host", Target="/container" | Ok | unit |
| Bind without source | Bind, Source="", Target="/container" | Err("bind source is required") | unit |
| Bind without target | Bind, Source="/host", Target="" | Err("bind target is required") | unit |
| Tmpfs | Tmpfs | Ok | unit |
| Unknown type | Unknown | Err("unknown mount type") | unit |

### Container creation integration scenarios

| Scenario | Task Config | Expected | Layer |
|----------|-------------|----------|-------|
| Full config | All fields populated | Container created | integration |
| Minimal config | Only ID and Image | Container created | integration |
| With GPU | GPUs="all" | Container with GPU resources | integration |
| With probe | Probe configured | Port exposed and mapped | integration |
| With networks | Multiple networks | Container attached to networks | integration |
| With mounts | Volume, Bind, Tmpfs | All mounts configured | integration |

---

## Open Questions

1. **Default workdir path**: What is the actual value of `defaultWorkdir`? Need to confirm for testing.
2. **GPU test environment**: GPU tests require nvidia-docker or compatible runtime. Should these be skippable in CI?
3. **Probe port collision**: When running parallel tests, how to avoid port collision on probe ports?
4. **Mock Docker client**: Should integration tests use a mock Docker client or real Docker (testcontainers)?
5. **Tar archive parsing**: Should symlinks in tar archives be rejected to prevent path traversal?
6. **Progress file polling interval**: 10-second interval makes tests slow. Should this be configurable for testing?
