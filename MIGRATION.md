# Migration Guide: Go Tork → Rust Twerk

## Configuration Format

### TOML Configuration (Rust Twerk)

Rust Twerk uses TOML format for configuration, matching the Go version's `knadh/koanf` TOML parser. The configuration file is typically named `config.toml` and supports dot-notation keys.

**Default config file search paths (in order):**
1. `config.local.toml` (local override)
2. `config.toml` (project root)
3. `~/tork/config.toml` (user home directory)
4. `/etc/tork/config.toml` (system-wide)

**Custom config path:** Set `TORK_CONFIG` environment variable to specify a custom config file path.

### Environment Variable Overrides

All TOML config keys can be overridden via environment variables using the `TORK_` prefix. Keys are converted to uppercase with dots replaced by underscores.

**Examples:**
- `broker.rabbitmq.consumer.timeout` → `TORK_BROKER_RABBITMQ_CONSUMER_TIMEOUT`
- `middleware.web.logger.enabled` → `TORK_MIDDLEWARE_WEB_LOGGER_ENABLED`
- `runtime.podman.privileged` → `TORK_RUNTIME_PODMAN_PRIVILEGED`

### Sample Configuration

```toml
[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"
consumer.timeout = "30m"
management.url = ""
durable.queues = false
queue.type = "classic"

[worker.limits]
cpus = ""    # supports fractions
memory = ""  # e.g. 100m
timeout = "" # e.g. 3h

[mounts.bind]
allowed = false
sources = [] # a list of paths that are allowed as mount sources

[mounts.temp]
dir = "/tmp"

[runtime]
type = "docker" # docker | shell

[runtime.docker]
config = ""
privileged = false
image.ttl = "24h"

[runtime.podman]
privileged = false
host.network = false

[middleware.web.logger]
enabled = true
level = "DEBUG"        # TRACE|DEBUG|INFO|WARN|ERROR
skip_paths = ["GET /health"] # supports wildcards (*)

[middleware.web.cors]
enabled = false
origins = "*"
methods = "*"
credentials = false
headers = "*"

[middleware.web.ratelimit]
enabled = false
rps = 20
```

### Config Keys Reference

| Config Key | Type | Default | Description |
|---|---|---|---|
| `broker.rabbitmq.url` | string | `amqp://guest:guest@localhost:5672/` | RabbitMQ connection URL |
| `broker.rabbitmq.consumer.timeout` | duration | `30m` | Consumer timeout (e.g., `30m`, `1h`) |
| `broker.rabbitmq.management.url` | string | `""` | RabbitMQ Management API URL |
| `broker.rabbitmq.durable.queues` | bool | `false` | Whether queues should be durable |
| `broker.rabbitmq.queue.type` | string | `"classic"` | Queue type (`"classic"` or `"quorum"`) |
| `worker.limits.cpus` | string | `""` | Default CPU limit (e.g., `"1"`, `"2"`) |
| `worker.limits.memory` | string | `""` | Default memory limit (e.g., `"512m"`) |
| `worker.limits.timeout` | string | `""` | Default timeout (e.g., `"5m"`) |
| `mounts.bind.allowed` | bool | `false` | Whether bind mounts are allowed |
| `mounts.bind.sources` | list | `[]` | Allowed bind mount source paths |
| `mounts.temp.dir` | string | `"/tmp"` | Temp directory for mounts |
| `runtime.type` | string | `"docker"` | Runtime type (`"docker"`, `"shell"`, `"podman"`) |
| `runtime.docker.privileged` | bool | `false` | Run containers in privileged mode |
| `runtime.docker.image.ttl` | duration | `24h` | Image cache TTL |
| `runtime.podman.privileged` | bool | `false` | Run containers in privileged mode |
| `runtime.podman.host.network` | bool | `false` | Use host network for Podman |
| `middleware.web.logger.enabled` | bool | `true` | Enable request logging |
| `middleware.web.logger.level` | string | `"info"` | Log level |
| `middleware.web.logger.skip_paths` | list | `[]` | Paths to skip logging |

### Duration Format

Durations support the following units:
- `ns` - nanoseconds
- `us` - microseconds
- `ms` - milliseconds
- `s` - seconds
- `m` - minutes
- `h` - hours
- `d` - days

**Examples:** `5m`, `1h`, `30s`, `24h`, `7d`

### List Format

Lists can be specified as TOML arrays or comma-separated strings:

```toml
# TOML array format
skip_paths = ["GET /health", "GET /metrics"]

# Comma-separated string format (via environment variable)
# TORK_MIDDLEWARE_WEB_LOGGER_SKIP_PATHS="GET /health,GET /metrics"
```

### Wildcard Patterns

The logger's `skip_paths` supports wildcard patterns using `*`:

```toml
skip_paths = ["GET /health*", "POST /api/*"]
```

This will skip:
- `GET /health`
- `GET /healthcheck`
- `POST /api/users`
- etc.

## Go → Rust Differences

### Configuration API

**Go (knadh/koanf):**
```go
conf.String("broker.rabbitmq.url")
conf.Bool("runtime.podman.privileged")
conf.DurationDefault("broker.rabbitmq.consumer.timeout", 30*time.Minute)
```

**Rust (tork-runtime/conf):**
```rust
conf::string("broker.rabbitmq.url")
conf::runtime_podman_privileged()
conf::broker_rabbitmq_consumer_timeout()
```

### Key Naming Conventions

The Rust version uses snake_case function names that map to the dot-notation config keys:

| Go Config Key | Rust Function |
|---|---|
| `broker.rabbitmq.url` | `conf::string("broker.rabbitmq.url")` |
| `broker.rabbitmq.consumer.timeout` | `conf::broker_rabbitmq_consumer_timeout()` |
| `runtime.podman.privileged` | `conf::runtime_podman_privileged()` |
| `middleware.web.logger.skip` | `conf::middleware_web_logger_skip_paths()` |

### Custom Helper Functions

For commonly-used config keys, dedicated helper functions are provided:

```rust
// Broker RabbitMQ
pub fn broker_rabbitmq_consumer_timeout() -> time::Duration
pub fn broker_rabbitmq_durable_queues() -> bool
pub fn broker_rabbitmq_queue_type() -> String

// Worker Limits
pub fn worker_limits() -> WorkerLimits  // struct with cpus, memory, timeout

// Mounts
pub fn mounts_bind_allowed() -> bool
pub fn mounts_bind_sources() -> Vec<String>
pub fn mounts_temp_dir() -> String

// Runtime Docker
pub fn runtime_docker_privileged() -> bool
pub fn runtime_docker_image_ttl() -> time::Duration

// Runtime Podman
pub fn runtime_podman_privileged() -> bool
pub fn runtime_podman_host_network() -> bool

// Middleware Web Logger
pub fn middleware_web_logger_enabled() -> bool
pub fn middleware_web_logger_level() -> String
pub fn middleware_web_logger_skip_paths() -> Vec<String>
```
