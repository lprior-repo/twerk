# Configuration

Twerk is configured via `config.toml` or environment variables.

## Config File Locations

1. `./config.toml` (current directory)
2. `~/twerk/config.toml`
3. `/etc/twerk/config.toml`

Or specify with:

```bash
TWERK_CONFIG=/path/to/config.toml ./twerk run standalone
```

## Environment Variables

Override any setting:

```bash
TWERK_<SECTION>_<KEY>=value
```

Example: `TWERK_LOGGING_LEVEL=debug`

## Full Configuration

```toml
[cli]
banner.mode = "console"  # off | console | log

[client]
endpoint = "http://localhost:8000"

[logging]
level = "debug"          # debug | info | warn | error
format = "pretty"        # pretty | json

[broker]
type = "inmemory"       # inmemory | rabbitmq

[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"
consumer.timeout = "30m"
management.url = ""
durable.queues = false
queue.type = "classic"   # classic | quorum

[datastore]
type = "postgres"         # postgres | inmemory

[datastore.retention]
logs.duration = "168h"   # 7 days
jobs.duration = "8760h"  # 1 year

[datastore.encryption]
key = ""                 # Encryption key for secrets at rest

[datastore.postgres]
dsn = "host=localhost user=twerk password=twerk dbname=twerk"
max_open_conns = 10
max_idle_conns = 5
conn_max_lifetime = "1h"
conn_max_idle_time = "30m"

[coordinator]
address = "localhost:8000"
name = "Coordinator"

[coordinator.api]
endpoints.health = true
endpoints.jobs = true
endpoints.tasks = true
endpoints.nodes = true
endpoints.queues = true
endpoints.metrics = true
endpoints.users = true

[coordinator.queues]
completed = 1
error = 1
pending = 1
started = 1
heartbeat = 1
jobs = 1

[middleware.web.cors]
enabled = false
origins = "*"
methods = "*"
credentials = false
headers = "*"

[middleware.web.basicauth]
enabled = false

[middleware.web.keyauth]
enabled = false
key = ""

[middleware.web]
bodylimit = "500K"

[middleware.web.ratelimit]
enabled = false
rps = 20

[middleware.web.logger]
enabled = true
level = "DEBUG"
skip = ["GET /health"]

[middleware.job.redact]
enabled = false

[middleware.task.hostenv]
vars = []

[worker]
address = "localhost:8001"
name = "Worker"

[worker.queues]
default = 1

[worker.limits]
cpus = ""
memory = ""
timeout = ""

[mounts.bind]
allowed = false
sources = []

[mounts.temp]
dir = "/tmp"

[runtime]
type = "docker"         # docker | podman | shell

[runtime.shell]
cmd = ["bash", "-c"]
uid = ""
gid = ""

[runtime.docker]
config = ""
privileged = false
image.ttl = "24h"

[runtime.podman]
privileged = false
host.network = false
```

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
