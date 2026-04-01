# Runtimes

Twerk supports multiple execution environments for tasks.

## Docker (Default)

Tasks run in isolated Docker containers using the `bollard` crate.

```toml
[runtime]
type = "docker"
```

Or via environment:

```bash
TWERK_RUNTIME_TYPE=docker
```

**Docker-specific options:**

```toml
[runtime.docker]
config = ""              # Path to Docker config
privileged = false        # Privileged container mode
image.ttl = "24h"        # Image cache TTL
```

## Podman

Daemonless Docker alternative:

```toml
[runtime]
type = "podman"
```

```bash
TWERK_RUNTIME_TYPE=podman
```

**Podman-specific options:**

```toml
[runtime.podman]
privileged = false
host.network = false     # Use host network
```

## Shell

Run directly on the host (for development/testing):

```toml
[runtime]
type = "shell"
```

```bash
TWERK_RUNTIME_TYPE=shell
```

**Warning:** Shell runtime executes arbitrary code on the host. Use only in trusted environments.

**Shell-specific options:**

```toml
[runtime.shell]
cmd = ["bash", "-c"]     # Shell command
uid = "1000"             # Run as specific user
gid = "1000"            # Run with specific group
```

## Environment Variables in Tasks

| Variable | Description |
|----------|-------------|
| `TWERK_OUTPUT` | Write task output here |
| `TWERK_TASK_ID` | Current task ID |
| `TWERK_JOB_ID` | Current job ID |

## Next Steps

- [Configuration](configuration.md) — Full configuration reference
- [REST API](rest-api.md) — API reference
