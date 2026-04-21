# Installation

## Requirements

1. **Rust 1.75+** — For building from source
2. **Bash-compatible shell** — For the zero-dependency shell runtime quick start
3. **Docker or Podman** — Optional, for image-based task execution
4. **PostgreSQL** — Optional, for persistence
5. **RabbitMQ** — Optional, for distributed mode

## Download Binary

```bash
# Check releases for your platform
curl -L https://github.com/runabol/twerk/releases/latest/download/twerk-linux-x86_64.tar.gz | tar xz
./twerk --help
```

## Build from Source

```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release -p twerk-cli
./target/release/twerk --help
```

For a local first run from the repo root, the checked-in `config.toml` already points Twerk at the in-memory broker/datastore and shell runtime:

```bash
./target/release/twerk run standalone
```

## Set up PostgreSQL

```shell
docker run -d \
  --name twerk-postgres \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=twerk \
  -e POSTGRES_USER=twerk \
  -e POSTGRES_DB=twerk \
  postgres:15.3
```

Run migration:

```bash
TWERK_DATASTORE_TYPE=postgres \
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable" \
./twerk migration
```

## Set up RabbitMQ (Distributed Mode)

```shell
docker run -d \
  --name twerk-rabbitmq \
  -p 5672:5672 \
  -p 15672:15672 \
  rabbitmq:3-management
```

Access management UI at `http://localhost:15672` (guest/guest).

## Configuration

Twerk is configured via TOML files or environment variables:

```bash
# Environment variable format
TWERK_<SECTION>_<KEY>=value

# Examples
TWERK_LOGGING_LEVEL=debug
TWERK_BROKER_TYPE=rabbitmq
TWERK_DATASTORE_TYPE=postgres
TWERK_RUNTIME_TYPE=docker
```

See [Configuration](configuration.md) for full reference.

## Next Steps

- [Quick Start](quick-start.md) — Run your first job
- [CLI Reference](cli.md) — All commands
