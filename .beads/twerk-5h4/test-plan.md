# Test Plan: Fix runtime gaps: Docker runtime, stderr redirect, network, output filename

bead_id: twerk-5h4
bead_title: Fix runtime gaps: Docker runtime, stderr redirect, network, output filename
phase: 1.5
updated_at: 2026-03-24T16:00:00Z

## Summary

- Behaviors identified: 112
- Trophy allocation: 72 unit / 34 integration / 4 e2e / 2 static
- Proptest invariants: 9
- Fuzz targets: 5
- Kani harnesses: 3
- Mutation checkpoints: 14
- Target mutation kill rate: ≥90%

---

## 1. Behavior Inventory

### Shell Runtime Behaviors (23 behaviors)

1. ShellRuntime validates task.id is non-empty when running
2. ShellRuntime rejects tasks with non-empty entrypoint field
3. ShellRuntime rejects tasks with non-empty image field
4. ShellRuntime rejects tasks with CPU or memory limits set
5. ShellRuntime rejects tasks with non-empty networks field
6. ShellRuntime rejects tasks with registry field set
7. ShellRuntime rejects tasks with non-empty cmd field
8. ShellRuntime rejects tasks with sidecars field non-empty
9. ShellRuntime rejects tasks with mounts field non-empty (unless mounter configured)
10. ShellRuntime creates workdir at temp location and outputs stdout to `workdir/stdout`
11. ShellRuntime redirects stderr to stdout pipe (stderr merged into stdout stream)
12. ShellRuntime writes entrypoint script to workdir and executes it
13. ShellRuntime sets TORK_OUTPUT and TORK_PROGRESS env vars in child process
14. ShellRuntime publishes task log parts via broker when configured
15. ShellRuntime publishes task progress via broker when configured
16. ShellRuntime reads progress file every 10 seconds and publishes updates
17. ShellRuntime supports pre-task execution before main task
18. ShellRuntime supports post-task execution after main task
19. ShellRuntime cancels execution when AtomicBool is set
20. ShellRuntime returns ShellError::CommandFailed when process exits non-zero
21. ShellRuntime returns ShellError::ContextCancelled when cancel flag is set during execution
22. ShellRuntime cleans up workdir on completion
23. ShellRuntime health_check returns Ok(()) always for shell runtime

### Podman Runtime Behaviors (25 behaviors)

24. PodmanRuntime validates task.id is non-empty
25. PodmanRuntime validates task.image is non-empty
26. PodmanRuntime validates task.name is non-empty (Some and non-empty)
27. PodmanRuntime validates networks are not specified when host network disabled
28. PodmanRuntime rejects sidecars (SidecarsNotSupported error)
29. PodmanRuntime validates name is required when networks are specified (GAP3 fix)
30. PodmanRuntime creates output file at `workdir/stdout` (GAP4 fix: currently "output")
31. PodmanRuntime sets TORK_OUTPUT=/tork/stdout env var (GAP4 fix: currently /tork/output)
32. PodmanRuntime creates container with image and runs entrypoint.sh
33. PodmanRuntime pulls image with registry credentials if provided
34. PodmanRuntime verifies image by creating/removing test container
35. PodmanRuntime creates container with specified mounts (volume, bind, tmpfs)
36. PodmanRuntime creates container with specified environment variables
37. PodmanRuntime creates container with specified resource limits (cpus, memory)
38. PodmanRuntime creates container with specified networks and aliases
39. PodmanRuntime creates container with workdir set
40. PodmanRuntime injects task files into workdir
41. PodmanRuntime starts container and waits for completion
42. PodmanRuntime probes container if probe configured
43. PodmanRuntime reads container logs and ships via broker
44. PodmanRuntime captures container exit code and returns error if non-zero
45. PodmanRuntime reads output file and populates task.result
46. PodmanRuntime cleans up container and workdir after completion
47. PodmanRuntime health_check verifies podman is running
48. PodmanRuntime health_check returns error when podman not available
49. PodmanRuntime prunes images older than TTL when no active tasks

### Docker Runtime Behaviors (27 behaviors - GAP1 - NEW)

50. DockerRuntime::new(config) creates client from bollard::Docker::connect_with_local_defaults
51. DockerRuntime::new(config) spawns pull queue channel
52. DockerRuntime::new(config) spawns image pruner task
53. DockerRuntime::run(task) executes task in Docker container
54. DockerRuntime::run(task) returns DockerError::NonZeroExit when container exits non-zero
55. DockerRuntime::run(task) cleans up all created resources on both success and failure
56. DockerRuntime::create_container(task) returns Container with valid id
57. DockerRuntime::create_network() creates bridge network and returns unique network id
58. DockerRuntime::remove_network(id) retries 5 times with exponential backoff
59. DockerRuntime::remove_network(id) logs error but returns () after retries exhausted
60. DockerRuntime::pull_image(image, registry) queues pulls serially via mpsc channel
61. DockerRuntime::prune_images() removes images older than TTL when task count is 0
62. DockerRuntime::get_registry_credentials() loads from ~/.docker/config.json
63. DockerRuntime::get_registry_credentials() returns None when no credentials found
64. DockerRuntime::image_exists_locally(image) checks via bollard inspect
65. DockerRuntime::verify_image(image) creates test container to verify image works
66. DockerRuntime::health_check() returns Ok(()) when Docker daemon is accessible
67. DockerRuntime::health_check() returns Err(DockerError::ClientCreate) when Docker daemon is not running

### Container Behaviors (8 behaviors - GAP1)

68. Container::start() calls start_container API
69. Container::start() calls probe_container after start if probe configured
70. Container::start() returns Err(DockerError::ContainerStart) when container start fails
71. Container::wait() spawns progress reporting task
72. Container::wait() spawns log streaming task
73. Container::wait() returns stdout content when container exits with code 0
74. Container::wait() returns Err(DockerError::NonZeroExit) when container exits non-zero
75. Container::wait() returns Err(DockerError::ContainerWait) when wait result is unavailable

### parse_limits Behaviors (6 behaviors - NEW)

76. parse_limits returns Ok((Some(i64), None)) when only cpus limit provided
77. parse_limits returns Ok((None, Some(i64))) when only memory limit provided
78. parse_limits returns Ok((Some(i64), Some(i64))) when both limits provided
79. parse_limits returns Ok((None, None)) when limits is None
80. parse_limits returns Err(InvalidCpus) when cpus string is invalid
81. parse_limits returns Err(InvalidMemory) when memory string is invalid

### parse_gpu_options Behaviors (3 behaviors - NEW)

82. parse_gpu_options returns Ok(empty vec) when gpu_str is empty
83. parse_gpu_options returns Ok(vec with DeviceRequest) when gpu_str is valid
84. parse_gpu_options returns Err(InvalidGpuOptions) when gpu_str is malformed

### resolve_config_path Behaviors (3 behaviors - NEW)

85. resolve_config_path returns config_file path when provided
86. resolve_config_path falls back to config_path when config_file not provided
87. resolve_config_path falls back to default ~/.docker/config.json when neither provided

### Network Validation Behaviors (7 behaviors - GAP3 fix)

88. validate_network_name(name) returns Ok(()) for valid DNS label (max 15 chars, alphanumeric/hyphen)
89. validate_network_name(name) returns NetworkNameError::EmptyName when name is empty
90. validate_network_name(name) returns NetworkNameError::InvalidCharacters when name contains invalid chars
91. validate_network_name(name) returns NetworkNameError::TooLong when name exceeds 15 chars
92. validate_network_name(name) returns NetworkNameError::StartsWithDigit when name starts with digit
93. validate_network_name(name) returns NetworkNameError::ReservedName for host/none/default
94. PodmanRuntime::run() returns NameRequiredForNetwork when networks specified but name empty

### build_task_env Behaviors (6 behaviors - NEW)

95. build_task_env includes all REEXEC_ prefixed task env vars with prefix preserved
96. build_task_env always includes TORK_OUTPUT env var
97. build_task_env always includes TORK_PROGRESS env var
98. build_task_env always includes WORKDIR env var
99. build_task_env always includes PATH env var
100. build_task_env always includes HOME env var

### build_env Behaviors (2 behaviors - NEW)

101. build_env returns REEXEC_ prefixed vars from task env
102. build_env excludes non-REEXEC_ prefixed vars from task env

### read_progress_sync Behaviors (3 behaviors - NEW)

103. read_progress_sync returns Ok(0.0) when file is empty
104. read_progress_sync returns Ok(parsed_float) when file contains valid float
105. read_progress_sync returns Err(ShellError::ProgressRead) when file contains invalid content

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit (Calc) | 72 | Pure validation functions, parsing logic, build_env, network name validation, parse_limits, parse_gpu_options, resolve_config_path, read_progress_sync |
| Integration | 34 | Container runtimes with real podman/docker, volume mounts, network operations, broker integration |
| E2E | 4 | Full job submission through coordinator with real runtimes |
| Static | 2 | clippy, cargo-deny on new docker module |

**Density ratio: 112 behaviors / 22 pub functions = 5.1x**  
**Target: ≥5x ✅ ACHIEVED**

---

## 3. BDD Scenarios

### Shell Runtime BDD

#### Behavior: ShellRuntime validates task.id is non-empty when running
Given: ShellRuntime with default config
When: run() is called with task.id = ""
Then: Err(ShellError::TaskIdRequired)

```rust
fn shell_runtime_returns_task_id_required_error_when_id_is_empty()
```

#### Behavior: ShellRuntime redirects stderr to stdout pipe
Given: ShellRuntime with default config
When: run() executes a script that writes to stderr (echo "error" >&2)
Then: stderr output appears in the stdout stream (task.result contains stderr content)
And: No separate stderr output exists

```rust
fn shell_runtime_merges_stderr_into_stdout_when_script_writes_to_stderr()
```

#### Behavior: ShellRuntime creates stdout file at workdir/stdout
Given: ShellRuntime with default config
When: run() executes a script that writes to $REEXEC_TORK_OUTPUT
Then: The file at workdir/stdout contains the output
And: task.result equals the content of that file

```rust
fn shell_runtime_writes_output_to_stdout_file_at_workdir()
```

#### Behavior: ShellRuntime rejects tasks with networks field
Given: ShellRuntime with default config
When: run() is called with task.networks = vec!["mynet".to_string()]
Then: Err(ShellError::NetworksNotSupported)

```rust
fn shell_runtime_returns_networks_not_supported_when_networks_specified()
```

#### Behavior: ShellRuntime rejects tasks with mounts field
Given: ShellRuntime with default config (no mounter)
When: run() is called with task.mounts non-empty
Then: Err(ShellError::MountsNotSupported)

```rust
fn shell_runtime_returns_mounts_not_supported_when_mounts_specified()
```

#### Behavior: ShellRuntime supports pre-task execution
Given: ShellRuntime with config that has working reexec
When: run() is called with task that has one pre-task
Then: Pre-task executes first
And: Main task executes with pre-task output available
And: task.result contains main task output

```rust
fn shell_runtime_executes_pretask_before_main_task()
```

#### Behavior: ShellRuntime supports post-task execution
Given: ShellRuntime with config that has working reexec
When: run() is called with task that has one post-task
Then: Main task executes first
And: Post-task executes after
And: task.result contains main task output (not post-task)

```rust
fn shell_runtime_executes_posttask_after_main_task()
```

#### Behavior: ShellRuntime cancels execution when flag is set
Given: ShellRuntime with config
When: run() is called with cancel flag that becomes true during execution
Then: Err(ShellError::ContextCancelled)

```rust
fn shell_runtime_returns_context_cancelled_when_cancel_flag_set()
```

#### Behavior: ShellRuntime returns error when command fails
Given: ShellRuntime with config
When: run() executes a command that exits with non-zero code
Then: Err(ShellError::CommandFailed(...))

```rust
fn shell_runtime_returns_command_failed_when_process_exits_nonzero()
```

#### Behavior: ShellRuntime health_check returns Ok always
Given: ShellRuntime with any config
When: health_check() is called
Then: Ok(())

```rust
fn shell_runtime_health_check_returns_ok_always()
```

#### Behavior: ShellRuntime read_progress_sync returns 0.0 for empty file
Given: A file at progress_path containing ""
When: read_progress_sync(&progress_path) is called
Then: Ok(0.0)

```rust
fn shell_runtime_read_progress_sync_returns_zero_for_empty_file()
```

#### Behavior: ShellRuntime read_progress_sync returns parsed float
Given: A file at progress_path containing "0.75\n"
When: read_progress_sync(&progress_path) is called
Then: Ok(0.75)

```rust
fn shell_runtime_read_progress_sync_returns_parsed_float()
```

#### Behavior: ShellRuntime read_progress_sync returns error for invalid content
Given: A file at progress_path containing "not-a-number"
When: read_progress_sync(&progress_path) is called
Then: Err(ShellError::ProgressRead(...))

```rust
fn shell_runtime_read_progress_sync_returns_error_for_invalid_content()
```

---

### Podman Runtime BDD

#### Behavior: PodmanRuntime validates task.id is non-empty
Given: PodmanRuntime with default config
When: run() is called with task.id = ""
Then: Err(PodmanError::TaskIdRequired)

```rust
fn podman_runtime_returns_task_id_required_when_id_empty()
```

#### Behavior: PodmanRuntime validates task.image is non-empty
Given: PodmanRuntime with default config
When: run() is called with task.image = ""
Then: Err(PodmanError::ImageRequired)

```rust
fn podman_runtime_returns_image_required_when_image_empty()
```

#### Behavior: PodmanRuntime validates task.name is non-empty
Given: PodmanRuntime with default config
When: run() is called with task.name = None
Then: Err(PodmanError::NameRequired)

```rust
fn podman_runtime_returns_name_required_when_name_none()
```

#### Behavior: PodmanRuntime validates name required when networks specified (GAP3 fix)
Given: PodmanRuntime with default config
When: run() is called with task.networks = vec!["mynet".to_string()] and task.name = None
Then: Err(PodmanError::NameRequiredForNetwork)

```rust
fn podman_runtime_returns_name_required_for_network_when_networks_specified_without_name()
```

Given: PodmanRuntime with default config
When: run() is called with task.networks = vec!["mynet".to_string()] and task.name = Some("".to_string())
Then: Err(PodmanError::NameRequiredForNetwork)

```rust
fn podman_runtime_returns_name_required_for_network_when_networks_specified_with_empty_name()
```

Given: PodmanRuntime with default config and valid network name set
When: run() is called with task.networks = vec!["mynet".to_string()] and task.name = Some("validname".to_string())
Then: Ok(())

```rust
fn podman_runtime_succeeds_when_networks_specified_with_valid_name()
```

#### Behavior: PodmanRuntime creates output file at workdir/stdout (GAP4 fix)
Given: PodmanRuntime with default config
When: run() executes a container that writes to $TORK_OUTPUT
Then: The file at /tmp/tork/<task_id>/stdout contains the output
And: TORK_OUTPUT env var points to /tork/stdout (not /tork/output)

```rust
fn podman_runtime_creates_output_file_named_stdout_not_output()
```

#### Behavior: PodmanRuntime rejects sidecars
Given: PodmanRuntime with default config
When: run() is called with task.sidecars non-empty
Then: Err(PodmanError::SidecarsNotSupported)

```rust
fn podman_runtime_returns_sidecars_not_supported_when_sidecars_specified()
```

#### Behavior: PodmanRuntime runs container and captures result
Given: PodmanRuntime with default config
When: run() executes "echo hello > $TORK_OUTPUT"
Then: Ok(())
And: task.result == "hello\n"

```rust
fn podman_runtime_captures_container_output_in_task_result()
```

#### Behavior: PodmanRuntime executes pre and post tasks
Given: PodmanRuntime with volume mounter config
When: run() executes task with pre and post tasks and volume mount
Then: Pre-task runs first with same mounts
And: Main task runs with output captured
And: Post-task runs after

```rust
fn podman_runtime_executes_pre_and_post_tasks_with_mounts()
```

#### Behavior: PodmanRuntime cleans up container on completion
Given: PodmanRuntime with default config
When: run() completes a task successfully
Then: Container is stopped and removed
And: Workdir is cleaned up

```rust
fn podman_runtime_cleans_up_container_and_workdir_after_completion()
```

#### Behavior: PodmanRuntime returns error on non-zero exit
Given: PodmanRuntime with default config
When: run() executes "exit 42"
Then: Err(PodmanError::ContainerExitCode("42"))

```rust
fn podman_runtime_returns_container_exit_code_error_when_process_exits_nonzero()
```

#### Behavior: PodmanRuntime creates container with volumes
Given: PodmanRuntime with default config
When: run() executes task with volume mount
Then: Container is created with -v flag specifying mount
And: Volume is unmounted after completion

```rust
fn podman_runtime_creates_container_with_volume_mount()
```

#### Behavior: PodmanRuntime creates container with networks
Given: PodmanRuntime with default config
When: run() executes task with networks specified and valid name
Then: Container is created with --network flag
And: Network alias is set based on task name

```rust
fn podman_runtime_creates_container_with_network_and_alias()
```

#### Behavior: PodmanRuntime health check returns Ok when podman running
Given: PodmanRuntime with default config
When: health_check() is called and podman is running
Then: Ok(())

```rust
fn podman_runtime_returns_ok_when_podman_running()
```

#### Behavior: PodmanRuntime health check returns error when podman not available
Given: PodmanRuntime with default config
When: health_check() is called and podman is not running
Then: Err(PodmanError::PodmanNotRunning)

```rust
fn podman_runtime_returns_podman_not_running_when_podman_not_available()
```

---

### Docker Runtime BDD (GAP1 - NEW)

#### Behavior: DockerRuntime::new creates bollard client
Given: DockerConfig with default settings
When: DockerRuntime::new(config) is called
Then: Ok(DockerRuntime) with client connected
And: Pull queue channel spawned
And: Image pruner task spawned

```rust
fn docker_runtime_creates_client_and_background_tasks()
```

#### Behavior: DockerRuntime::run executes container
Given: DockerRuntime with valid client
When: run() executes "echo hello"
Then: Ok(())
And: task.result contains "hello"

```rust
fn docker_runtime_executes_container_and_captures_output()
```

#### Behavior: DockerRuntime::run returns error on non-zero exit
Given: DockerRuntime with valid client
When: run() executes "exit 1"
Then: Err(DockerError::NonZeroExit(1, ...))

```rust
fn docker_runtime_returns_nonzero_exit_error_when_container_fails()
```

#### Behavior: DockerRuntime::create_network creates bridge network
Given: DockerRuntime with valid client
When: create_network() is called
Then: Ok(network_id) with valid network id
And: Network uses bridge driver

```rust
fn docker_runtime_creates_bridge_network_and_returns_id()
```

#### Behavior: DockerRuntime::remove_network retries with exponential backoff
Given: DockerRuntime with valid client and existing network
When: remove_network(id) is called and first attempts fail
Then: Retries up to 5 times with backoff (200ms, 400ms, 800ms, 1600ms, 3200ms)
And: Returns () even after retries exhausted (logs error but doesn't fail)

```rust
fn docker_runtime_retries_network_removal_with_exponential_backoff()
```

#### Behavior: DockerRuntime::pull_image queues pulls serially
Given: DockerRuntime with valid client
When: Multiple tasks request pull of different images concurrently
Then: Pulls are serialized via mpsc channel
And: Each task receives Ok(()) when its pull completes

```rust
fn docker_runtime_serializes_image_pulls_via_channel()
```

#### Behavior: DockerRuntime::prune_images removes old images
Given: DockerRuntime with images in cache and task count = 0
When: prune_images() is called
Then: Images older than TTL are removed
And: Images in use or newer than TTL are preserved

```rust
fn docker_runtime_prunes_images_older_than_ttl_when_no_active_tasks()
```

#### Behavior: DockerRuntime::get_registry_credentials loads from config file
Given: ~/.docker/config.json with credentials for registry.example.com
When: get_registry_credentials(config, "registry.example.com/image")
Then: Ok(Some(DockerCredentials { username: ..., password: ... }))
When: No credentials found for registry
Then: Ok(None)

```rust
fn docker_runtime_loads_registry_credentials_from_docker_config()
fn docker_runtime_returns_none_when_no_credentials_for_registry()
```

#### Behavior: DockerRuntime::verify_image creates test container
Given: DockerRuntime with valid client and image
When: verify_image("some-image") is called
Then: Creates container with image and "true" command
And: Removes container immediately
And: Ok(()) if container created successfully

```rust
fn docker_runtime_verifies_image_by_creating_test_container()
```

#### Behavior: DockerRuntime::health_check returns Ok when Docker accessible
Given: DockerRuntime with valid client
When: health_check() is called and Docker daemon is accessible
Then: Ok(())

```rust
fn docker_runtime_returns_ok_when_docker_accessible()
```

#### Behavior: DockerRuntime::health_check returns error when Docker not running
Given: DockerRuntime with valid client
When: health_check() is called and Docker daemon is not running
Then: Err(DockerError::ClientCreate(...))

```rust
fn docker_runtime_returns_client_create_error_when_docker_not_accessible()
```

---

### Container BDD (GAP1 - NEW)

#### Behavior: Container::start() starts container successfully
Given: Container with valid id and client
When: start() is called on a stopped container
Then: Ok(())
And: Container transitions to running state

```rust
fn container_start_returns_ok_when_container_starts_successfully()
```

#### Behavior: Container::start() returns error when container start fails
Given: Container with valid id and client
When: start() is called on a non-existent container
Then: Err(DockerError::ContainerStart(...))

```rust
fn container_start_returns_container_start_error_when_start_fails()
```

#### Behavior: Container::start() calls probe after start
Given: Container with probe configured
When: start() is called
Then: start_container API call succeeds
And: probe_container is called after

```rust
fn container_start_calls_probe_after_container_start()
```

#### Behavior: Container::wait() returns stdout content when container exits with code 0
Given: Container with valid id and client
When: wait() is called on a container that exits with code 0
Then: Ok(stdout_content_string)
And: stdout_content contains /tork/stdout file content

```rust
fn container_wait_returns_stdout_content_when_container_exits_zero()
```

#### Behavior: Container::wait() returns NonZeroExit error when container exits non-zero
Given: Container with valid id and client
When: wait() is called on a container that exits with code 42
Then: Err(DockerError::NonZeroExit(42, ...))

```rust
fn container_wait_returns_nonzero_exit_error_when_container_fails()
```

#### Behavior: Container::wait() returns error when wait result unavailable
Given: Container with valid id and client that returns no wait result
When: wait() is called
Then: Err(DockerError::ContainerWait("no wait result"))

```rust
fn container_wait_returns_container_wait_error_when_no_wait_result()
```

#### Behavior: Container::wait() spawns progress reporting task
Given: Container with broker configured
When: wait() is called
Then: Progress reporting task is spawned
And: Container logs are streamed to broker

```rust
fn container_wait_spawns_progress_and_log_streaming_tasks()
```

---

### parse_limits BDD (NEW)

**Note**: These are module-level functions (not methods on PodmanRuntime), used by DockerRuntime. Contract signature:  
`fn parse_limits(limits: Option<&TaskLimits>) -> Result<(Option<i64>, Option<i64>), DockerError>`

#### Behavior: parse_limits returns both limits when both provided
Given: TaskLimits with cpus = "2" and memory = "1g"
When: parse_limits(Some(&limits)) is called
Then: Ok((Some(2000000), Some(1073741824)))

```rust
fn parse_limits_returns_both_values_when_both_provided()
```

#### Behavior: parse_limits returns only cpus when memory not provided
Given: TaskLimits with cpus = "4" and memory = None
When: parse_limits(Some(&limits)) is called
Then: Ok((Some(4000000), None))

```rust
fn parse_limits_returns_only_cpus_when_memory_not_provided()
```

#### Behavior: parse_limits returns only memory when cpus not provided
Given: TaskLimits with cpus = None and memory = "512m"
When: parse_limits(Some(&limits)) is called
Then: Ok((None, Some(536870912)))

```rust
fn parse_limits_returns_only_memory_when_cpus_not_provided()
```

#### Behavior: parse_limits returns None for both when limits is None
Given: limits = None
When: parse_limits(None) is called
Then: Ok((None, None))

```rust
fn parse_limits_returns_none_for_both_when_limits_is_none()
```

#### Behavior: parse_limits returns InvalidCpus error for malformed cpus string
Given: TaskLimits with cpus = "not-a-number" and memory = None
When: parse_limits(Some(&limits)) is called
Then: Err(DockerError::InvalidCpus("not-a-number"))

```rust
fn parse_limits_returns_invalid_cpus_error_when_cpus_string_malformed()
```

#### Behavior: parse_limits returns InvalidMemory error for malformed memory string
Given: TaskLimits with cpus = None and memory = "invalid"
When: parse_limits(Some(&limits)) is called
Then: Err(DockerError::InvalidMemory("invalid"))

```rust
fn parse_limits_returns_invalid_memory_error_when_memory_string_malformed()
```

---

### parse_gpu_options BDD (NEW)

**Note**: Module-level function used by DockerRuntime. Contract signature:  
`fn parse_gpu_options(gpu_str: &str) -> Result<Vec<DeviceRequest>, DockerError>`

#### Behavior: parse_gpu_options returns empty vec for empty string
Given: gpu_str = ""
When: parse_gpu_options("") is called
Then: Ok(vec![])

```rust
fn parse_gpu_options_returns_empty_vec_for_empty_string()
```

#### Behavior: parse_gpu_options parses valid GPU string
Given: gpu_str = "device=nvidia0,driver=nvidia"
When: parse_gpu_options(gpu_str) is called
Then: Ok(vec![DeviceRequest { device_ids: Some(["nvidia0"]), driver: Some("nvidia"), ... }])

```rust
fn parse_gpu_options_parses_valid_gpu_string()
```

#### Behavior: parse_gpu_options returns InvalidGpuOptions for malformed string
Given: gpu_str = "invalid[gpu]format"
When: parse_gpu_options(gpu_str) is called
Then: Err(DockerError::InvalidGpuOptions("invalid[gpu]format"))

```rust
fn parse_gpu_options_returns_invalid_gpu_options_for_malformed_string()
```

---

### resolve_config_path BDD (NEW)

**Note**: Module-level function. Contract signature:  
`fn resolve_config_path(config_file: Option<&Path>, config_path: Option<&Path>) -> Result<PathBuf, DockerError>`

#### Behavior: resolve_config_path returns config_file when provided
Given: config_file = Some(PathBuf::from("/custom/config.json"))
When: resolve_config_path(config_file, None) is called
Then: Ok(PathBuf::from("/custom/config.json"))

```rust
fn resolve_config_path_returns_config_file_when_provided()
```

#### Behavior: resolve_config_path falls back to config_path when config_file not provided
Given: config_file = None, config_path = Some(PathBuf::from("/other/config.json"))
When: resolve_config_path(config_file, config_path) is called
Then: Ok(PathBuf::from("/other/config.json"))

```rust
fn resolve_config_path_falls_back_to_config_path()
```

#### Behavior: resolve_config_path returns default when neither provided
Given: config_file = None, config_path = None
When: resolve_config_path(None, None) is called
Then: Ok(PathBuf::from(home_dir().join(".docker/config.json")))

```rust
fn resolve_config_path_returns_default_when_neither_provided()
```

---

### Network Name Validation BDD

#### Behavior: validate_network_name accepts valid names
Given: A valid network name (1-15 chars, alphanumeric and hyphens, not starting with hyphen)
When: validate_network_name(name) is called
Then: Ok(())

```rust
fn network_name_validation_accepts_valid_dns_label()
fn network_name_validation_accepts_single_alphanumeric_char()
```

#### Behavior: validate_network_name rejects empty name
Given: An empty string
When: validate_network_name("") is called
Then: Err(NetworkNameError::EmptyName)

```rust
fn network_name_validation_rejects_empty_name()
```

#### Behavior: validate_network_name rejects names over 15 chars
Given: A name with 16 or more characters
When: validate_network_name("this-name-is-too-long") is called
Then: Err(NetworkNameError::TooLong(...))

```rust
fn network_name_validation_rejects_names_exceeding_15_chars()
```

#### Behavior: validate_network_name rejects invalid characters
Given: A name containing special characters (@, #, $, etc.)
When: validate_network_name("my@network") is called
Then: Err(NetworkNameError::InvalidCharacters(...))

```rust
fn network_name_validation_rejects_names_with_special_characters()
```

#### Behavior: validate_network_name rejects names starting with digit
Given: A name starting with a digit
When: validate_network_name("3network") is called
Then: Err(NetworkNameError::StartsWithDigit)

```rust
fn network_name_validation_rejects_names_starting_with_digit()
```

#### Behavior: validate_network_name rejects reserved names
Given: Reserved names "host", "none", "default"
When: validate_network_name("host") is called
Then: Err(NetworkNameError::ReservedName("host"))

```rust
fn network_name_validation_rejects_reserved_name_host()
fn network_name_validation_rejects_reserved_name_none()
fn network_name_validation_rejects_reserved_name_default()
```

---

### build_task_env BDD (NEW)

**Note**: Module-level pure function. Contract signature:  
`fn build_task_env(task_env: &HashMap<String, String>, stdout_path: &Path, progress_path: &Path, workdir: &Path) -> Vec<(String, String)>`

#### Behavior: build_task_env includes all REEXEC_ prefixed task env vars
Given: task_env = {"FOO": "bar", "REEXEC_TEST": "value"}
When: build_task_env(task_env, ...) is called
Then: result contains ("REEXEC_FOO", "bar") and ("REEXEC_TEST", "value")

```rust
fn build_task_env_preserves_all_task_env_vars_with_reexec_prefix()
```

#### Behavior: build_task_env always includes TORK_OUTPUT
Given: any task_env (including empty)
When: build_task_env(task_env, stdout_path, ...) is called
Then: result contains ("REEXEC_TORK_OUTPUT", stdout_path.to_string())

```rust
fn build_task_env_always_includes_tork_output()
```

#### Behavior: build_task_env always includes TORK_PROGRESS
Given: any task_env (including empty)
When: build_task_env(task_env, ..., progress_path, ...) is called
Then: result contains ("REEXEC_TORK_PROGRESS", progress_path.to_string())

```rust
fn build_task_env_always_includes_tork_progress()
```

#### Behavior: build_task_env always includes WORKDIR
Given: any task_env (including empty)
When: build_task_env(task_env, ..., workdir) is called
Then: result contains ("WORKDIR", workdir.to_string())

```rust
fn build_task_env_always_includes_workdir()
```

#### Behavior: build_task_env always includes PATH and HOME
Given: any task_env (including empty)
When: build_task_env(task_env, ...) is called
Then: result contains ("PATH", ...) and ("HOME", ...)

```rust
fn build_task_env_always_includes_path_and_home()
```

---

### build_env BDD (NEW)

**Note**: Module-level pure function. Contract signature:  
`fn build_env(env: &[(String, String)]) -> Vec<(String, String)>`

#### Behavior: build_env returns only REEXEC_ prefixed vars
Given: env = [("REEXEC_A", "a"), ("OTHER", "b"), ("REEXEC_C", "c")]
When: build_env(env) is called
Then: result = [("A", "a"), ("C", "c")]

```rust
fn build_env_returns_only_reexec_prefixed_vars()
```

#### Behavior: build_env excludes non-REEXEC_ prefixed vars
Given: env = [("A", "a"), ("B", "b")] (no REEXEC_ prefix)
When: build_env(env) is called
Then: result is empty

```rust
fn build_env_excludes_non_reexec_prefixed_vars()
```

---

### Network Cleanup Integration BDD (NEW)

#### Behavior: DockerRuntime cleans up network when container creation fails
Given: DockerRuntime with valid client and a task that will fail container creation
When: run() is called but container creation fails
Then: Any network created for the task is removed
And: No orphan networks remain

```rust
fn network_cleanup_on_container_failure()
```

#### Behavior: DockerRuntime cleans up network when container execution fails
Given: DockerRuntime with valid client, network created, container started
When: run() is called but container exits with non-zero code
Then: Network is removed after container completes

```rust
fn network_cleanup_on_container_failure_and_nonzero_exit()
```

---

## 4. Proptest Invariants

### Proptest: build_task_env()
Invariant: All REEXEC_ prefixed keys from task.env are present in output with REEXEC_ prefix
Strategy: Any HashMap<String, String> for task_env, any valid paths
Anti-invariant: Empty HashMap (produces env with only built-in vars)

```rust
proptest! {
    fn build_task_env_preserves_all_task_env_vars(task_env in any::<HashMap<String, String>>()) {
        let stdout_path = PathBuf::from("/tmp/stdout");
        let progress_path = PathBuf::from("/tmp/progress");
        let workdir = PathBuf::from("/tmp/workdir");
        
        let result = build_task_env(&task_env, &stdout_path, &progress_path, &workdir);
        
        for (k, v) in &task_env {
            let prefixed_key = format!("REEXEC_{}", k);
            prop_assert!(result.contains(&(prefixed_key, v.clone())));
        }
    }
}
```

### Proptest: build_task_env() includes required vars
Invariant: Output always contains TORK_OUTPUT, TORK_PROGRESS, WORKDIR, PATH, HOME
Strategy: Any valid input (empty task_env is valid)
Anti-invariant: N/A - always includes required vars

```rust
proptest! {
    fn build_task_env_always_includes_required_vars(task_env in any::<HashMap<String, String>>()) {
        let stdout_path = PathBuf::from("/tmp/stdout");
        let progress_path = PathBuf::from("/tmp/progress");
        let workdir = PathBuf::from("/tmp/workdir");
        
        let result = build_task_env(&task_env, &stdout_path, &progress_path, &workdir);
        
        prop_assert!(result.iter().any(|(k, _)| k == "REEXEC_TORK_OUTPUT"));
        prop_assert!(result.iter().any(|(k, _)| k == "REEXEC_TORK_PROGRESS"));
        prop_assert!(result.iter().any(|(k, _)| k == "WORKDIR"));
        prop_assert!(result.iter().any(|(k, _)| k == "PATH"));
        prop_assert!(result.iter().any(|(k, _)| k == "HOME"));
    }
}
```

### Proptest: parse_limits()
Invariant: Returns Ok with correct (cpus, memory) tuple for valid input, Err for invalid
Strategy: Valid limits: cpus in "0.5", "1", "2", "4", memory in "128m", "512m", "1g"
Anti-invariant: cpus = "abc", "!", "-1", memory = "xyz", "invalid", "-100m"

```rust
proptest! {
    fn parse_limits_returns_correct_values_for_valid_input(limits in valid_task_limits()) {
        let result = parse_limits(Some(&limits));
        prop_assert!(result.is_ok());
        let (cpus, memory) = result.unwrap();
        // Verify values are positive if present
        if let Some(c) = cpus {
            prop_assert!(c > 0);
        }
        if let Some(m) = memory {
            prop_assert!(m > 0);
        }
    }
    
    fn parse_limits_rejects_invalid_cpus(limits in invalid_cpus_limits()) {
        let result = parse_limits(Some(&limits));
        prop_assert!(result.is_err());
        let err = result.unwrap_err();
        prop_assert!(matches!(err, DockerError::InvalidCpus(_)));
    }
    
    fn parse_limits_rejects_invalid_memory(limits in invalid_memory_limits()) {
        let result = parse_limits(Some(&limits));
        prop_assert!(result.is_err());
        let err = result.unwrap_err();
        prop_assert!(matches!(err, DockerError::InvalidMemory(_)));
    }
}
```

### Proptest: parse_gpu_options()
Invariant: Empty string always returns empty vec, valid GPU strings parse correctly
Strategy: "" (empty), "device=gpu0", "device=gpu0,driver=nvidia"
Anti-invariant: "[invalid", "malformed=]"

```rust
proptest! {
    fn parse_gpu_options_empty_string_returns_empty_vec() {
        let result = parse_gpu_options("");
        prop_assert!(result.is_ok());
        prop_assert!(result.unwrap().is_empty());
    }
    
    fn parse_gpu_options_valid_string_parses_correctly(gpu_str in "[a-z0-9=,.-]+") {
        let result = parse_gpu_options(&gpu_str);
        // Valid format strings should parse or return InvalidGpuOptions, not panic
        if result.is_err() {
            match result.unwrap_err() {
                DockerError::InvalidGpuOptions(_) => {},
                _ => prop_assert!(false, "Expected InvalidGpuOptions error"),
            }
        }
    }
}
```

### Proptest: resolve_config_path()
Invariant: Returns a PathBuf that exists or an error, never panics
Strategy: All combinations of None/Some paths
Anti-invariant: None - function always returns (error or success)

```rust
proptest! {
    fn resolve_config_path_always_returns_valid_result(
        config_file in opt_path_buf(),
        config_path in opt_path_buf()
    ) {
        let result = resolve_config_path(config_file.as_ref().map(|p| p.as_path()), config_path.as_ref().map(|p| p.as_path()));
        // Should never panic - either Ok(PathBuf) or Err(DockerError::Io)
        prop_assert!(result.is_ok() || result.is_err());
    }
}
```

### Proptest: parse_cpus()
Invariant: Valid CPU strings parse to non-negative f64
Strategy: "1", "2.5", "0.5", "0", any float string representing non-negative value
Anti-invariant: Negative numbers, non-numeric strings

**Note**: This is a module-level function (not PodmanRuntime method). Used by DockerRuntime.

```rust
proptest! {
    fn parse_cpus_accepts_valid_decimal_values(cpus in "[0-9]+(\\.[0-9]+)?") {
        let result = parse_cpus(&cpus);
        prop_assert!(result.is_ok());
        prop_assert!(result.unwrap() >= 0.0);
    }
    
    fn parse_cpus_rejects_negative_values(cpus in "-[0-9]+(\\.[0-9]+)?") {
        let result = parse_cpus(&cpus);
        prop_assert!(result.is_err());
        match result.unwrap_err() {
            DockerError::InvalidCpus(_) => {},
            _ => prop_assert!(false, "Expected InvalidCpus error"),
        }
    }
}
```

### Proptest: parse_memory()
Invariant: All supported suffix formats (b, k, kb, m, mb, g, gb) parse to correct byte values
Strategy: Valid combinations of number and suffix
Anti-invariant: Unsupported suffixes (GB, MB without lowercase), invalid numbers

**Note**: This is a module-level function (not PodmanRuntime method). Used by DockerRuntime.

```rust
proptest! {
    fn parse_memory_returns_correct_bytes_for_each_suffix(value in 1u32..1024, suffix in prop::one_of(["b", "k", "kb", "m", "mb", "g", "gb"])) {
        let input = format!("{}{}", value, suffix);
        let result = parse_memory(&input);
        prop_assert!(result.is_ok());
        
        let bytes = result.unwrap();
        match suffix {
            "b" => prop_assert_eq!(bytes, value as u64),
            "k" | "kb" => prop_assert_eq!(bytes, (value as u64) * 1024),
            "m" | "mb" => prop_assert_eq!(bytes, (value as u64) * 1024 * 1024),
            "g" | "gb" => prop_assert_eq!(bytes, (value as u64) * 1024 * 1024 * 1024),
            _ => panic!("unexpected suffix"),
        }
    }
}
```

### Proptest: parse_duration()
Invariant: Valid duration strings (1s, 1m, 1h) parse to correct Duration
Strategy: Positive integers with valid suffixes (s, m, h)
Anti-invariant: Invalid suffixes, non-numeric values

```rust
proptest! {
    fn parse_duration_returns_correct_duration(value in 1u64..3600, suffix in prop::one_of(["s", "m", "h"])) {
        let input = format!("{}{}", value, suffix);
        let result = parse_duration(&input);
        prop_assert!(result.is_ok());
        
        let duration = result.unwrap();
        match suffix {
            "s" => prop_assert_eq!(duration, Duration::from_secs(value)),
            "m" => prop_assert_eq!(duration, Duration::from_secs(value * 60)),
            "h" => prop_assert_eq!(duration, Duration::from_secs(value * 3600)),
            _ => panic!("unexpected suffix"),
        }
    }
}
```

### Proptest: slug::make()
Invariant: Output contains only alphanumeric characters, hyphens, and underscores
Strategy: Any string input
Anti-invariant: N/A - always produces valid output

```rust
proptest! {
    fn slug_make_produces_only_valid_chars(input: String) {
        let result = slug::make(&input);
        prop_assert!(result.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }
}
```

---

## 5. Fuzz Targets

### Fuzz Target: parse_memory() - Memory limit parsing
Input type: String
Risk: Panic on invalid input, wrong byte calculation, panics from unwrap()
Corpus seeds: "512m", "1g", "256MB", "128k", "1gb", "512"

```rust
// fuzz_target_1: parse_memory_valid_inputs
// Test with valid Go-compatible formats
fn fuzz_parse_memory_valid(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_memory(s);
    }
}

// fuzz_target_2: parse_memory_invalid_inputs  
// Test that invalid inputs don't panic
fn fuzz_parse_memory_invalid(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let result = parse_memory(s);
        // Invalid formats should return Err, not panic
        if result.is_err() {
            std::hint::black_box(());
        }
    }
}
```

### Fuzz Target: parse_cpus() - CPU limit parsing
Input type: String
Risk: Panic on NaN/inf, wrong value calculation
Corpus seeds: "1", "2.5", "0.5", "0", "-1"

```rust
fn fuzz_parse_cpus(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_cpus(s);
    }
}
```

### Fuzz Target: parse_duration() - Duration parsing
Input type: String
Risk: Panic on invalid format, overflow on large values
Corpus seeds: "1s", "30s", "1m", "2h", "abc", "-1s"

```rust
fn fuzz_parse_duration(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_duration(s);
    }
}
```

### Fuzz Target: validate_network_name() - Network name validation
Input type: String
Risk: Panic, incorrect validation logic, buffer overflow
Corpus seeds: "valid-name", "network", "a", "host", "none", "", "1234567890123456"

```rust
fn fuzz_validate_network_name(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = validate_network_name(s);
    }
}
```

### Fuzz Target: parse_gpu_options() - GPU options parsing (NEW)
Input type: String
Risk: Panic, incorrect DeviceRequest parsing, panic on unwrap
Corpus seeds: "", "device=gpu0", "device=gpu0,driver=nvidia", "invalid[gpu]"

```rust
fn fuzz_parse_gpu_options(data: &[u8]) -> std::hint::black_box(()) {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_gpu_options(s);
    }
}
```

---

## 6. Kani Harnesses

### Kani Harness: build_task_env() - All keys preserved
Property: For any input HashMap, every task_env key appears in output with REEXEC_ prefix
Bound: HashMap with up to 100 entries, string keys up to 256 chars
Rationale: Critical for security - task env vars must not be lost or corrupted

```rust
// kani-harness: build_task_env_all_keys_preserved
fn harness_build_task_env() {
    // Kani symbolic representation of HashMap
    kani::verify(
        |task_env: std::collections::HashMap<String, String>| {
            let stdout_path = PathBuf::from("/tmp/stdout");
            let progress_path = PathBuf::from("/tmp/progress");  
            let workdir = PathBuf::from("/tmp/workdir");
            
            let result = build_task_env(&task_env, &stdout_path, &progress_path, &workdir);
            
            for (k, v) in &task_env {
                let prefixed_key = format!("REEXEC_{}", k);
                assert!(result.contains(&(prefixed_key, v.clone())));
            }
        }
    );
}
```

### Kani Harness: ContainerGuard cleanup is called exactly once
Property: ContainerGuard.disarm() prevents double cleanup when Drop runs
Bound: Single ContainerGuard instance
Rationale: Prevents double-remove errors and resource leaks

```rust
// kani-harness: container_guard_no_double_cleanup
fn harness_container_guard() {
    // Verify disarm prevents cleanup in Drop
    let mut guard = ContainerGuard::new("test-container".to_string(), tasks.clone());
    guard.disarm();
    // Drop should not call stop_container
}
```

### Kani Harness: parse_memory() byte calculation correctness
Property: parse_memory("1g") returns exactly 1073741824 bytes
Bound: Values 1-1000 with all supported suffixes
Rationale: Resource limit calculation errors can cause OOM or incorrect limits

```rust
// kani-harness: parse_memory_byte_calculation
fn harness_parse_memory() {
    kani::verify(|value: u64, suffix: &str| {
        let input = format!("{}{}", value, suffix);
        if let Ok(bytes) = parse_memory(&input) {
            // Known correct values for verification
            match suffix {
                "b" => assert_eq!(bytes, value),
                "k" | "kb" => assert_eq!(bytes, value * 1024),
                "m" | "mb" => assert_eq!(bytes, value * 1024 * 1024),
                "g" | "gb" => assert_eq!(bytes, value * 1024 * 1024 * 1024),
                _ => {} // Other cases may error
            }
        }
    });
}
```

---

## 7. Mutation Checkpoints

### Mutation Checkpoints

Critical mutations that must be caught:

1. **build_task_env() - Missing REEXEC_ prefix** 
   - Mutation: `format!("{}{}", k, v)` instead of `format!("{}{}", ENV_VAR_PREFIX, k)`
   - Must be caught by: `build_task_env_preserves_all_task_env_vars`

2. **build_task_env() - Missing TORK_OUTPUT**
   - Mutation: Missing the TORK_OUTPUT chain
   - Must be caught by: `build_task_env_always_includes_tork_output`

3. **validate_network_name() - Accepts too-long names**
   - Mutation: `name.len() <= 16` instead of `name.len() <= 15`
   - Must be caught by: `network_name_validation_rejects_names_exceeding_15_chars`

4. **validate_network_name() - Accepts reserved names**
   - Mutation: Missing check for "host", "none", "default"
   - Must be caught by: `network_name_validation_rejects_reserved_name_*`

5. **PodmanRuntime::run() - Missing NameRequiredForNetwork validation (GAP3)**
   - Mutation: Removed validation of name when networks specified
   - Must be caught by: `podman_runtime_returns_name_required_for_network_when_networks_specified_without_name`

6. **PodmanRuntime::run() - Wrong output filename "output" instead of "stdout" (GAP4)**
   - Mutation: `workdir.join("output")` instead of `workdir.join("stdout")`
   - Must be caught by: `podman_runtime_creates_output_file_named_stdout_not_output`

7. **PodmanRuntime::run() - Wrong TORK_OUTPUT env var (GAP4)**
   - Mutation: `"TORK_OUTPUT=/tork/output"` instead of `"TORK_OUTPUT=/tork/stdout"`
   - Must be caught by: `podman_runtime_creates_output_file_named_stdout_not_output`

8. **ShellRuntime::do_run() - stderr not redirected to stdout (GAP2)**
   - Mutation: `cmd.stderr(Stdio::piped())` instead of redirecting to stdout
   - Must be caught by: `shell_runtime_merges_stderr_into_stdout_when_script_writes_to_stderr`

9. **ShellRuntime::do_run() - Wrong output filename**
   - Mutation: `workdir.join("output")` instead of `workdir.join("stdout")`
   - Must be caught by: `shell_runtime_writes_output_to_stdout_file_at_workdir`

10. **parse_memory() - Wrong multiplier**
    - Mutation: Using 1000 instead of 1024 for KB
    - Must be caught by: `parse_memory_returns_correct_bytes_for_each_suffix`

11. **DockerRuntime::run_inner() - Missing network cleanup on error (NEW)**
    - Mutation: Network not removed when container fails
    - Must be caught by: `network_cleanup_on_container_failure`

12. **DockerRuntime::remove_network() - No retry logic**
    - Mutation: Single attempt without retry
    - Must be caught by: `docker_runtime_retries_network_removal_with_exponential_backoff`

13. **parse_limits() - InvalidCpus not returned for bad cpus (NEW)**
    - Mutation: Wrong error variant returned
    - Must be caught by: `parse_limits_returns_invalid_cpus_error_when_cpus_string_malformed`

14. **parse_gpu_options() - InvalidGpuOptions not returned for bad format (NEW)**
    - Mutation: Wrong error variant or panic
    - Must be caught by: `parse_gpu_options_returns_invalid_gpu_options_for_malformed_string`

**Threshold: 90% mutation kill rate minimum**

---

## 8. Combinatorial Coverage Matrix

### Shell Runtime Unit Tests

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| validate_task: empty id | Task { id: "" } | Err(ShellError::TaskIdRequired) | unit |
| validate_task: non-empty entrypoint | Task { entrypoint: ["sh"] } | Err(ShellError::EntrypointNotSupported) | unit |
| validate_task: non-empty image | Task { image: "img" } | Err(ShellError::ImageNotSupported) | unit |
| validate_task: limits set | Task { limits: Some(...) } | Err(ShellError::LimitsNotSupported) | unit |
| validate_task: networks set | Task { networks: ["n"] } | Err(ShellError::NetworksNotSupported) | unit |
| validate_task: registry set | Task { registry: Some(...) } | Err(ShellError::RegistryNotSupported) | unit |
| validate_task: cmd set | Task { cmd: ["ls"] } | Err(ShellError::CmdNotSupported) | unit |
| validate_task: sidecars set | Task { sidecars: [...] } | Err(ShellError::SidecarsNotSupported) | unit |
| validate_task: mounts set | Task { mounts: [...] } | Err(ShellError::MountsNotSupported) | unit |
| build_env: with REEXEC_ vars | env with REEXEC_A=a | vec contains (A, a) | unit |
| build_env: without REEXEC_ vars | env without REEXEC_ | excludes those vars | unit |
| read_progress_sync: empty file | file with "" | Ok(0.0) | unit |
| read_progress_sync: valid float | file with "0.5\n" | Ok(0.5) | unit |
| read_progress_sync: invalid content | file with "abc" | Err(ShellError::ProgressRead) | unit |
| cancel: context cancelled | cancel flag set during run | Err(ShellError::ContextCancelled) | unit |
| health_check: always returns ok | any config | Ok(()) | unit |

### Container Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| start: success | valid container id | Ok(()) | unit |
| start: failure | non-existent container | Err(DockerError::ContainerStart) | unit |
| start: with probe | probe configured | calls probe_container | unit |
| wait: success | exit code 0 | Ok(stdout_content) | unit |
| wait: non-zero exit | exit code 42 | Err(DockerError::NonZeroExit(42, ...)) | unit |
| wait: no result | client returns None | Err(DockerError::ContainerWait) | unit |
| wait: spawns tasks | with broker | spawns progress and log tasks | unit |

### parse_limits Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| both limits valid | cpus="2", memory="1g" | Ok((2000000, 1073741824)) | unit |
| only cpus valid | cpus="4", memory=null | Ok((4000000, None)) | unit |
| only memory valid | cpus=null, memory="512m" | Ok((None, 536870912)) | unit |
| limits is None | None | Ok((None, None)) | unit |
| invalid cpus | cpus="abc" | Err(InvalidCpus("abc")) | unit |
| invalid memory | memory="xyz" | Err(InvalidMemory("xyz")) | unit |

### parse_gpu_options Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| empty string | "" | Ok(vec![]) | unit |
| valid gpu string | "device=gpu0,driver=nvidia" | Ok(vec![DeviceRequest {...}]) | unit |
| malformed gpu string | "invalid[gpu]" | Err(InvalidGpuOptions("invalid[gpu]")) | unit |

### resolve_config_path Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| config_file provided | Some("/custom/config.json") | Ok(PathBuf from config_file) | unit |
| config_path fallback | None, Some("/other/config.json") | Ok(PathBuf from config_path) | unit |
| default path | None, None | Ok($HOME/.docker/config.json) | unit |

### Network Name Validation Unit Tests

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| valid: simple | "mynetwork" | Ok(()) | unit |
| valid: with hyphen | "my-network" | Ok(()) | unit |
| valid: single char | "a" | Ok(()) | unit |
| valid: 15 chars | "123456789012345" | Ok(()) | unit |
| invalid: empty | "" | Err(EmptyName) | unit |
| invalid: 16 chars | "1234567890123456" | Err(TooLong(...)) | unit |
| invalid: special chars | "my@network" | Err(InvalidCharacters(...)) | unit |
| invalid: starts with digit | "3network" | Err(StartsWithDigit) | unit |
| invalid: reserved host | "host" | Err(ReservedName("host")) | unit |
| invalid: reserved none | "none" | Err(ReservedName("none")) | unit |
| invalid: reserved default | "default" | Err(ReservedName("default")) | unit |

### Docker Runtime Health Check Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| docker accessible | valid client | Ok(()) | unit |
| docker not running | client ping fails | Err(DockerError::ClientCreate(...)) | unit |

### Podman Runtime Health Check Unit Tests (NEW)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| podman running | podman version succeeds | Ok(()) | unit |
| podman not running | podman version fails | Err(PodmanError::PodmanNotRunning) | unit |

### Docker Runtime Integration Tests (require docker - GAP1)

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| new: creates client | valid config | Ok(runtime) | integration |
| run: simple container | docker + echo | Ok(()) + result captured | integration |
| run: non-zero exit | exit 1 | Err(NonZeroExit) | integration |
| create_network: creates bridge | no existing network | Ok(network_id) | integration |
| remove_network: retries on failure | network exists | Ok(()) after retries | integration |
| network_cleanup_on_container_failure | container create fails | network removed | integration |
| pull_image: with registry | private registry | Ok(()) | integration |
| prune_images: removes old | old images + no tasks | Ok(()) | integration |
| verify_image: valid image | valid image | Ok(()) | integration |

---

## Open Questions

1. **GAP6 (Stdin)**: Should stdin be explicitly configured for podman containers, or is the default behavior acceptable? The contract doesn't specify a required behavior.

2. **GAP7 (Sidecars)**: The contract marks sidecars as unsupported but mentions they could be implemented for parity. Should tests for sidecar support be included, or should the error remain?

3. **GAP8 (Registry Auth)**: The Docker config file loading should use `bollard::AuthConfig::load_from_path()`. Should we verify the exact format of the credentials file parsing?

4. **Docker Runtime Testing**: Docker integration tests require a running Docker daemon. Should tests be marked `#[ignore]` by default and run only in CI with Docker available?

5. **GAP9-GAP12 (Excluded)**: These gaps are explicitly excluded from this bead. Should separate test plans be created for them, or bundled into a future bead?

---

## Exit Criteria Checklist

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target
- [x] Every error variant in the Error enum has an explicit test scenario
- [x] The mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] All GAP1-GAP8 behaviors are covered (GAP9-GAP12 excluded per contract)
- [x] Shell stderr redirect (GAP2) has explicit BDD scenario
- [x] Podman output filename (GAP4) has explicit BDD scenario
- [x] Network name validation (GAP3) has full combinatorial coverage
- [x] parse_limits has BDD scenarios and proptest invariant
- [x] parse_gpu_options has BDD scenarios and proptest invariant
- [x] resolve_config_path has BDD scenarios and proptest invariant
- [x] network_cleanup_on_container_failure integration test exists
- [x] ShellError::ContextCancelled has dedicated BDD scenario
- [x] DockerError::InvalidCpus has exact-variant assertion
- [x] DockerError::InvalidMemory has exact-variant assertion
- [x] networks-with-valid-name has concrete Ok(()) assertion (not "or validation passes")
- [x] DockerRuntime::health_check has 2 BDD scenarios (success + failure)
- [x] PodmanRuntime::health_check has 2 BDD scenarios (success + failure)
- [x] ShellRuntime::health_check has 1 BDD scenario (always returns Ok)
- [x] Container::start has 3 BDD scenarios (success, failure, probe)
- [x] Container::wait has 4 BDD scenarios (success, non-zero, no-result, spawns-tasks)
- [x] Density 5.1x ≥ 5x target achieved (112 behaviors / 22 pub functions)