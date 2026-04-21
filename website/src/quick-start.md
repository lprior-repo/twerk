# Quick Start

Get Twerk running locally with no Postgres, RabbitMQ, Docker, or Podman.

## Start Twerk

Use the local-friendly in-memory and shell settings:

```bash
TWERK_BROKER_TYPE=inmemory \
TWERK_DATASTORE_TYPE=inmemory \
TWERK_RUNTIME_TYPE=shell \
./twerk run standalone
```

If you built from source inside this repository, the checked-in `config.toml` already uses the same settings, so `./target/release/twerk run standalone` works from the repo root.

Twerk starts on `http://localhost:8000`.

## Create a Job

Create `hello-shell.yaml`:

```yaml
name: hello shell
tasks:
  - name: say hello
    run: |
      echo "hello from twerk"
```

## Submit and Wait for Completion

```bash
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @hello-shell.yaml
```

`wait=true` blocks until the job finishes, which makes the first-run flow much easier to verify.

## Inspect the Run

```bash
curl http://localhost:8000/jobs
curl http://localhost:8000/jobs/<job-id>/log
```

## Health Check

```bash
./twerk health
# or
curl http://localhost:8000/health
```

## Distributed Mode

Run coordinator and workers separately when you want Postgres, RabbitMQ, and container-backed tasks:

```bash
# Terminal 1: Coordinator
TWERK_DATASTORE_TYPE=postgres \
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable" \
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
./twerk run coordinator

# Terminal 2: Worker
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
./twerk run worker
```

Container images require `docker` or `podman`. The zero-dependency quick start above uses the shell runtime instead.

## Next Steps

- [Jobs](jobs.md) — Learn about job definitions
- [Tasks](tasks.md) — Configure task behavior
- [CLI Reference](cli.md) — All available commands
- [REST API](rest-api.md) — API reference
