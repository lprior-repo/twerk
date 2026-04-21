# Configuration

Twerk reads TOML configuration from files and environment variables.

## Config File Locations

Twerk checks these locations in order:

1. `./config.local.toml`
2. `./config.toml`
3. `./config.local.yaml`
4. `./config.yaml`
5. `./config.local.yml`
6. `./config.yml`
7. `~/twerk/config.toml`
8. `~/twerk/config.yaml`
9. `/etc/twerk/config.toml`
10. `/etc/twerk/config.yaml`

`.yaml` and `.yml` filenames are legacy compatibility names only. Their contents must still be valid TOML.

Or specify a file directly:

```bash
TWERK_CONFIG=/path/to/config.toml twerk run standalone
```

## Environment Variables

Override any setting with:

```bash
TWERK_<SECTION>_<KEY>=value
```

Example: `TWERK_LOGGING_LEVEL=debug`

## Local Standalone Example

```toml
[logging]
level = "info"
format = "pretty"

[broker]
type = "inmemory"

[datastore]
type = "inmemory"

[coordinator]
address = "localhost:8000"

[worker]
address = "localhost:8001"

[runtime]
type = "shell"

[runtime.shell]
cmd = ["bash", "-c"]
uid = ""
gid = ""
```

This is the same shape as the repo-root `config.toml` used for the primary local docs journey.

## Distributed Example

```toml
[broker]
type = "rabbitmq"

[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"

[datastore]
type = "postgres"

[datastore.postgres]
dsn = "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable"

[runtime]
type = "docker"
```

Use `docker` or `podman` when your task definitions include container images.

For a fuller reference, see `configs/sample.config.toml` in the repository.

## Environment Variable Reference

| Config | Environment Variable |
|--------|---------------------|
| `logging.level` | `TWERK_LOGGING_LEVEL` |
| `logging.format` | `TWERK_LOGGING_FORMAT` |
| `broker.type` | `TWERK_BROKER_TYPE` |
| `broker.rabbitmq.url` | `TWERK_BROKER_RABBITMQ_URL` |
| `datastore.type` | `TWERK_DATASTORE_TYPE` |
| `datastore.postgres.dsn` | `TWERK_DATASTORE_POSTGRES_DSN` |
| `runtime.type` | `TWERK_RUNTIME_TYPE` |
| `coordinator.address` | `TWERK_COORDINATOR_ADDRESS` |
| `worker.address` | `TWERK_WORKER_ADDRESS` |

## Next Steps

- [REST API](rest-api.md) — API reference
- [Quick Start](quick-start.md) — Get started
