# Quick Start

Get Twerk running with your first job in under 5 minutes.

## Start Twerk

In standalone mode (all-in-one):

```bash
./twerk run standalone
```

Twerk will start on `http://localhost:8000`.

## Create a Job

Create `hello.yaml`:

```yaml
name: hello job
tasks:
  - name: say hello
    image: ubuntu:mantic
    run: |
      echo -n hello world
  - name: say goodbye
    image: alpine:latest
    run: |
      echo -n bye world
```

## Submit the Job

```bash
curl -X POST http://localhost:8000/jobs \
  -H "Content-type: text/yaml" \
  --data-binary @hello.yaml
```

```json
{
  "id": "ed0dba93d262492b8cf26e6c1c4f1c98",
  "state": "SCHEDULED",
  ...
}
```

## Check Status

```bash
curl http://localhost:8000/jobs/ed0dba93d262492b8cf26e6c1c4f1c98
```

```json
{
  "id": "ed0dba93d262492b8cf26e6c1c4f1c98",
  "state": "COMPLETED",
  "tasks": [
    {"name": "say hello", "state": "COMPLETED"},
    {"name": "say goodbye", "state": "COMPLETED"}
  ],
  ...
}
```

## What Happened?

1. Twerk received your job and scheduled both tasks
2. Task 1 ran in an Ubuntu container
3. Task 2 ran in an Alpine container
4. Twerk marked the job as `COMPLETED`

## Distributed Mode

Run coordinator and workers separately:

```bash
# Terminal 1: Coordinator
TWERK_DATASTORE_TYPE=postgres \
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk" \
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
./twerk run coordinator

# Terminal 2: Worker
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
./twerk run worker
```

Submit jobs to the coordinator at `http://localhost:8000`.

## Health Check

```bash
./twerk health
# or
curl http://localhost:8000/health
```

## Next Steps

- [Jobs](jobs.md) — Learn about job definitions
- [Tasks](tasks.md) — Configure task behavior
- [CLI Reference](cli.md) — All available commands
- [REST API](rest-api.md) — API reference
