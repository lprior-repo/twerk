# Tasks

Tasks are the unit of execution in Twerk. Each task runs in a container.

## Basic Task

```yaml
- name: say hello
  image: ubuntu:mantic
  run: echo hello world
```

## Task Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Task name |
| `description` | string | Optional description |
| `image` | string | Container image |
| `run` | string | Shell script to execute |
| `cmd` | list | Override container entrypoint |
| `entrypoint` | list | Override container entrypoint |
| `env` | map | Environment variables |
| `files` | map | Files to create in working directory |
| `queue` | string | Target queue name |
| `var` | string | Store output under this key |
| `if` | string | Conditional execution expression |
| `timeout` | string | Max execution time (e.g., `5m`, `30s`) |
| `priority` | int | Queue priority (0-9) |
| `tags` | list | Metadata tags |
| `workdir` | string | Working directory |
| `gpus` | string | GPU access (e.g., `all`) |
| `retry` | object | Retry configuration |
| `limits` | object | Resource limits |
| `mounts` | list | Volume/bind/tmpfs mounts |
| `networks` | list | Container networks |
| `pre` | list | Pre-execution tasks |
| `post` | list | Post-execution tasks |
| `parallel` | object | Parallel task execution |
| `each` | object | Loop over items |
| `subjob` | object | Spawn sub-job |
| `registry` | object | Private registry credentials |
| `probe` | object | Health check configuration |

## Output Between Tasks

Tasks can pass output to subsequent tasks:

```yaml
name: multi-step job
tasks:
  - name: generate data
    var: my_data
    image: ubuntu:mantic
    run: echo -n "secret data" > $TWERK_OUTPUT
  - name: process data
    image: alpine:latest
    env:
      DATA: '{{ tasks.my_data }}'
    run: echo "Received: $DATA"
```

## Expressions

Use the `expr` language for dynamic values:

```yaml
name: conditional task
inputs:
  run_mode: production
tasks:
  - name: deploy
    if: "{{ inputs.run_mode == 'production' }}"
    image: alpine:latest
    run: echo "Deploying to production!"
```

**Available namespaces:**
- `inputs` — Job inputs
- `secrets` — Job secrets
- `tasks` — Previous task outputs
- `job` — Job metadata

## Environment Variables

```yaml
- name: configure app
  image: ubuntu:mantic
  env:
    APP_ENV: production
    LOG_LEVEL: debug
  run: |
    export APP_ENV
    ./start.sh
```

## Files

Create files in the task's working directory:

```yaml
- name: run script
  image: python:3
  files:
    script.py: |
      import requests
      response = requests.get("https://api.example.com")
      print(response.json())
  run: python script.py > $TWERK_OUTPUT
```

## Retry

```yaml
- name: unreliable task
  retry:
    limit: 3
  image: alpine:latest
  run: ./might-fail.sh
```

**Retry fields:**

| Field | Type | Description |
|-------|------|-------------|
| `limit` | int | Max retry attempts |
| `attempts` | int | Current attempt count |

## Timeout

```yaml
- name: long running task
  timeout: 5m
  image: alpine:latest
  run: sleep 300
```

## Resource Limits

```yaml
- name: constrained task
  limits:
    cpus: 0.5
    memory: 256m
  image: alpine:latest
  run: echo hello
```

## Parallel Tasks

Run multiple tasks concurrently:

```yaml
- name: parallel work
  parallel:
    tasks:
      - image: ubuntu:mantic
        run: sleep 2 && echo "Task 1 done"
      - image: ubuntu:mantic
        run: sleep 1 && echo "Task 2 done"
```

## Each Task (Loop)

Run a task for each item in a list:

```yaml
- name: process items
  each:
    list: '{{ sequence(1, 5) }}'
    concurrency: 2
    task:
      image: alpine:latest
      env:
        ITEM: '{{ item.value }}'
      run: echo "Processing $ITEM"
```

**Each fields:**

| Field | Type | Description |
|-------|------|-------------|
| `list` | expression | Items to iterate |
| `concurrency` | int | Max parallel executions |
| `task` | object | Task to run for each item |
| `var` | string | Output variable name |
| `size` | int | Total items |
| `index` | int | Current index |

## Sub-Job Task

Start another job from a task:

```yaml
- name: trigger sub-job
  subjob:
    name: my sub job
    tasks:
      - name: sub task 1
        image: alpine:latest
        run: echo "Sub job task"
```

**Sub-job fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Sub-job name |
| `tasks` | list | Sub-job tasks |
| `detached` | bool | Fire-and-forget |

## Mounts

Share files between pre/post tasks and main task:

```yaml
- name: process video
  image: jrottenberg/ffmpeg:3.4-alpine
  run: ffmpeg -i /tmp/input.mov /tmp/output.mp4
  mounts:
    - type: volume
      target: /tmp
  pre:
    - name: download video
      image: alpine:latest
      run: wget http://example.com/video.mov -O /tmp/input.mov
```

**Mount types:**
- `volume` — Docker volume (temporary)
- `bind` — Host path mount
- `tmpfs` — Memory filesystem (Linux only)

## Health Probe

Configure health check for task:

```yaml
- name: service task
  image: myservice:latest
  run: /start.sh
  probe:
    path: /health
    port: 8080
    timeout: 5s
```

## Private Registries

```yaml
- name: private image
  image: myregistry.com/myimage:latest
  registry:
    username: user
    password: secret
  run: echo hello
```

## Next Steps

- [Runtimes](runtimes.md) — Docker, Podman, Shell execution
- [Configuration](configuration.md) — Full configuration reference
