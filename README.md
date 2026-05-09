# Twerk

**Twerk** is a personal workflow engine for developers who want more than cron, but less than a full orchestration stack.

Run scheduled jobs, file-processing pipelines, and local automations across shell, Docker, or Podman — locally in zero-setup mode or in a distributed setup with PostgreSQL and RabbitMQ.

Twerk is built for individual developers automating their own workflows. It is **not production-ready** and is intended for personal, self-managed use.

## Documentation

| Topic | Link |
|-------|------|
| Full documentation | https://lprior-repo.github.io/twerk/ |
| Quick start | https://github.com/lprior-repo/twerk/blob/main/website/src/quick-start.md |
| Architecture | https://github.com/lprior-repo/twerk/blob/main/website/src/architecture.md |
| REST API | https://github.com/lprior-repo/twerk/blob/main/website/src/rest-api.md |
| YAML language spec | https://github.com/lprior-repo/twerk/blob/main/website/src/yaml-language-spec.md |
| Jobs reference | https://github.com/lprior-repo/twerk/blob/main/website/src/jobs.md |
| Tasks reference | https://github.com/lprior-repo/twerk/blob/main/website/src/tasks.md |
| Runtimes | https://github.com/lprior-repo/twerk/blob/main/website/src/runtimes.md |
| Configuration | https://github.com/lprior-repo/twerk/blob/main/website/src/configuration.md |
| CLI reference | https://github.com/lprior-repo/twerk/blob/main/website/src/cli.md |
| Examples | https://github.com/lprior-repo/twerk/blob/main/website/src/examples.md |
| OpenAPI Spec | https://github.com/lprior-repo/twerk/blob/main/docs/openapi.yaml |
| Inspiration (Tork) | https://github.com/runabol/tork |
| Releases | https://github.com/lprior-repo/twerk/releases |

## Why Twerk

- Replace scattered cron jobs, shell scripts, and ad hoc container commands with one workflow runner.
- Run tasks in shell, Docker, or Podman depending on how much isolation you need.
- Execute parallel workflows and each-loops with concurrency control.
- Schedule recurring jobs with cron syntax.
- Submit, monitor, and inspect jobs over HTTP.
- Start with a single binary in standalone mode, then scale out to coordinator/worker mode if needed.

## Quick start

Download and start Twerk in standalone mode:

```bash
curl -L https://github.com/lprior-repo/twerk/releases/latest/download/twerk-linux-x86_64.tar.gz | tar xz
./twerk server-start standalone
```

Submit a simple job:

```bash
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @- <<'EOF'
name: hello-world
tasks:
  - name: say hello
    run: echo "Hello from Twerk!"
EOF
```

## Example workflows

### Simple shell task

```yaml
name: hello-world
tasks:
  - name: say hello
    run: echo "Hello from Twerk!"
```

### Parallel fan-out

```yaml
name: parallel-work
tasks:
  - name: parent
    parallel:
      tasks:
        - name: task-a
          image: alpine:latest
          run: echo A
        - name: task-b
          image: alpine:latest
          run: echo B
```

### Each-loop with concurrency

```yaml
name: process-items
tasks:
  - name: each-loop
    each:
      list: '[1, 2, 3, 4, 5]'
      concurrency: 2
      task:
        image: alpine:latest
        run: echo "Item {{ item.value }}"
```

### Container task

```yaml
name: docker-work
tasks:
  - name: alpine-task
    image: alpine:latest
    run: echo "hello from a container"
```

## What works today

- Single binary, zero-setup standalone mode with in-memory broker and shell runtime.
- Docker, Podman, and shell task execution.
- Parallel tasks and each-loops with concurrency control.
- Scheduled jobs with cron syntax.
- HTTP API for job submission, inspection, cancellation, restart, and logs.
- Distributed mode with PostgreSQL and RabbitMQ.
- CLI, YAML workflow definitions, and OpenAPI surface.

## Who it's for

- Developers automating personal workflows.
- People running backups, file processing, local maintenance jobs, and script pipelines.
- Users who want something lighter than a full production orchestration platform.
- Builders who want container isolation without dragging in a larger platform.

## Who it's not for

- Teams that need strong security guarantees, governance, or formal support.
- Business-critical workloads where reliability and operational maturity are non-negotiable.
- Organizations that need audited controls, enterprise features, or production SLAs.
- Users looking for a battle-tested production workflow platform.

## Transparency

Twerk is an AI-assisted codebase built in public.

That means:
- I have stress-tested it heavily through adversarial review, manual proofs, and extensive runtime testing.
- I have **not** finished doing a full human audit of the entire codebase.
- Some parts are better validated than others.
- There may be implementation, reliability, or security issues that runtime testing has not exposed yet.

Use Twerk when you want a fast, flexible personal automation tool.
Do not use Twerk when you need production guarantees.

---

## CLI commands

### Server commands

| Command | Description |
|---------|-------------|
| `twerk server-start standalone` | Run coordinator and worker in one process |
| `twerk server-start coordinator` | Run coordinator only |
| `twerk server-start worker` | Run worker only |
| `twerk health` | Check server health |
| `twerk version` | Show version |

### Job commands

| Command | Description |
|---------|-------------|
| `twerk job list` | List all jobs |
| `twerk job create <body>` | Submit a new job |
| `twerk job get <id>` | Get job by ID |
| `twerk job log <id>` | Get job logs |
| `twerk job cancel <id>` | Cancel a job |
| `twerk job restart <id>` | Restart a job |

### Scheduled job commands

| Command | Description |
|---------|-------------|
| `twerk scheduled-job list` | List scheduled jobs |
| `twerk scheduled-job create <body>` | Create a scheduled job |
| `twerk scheduled-job get <id>` | Get scheduled job by ID |
| `twerk scheduled-job pause <id>` | Pause a scheduled job |
| `twerk scheduled-job resume <id>` | Resume a scheduled job |
| `twerk scheduled-job delete <id>` | Delete a scheduled job |

### Task commands

| Command | Description |
|---------|-------------|
| `twerk task get <id>` | Get task by ID |
| `twerk task log <id>` | Get task logs |

### Queue commands

| Command | Description |
|---------|-------------|
| `twerk queue list` | List all queues |
| `twerk queue get <name>` | Get queue by name |
| `twerk queue delete <name>` | Delete a queue |

### Other commands

| Command | Description |
|---------|-------------|
| `twerk node list` | List all nodes |
| `twerk metrics get` | Get metrics |
| `twerk user create <username> <password>` | Create a user |
| `twerk trigger list` | List triggers |
| `twerk trigger get <id>` | Get trigger by ID |
| `twerk trigger create <body>` | Create a trigger |
| `twerk trigger update <id> <body>` | Update a trigger |
| `twerk trigger delete <id>` | Delete a trigger |

---

## REST API

Base URL: `http://localhost:8000`

### System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/nodes` | List nodes |
| `GET` | `/nodes/{id}` | Get node by ID |
| `GET` | `/metrics` | Get metrics |
| `POST` | `/users` | Create user |

### Jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/jobs` | Submit a job |
| `GET` | `/jobs` | List jobs |
| `GET` | `/jobs/{id}` | Get job by ID |
| `DELETE` | `/jobs/{id}` | Delete a job |
| `GET` | `/jobs/{id}/log` | Get job logs |
| `PUT` | `/jobs/{id}/cancel` | Cancel a job |
| `POST` | `/jobs/{id}/cancel` | Cancel a job (alternative) |
| `PUT` | `/jobs/{id}/restart` | Restart a job |

### Scheduled jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scheduled-jobs` | Create a scheduled job |
| `GET` | `/scheduled-jobs` | List scheduled jobs |
| `GET` | `/scheduled-jobs/{id}` | Get scheduled job by ID |
| `PUT` | `/scheduled-jobs/{id}/pause` | Pause a scheduled job |
| `PUT` | `/scheduled-jobs/{id}/resume` | Resume a scheduled job |
| `DELETE` | `/scheduled-jobs/{id}` | Delete a scheduled job |

### Tasks

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/tasks/{id}` | Get task by ID |
| `GET` | `/tasks/{id}/log` | Get task logs |

### Queues

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/queues` | List queues |
| `GET` | `/queues/{name}` | Get queue by name |
| `DELETE` | `/queues/{name}` | Delete a queue |

### Triggers

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/triggers` | List triggers |
| `POST` | `/triggers` | Create a trigger |
| `GET` | `/triggers/{id}` | Get trigger by ID |
| `PUT` | `/triggers/{id}` | Update a trigger |
| `DELETE` | `/triggers/{id}` | Delete a trigger |

### OpenAPI

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/openapi.json` | OpenAPI spec |

---

## License

Apache-2.0
