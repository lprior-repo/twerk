# Contract Specification

## Metadata

```
bead_id: twerk-5h4
bead_title: Fix runtime gaps: Docker runtime, stderr redirect, network, output filename
phase: 1
updated_at: 2026-03-24T00:00:00Z
```

## Context

- **Feature**: Runtime parity gaps between Go tork and Rust twerk
- **Domain terms**:
  - `DockerRuntime` — container runtime using bollard crate
  - `ShellRuntime` — local shell command execution
  - `Task` — executable unit with image, cmd, env, mounts, networks, sidecars
  - `Mount` — volume mount (bind, tmpfs, volume)
  - `Network` — Docker/Podman bridge network for sidecar isolation
  - `Registry` — image registry credentials
  - `Container` — running Docker container with lifecycle management
  - `sidecar` — auxiliary container running alongside main task container
- **Assumptions**:
  - Docker daemon is available and accessible via bollard
  - Network operations require bridge driver
  - Registry auth uses ~/.docker/config.json format
- **Open questions**:
  - GAP9-GAP12 (progress tracking, memory parsing, CPU limits, concurrent sidecars) — excluded from this bead scope

## Preconditions

- [ ] `DockerRuntime::new(config)` requires Docker daemon accessible via local socket
- [ ] `DockerRuntime::run(task)` requires `task.id` is non-empty
- [ ] `DockerRuntime::run(task)` requires `task.image` is non-empty when using Docker runtime
- [ ] `ShellRuntime::do_run(task)` requires `task.run` is non-empty (shell script content)
- [ ] `create_network()` requires network name is valid (non-empty, valid DNS label)
- [ ] `create_container(task)` requires mount source/target validity per mount type

## Postconditions

- [ ] `DockerRuntime::new(config)` returns `Ok(runtime)` with client connected and workers spawned
- [ ] `DockerRuntime::run(task)` returns `Ok(())` when task completes with exit code 0
- [ ] `DockerRuntime::run(task)` returns `Err(DockerError::NonZeroExit)` when container exits non-zero
- [ ] `DockerRuntime::run(task)` cleans up all created resources (containers, networks, volumes) on both success and failure
- [ ] `DockerRuntime::create_container(task)` returns `Container` with valid `id`
- [ ] `DockerRuntime::create_network()` returns unique network `id`
- [ ] `DockerRuntime::remove_network(id)` eventually removes network (5 retries with exponential backoff)
- [ ] `ShellRuntime::do_run(task)` creates stdout file at `workdir/stdout` (not `workdir/output`)
- [ ] `ShellRuntime::do_run(task)` redirects stderr to stdout pipe (not separate stderr pipe)

## Invariants

- [ ] `DockerRuntime` always has valid `client` connection
- [ ] `DockerRuntime` image cache TTL is respected
- [ ] `DockerRuntime` pull queue serializes all image pulls
- [ ] `DockerRuntime` task count is accurate (increment on run, decrement on complete)
- [ ] `DockerRuntime` pruner only runs when task count is 0
- [ ] All created Docker networks use bridge driver
- [ ] Container workdir always includes `/tork` volume mount
- [ ] `TORK_OUTPUT=/tork/stdout` env var is set in all containers
- [ ] `TORK_PROGRESS=/tork/progress` env var is set in all containers
- [ ] Task `result` field contains stdout content on success

## Error Taxonomy

### DockerError Variants

```rust
// Client/Connection
DockerError::ClientCreate(String),           // bollard connection failure

// Task Validation
DockerError::TaskIdRequired,                 // task.id is empty
DockerError::NameRequiredForNetwork,        // networks specified but name empty

// Mount Validation
DockerError::VolumeTargetRequired,          // VOLUME mount missing target
DockerError::BindTargetRequired,            // BIND mount missing target
DockerError::BindSourceRequired,            // BIND mount missing source
DockerError::UnknownMountType(String),      // invalid mount type string

// Image Operations
DockerError::ImagePull(String),             // pull failed
DockerError::CorruptedImage(String),       // verification failed
DockerError::ImageNotFound(String),         // image does not exist
DockerError::ImageVerifyFailed(String),     // verify test container failed

// Container Lifecycle
DockerError::ContainerCreate(String),      // create failed or timeout
DockerError::ContainerStart(String),       // start failed
DockerError::ContainerWait(String),        // wait failed
DockerError::ContainerLogs(String),       // log read failed
DockerError::ContainerRemove(String),      // remove failed
DockerError::ContainerInspect(String),    // inspect failed

// Network Operations
DockerError::NetworkCreate(String),        // network creation failed
DockerError::NetworkRemove(String),        // network removal failed (after retries)
DockerError::InvalidNetworkName(String),   // network name validation failed

// Volume Operations
DockerError::VolumeCreate(String),         // volume creation failed
DockerError::VolumeRemove(String),         // volume removal failed

// File Operations
DockerError::CopyToContainer(String),      // tar copy failed
DockerError::CopyFromContainer(String),     // tar extract failed

// Resource Limits
DockerError::InvalidCpus(String),          // invalid CPU limit string
DockerError::InvalidMemory(String),        // invalid memory limit string
DockerError::InvalidGpuOptions(String),   // invalid GPU options string

// Health/Probe
DockerError::ProbeTimeout(String),         // health probe timeout
DockerError::ProbeError(String),           // probe HTTP error

// Security
DockerError::HostNetworkDisabled,         // host network mode not enabled

// Execution Result
DockerError::NonZeroExit(i64, String),    // container exited non-zero

// I/O
DockerError::Io(#[from] std::io::Error),  // stdio IO error
DockerError::Api(#[from] bollard::errors::Error), // bollard API error
```

### ShellError Variants

```rust
ShellError::TaskIdRequired,                    // task.id is empty
ShellError::MountsNotSupported,                // mounts not supported in shell
ShellError::EntrypointNotSupported,            // entrypoint not supported
ShellError::ImageNotSupported,                  // image not supported
ShellError::LimitsNotSupported,                 // limits not supported
ShellError::NetworksNotSupported,              // networks not supported in shell
ShellError::RegistryNotSupported,               // registry not supported
ShellError::CmdNotSupported,                    // cmd not supported (use run)
ShellError::SidecarsNotSupported,              // sidecars not supported
ShellError::WorkdirCreation(String),            // tempdir creation failed
ShellError::FileWrite(String),                 // file write failed
ShellError::OutputRead(String),                // stdout file read failed
ShellError::ProgressRead(String),              // progress file read failed
ShellError::CommandFailed(String),            // process exited non-zero
ShellError::ContextCancelled,                  // cancellation requested
```

### NetworkNameError Variants

```rust
NetworkNameError::EmptyName,               // network name is empty
NetworkNameError::InvalidCharacters(String), // name contains invalid chars
NetworkNameError::TooLong(String),         // name exceeds 15 chars (bridge limit)
NetworkNameError::StartsWithDigit,         // name starts with digit
NetworkNameError::ReservedName(String),    // name is reserved (host, none, default)
```

## Contract Signatures

### DockerRuntime

```rust
// Constructor
pub async fn new(config: DockerConfig) -> Result<Self, DockerError>

// Lifecycle
pub async fn run(&self, task: &mut Task) -> Result<(), DockerError>
pub async fn health_check(&self) -> Result<(), DockerError>

// Container management
pub async fn create_container(&self, task: &Task) -> Result<Container, DockerError>
async fn run_task(&self, task: &mut Task) -> Result<(), DockerError>

// Network management  
async fn create_network(&self) -> Result<String, DockerError>
async fn remove_network(&self, network_id: &str)

// Image management
async fn pull_image(&self, image: &str, registry: Option<&Registry>) -> Result<(), DockerError>
async fn do_pull_request(...) -> Result<(), DockerError>
async fn image_exists_locally(client: &Docker, name: &str) -> Result<bool, DockerError>
async fn verify_image(client: &Docker, image: &str) -> Result<(), DockerError>
async fn get_registry_credentials(...) -> Result<Option<DockerCredentials>, DockerError>
async fn prune_images(...)

// Utilities
fn parse_limits(limits: Option<&TaskLimits>) -> Result<(Option<i64>, Option<i64>), DockerError>
fn parse_gpu_options(gpu_str: &str) -> Result<Vec<DeviceRequest>, DockerError>
```

### Container

```rust
pub async fn start(&self) -> Result<(), DockerError>
pub async fn wait(&self) -> Result<String, DockerError>  // Returns stdout content
async fn probe_container(&self) -> Result<(), DockerError>
async fn read_logs_tail(&self, lines: usize) -> Result<String, DockerError>
async fn read_output(&self) -> Result<String, DockerError>
async fn init_torkdir(&self, run_script: Option<&str>) -> Result<(), DockerError>
async fn init_workdir(&self, files: &HashMap<String, String>, workdir: &str) -> Result<(), DockerError>
```

### ShellRuntime

```rust
pub fn new(config: ShellConfig) -> Self
pub async fn run(&self, cancel: Arc<AtomicBool>, task: &mut Task) -> Result<(), ShellError>
async fn do_run(&self, cancel: Arc<AtomicBool>, task: &mut Task) -> Result<(), ShellError>
pub async fn health_check(&self) -> Result<(), ShellError>

// Validation (pure calc)
fn validate_task(task: &Task) -> Result<(), ShellError>
fn build_task_env(...) -> Vec<(String, String)>
fn build_env() -> Vec<(String, String)>
```

### Network Validation

```rust
// Validates network name per Docker bridge naming rules
fn validate_network_name(name: &str) -> Result<(), NetworkNameError>

// Returns true if name is valid DNS label (max 15 chars, alphanumeric/hyphen, not starting with hyphen)
fn is_valid_network_name(name: &str) -> bool
```

### Credential Loading

```rust
// Loads registry credentials from Docker config file
async fn load_registry_credentials(
    config_file: Option<&Path>,
    config_path: Option<&Path>,
    image: &str,
) -> Result<Option<DockerCredentials>, DockerError>

// Resolves config file path: config_file > config_path > default ~/.docker/config.json
fn resolve_config_path(config_file: Option<&Path>, config_path: Option<&Path>) -> Result<PathBuf, DockerError>
```

## Non-goals

- [ ] GAP9: Shell progress tracking broker integration (already correct)
- [ ] GAP10: Memory limit parsing format parity (separate issue)
- [ ] GAP11: CPU limit handling nanocpus vs --cpus (separate issue)
- [ ] GAP12: Concurrent sidecar log reading (sidecars not yet supported)

## Gap-Specific Contracts

### GAP1: Docker Runtime (bollard)

**Preconditions:**
- Docker socket accessible at `/var/run/docker.sock` or via `DOCKER_HOST`
- bollard crate can connect with local defaults

**Postconditions:**
- `DockerRuntime` wraps bollard `Docker` client
- Image pull queue serializes via mpsc channel
- Pruner runs hourly and respects image TTL
- All Go docker.go behaviors are replicated

### GAP2: Shell stderr Redirect

**Preconditions:**
- Shell command spawning with piped stdout/stderr

**Postconditions:**
- Go behavior: `cmd.Stderr = cmd.Stdout` (stderr merged into stdout pipe)
- Rust current (incorrect): `cmd.stderr(Stdio::piped())` — separate stderr
- Rust fix: redirect stderr to stdout: `cmd.stderr(cmd.stdout.take().unwrap())`
- Result: stderr lines appear in stdout stream, not separately

### GAP3: Network Name Validation

**Preconditions:**
- Task specifies networks
- Docker/Podman network names must be valid bridge names

**Postconditions:**
- When `!task.networks.is_empty()` and `task.name.is_none_or(|n| n.is_empty())`:
  - Returns `DockerError::NameRequiredForNetwork`
- Network name validation rules:
  - Max 15 characters (bridge driver limit)
  - Alphanumeric and hyphens only
  - Cannot start with hyphen or digit
  - Cannot be reserved names: "host", "none", "default"

### GAP4: Output Filename "stdout" not "output"

**Preconditions:**
- Shell/Container workdir creation

**Postconditions:**
- Go behavior: files at `/tmp/tork/<task_id>/stdout`
- Rust shell (correct): `workdir.join("stdout")`
- Rust podman (incorrect at GAP4): `workdir.join("output")`
- Fix: change to `workdir.join("stdout")`
- `TORK_OUTPUT` env var must point to `/tork/stdout`

### GAP5: Network Create/Remove

**Preconditions:**
- Task has sidecars (requires network for sidecar isolation)

**Postconditions:**
- `create_network()` creates bridge network with UUID name
- `create_network()` returns network `id`
- `remove_network()` retries 5 times with exponential backoff (200ms → 3200ms)
- `remove_network()` logs error but does not fail after retries exhausted
- Network cleanup happens in `DockerRuntime::run()` defer

### GAP6: Stdin Config

**Preconditions:**
- Container creation for interactive tasks

**Postconditions:**
- When stdin is needed: container created with `-i` flag (interactive)
- When stdin not needed: no explicit stdin configuration (default)
- Podman: `create_cmd.arg("-i")` when interactive mode required

### GAP7: Sidecars Support

**Preconditions:**
- Task has `sidecars` field with non-empty vec

**Postconditions:**
- Each sidecar is created as separate container with same image/mounts/networks
- Sidecars start before main container
- Sidecars are removed after main container completes
- If any sidecar fails to start: main container still runs (best-effort)
- Network created for sidecar isolation when `!sidecars.is_empty()`

### GAP8: Registry Auth from Config File

**Preconditions:**
- Image references private registry with domain (e.g., `registry.example.com/image`)
- `~/.docker/config.json` contains credentials for that domain

**Postconditions:**
- `get_registry_credentials()` loads from config file
- Priority: `config.config_file` > `config.config_path` > default `~/.docker/config.json`
- Returns `DockerCredentials` with username/password
- If no credentials found: returns `None` (anonymous pull)
- Uses bollard `AuthConfig::load_from_path()` for parsing
