# Examples

Real-world workflow examples.

## Simple Job

```yaml
name: hello world
tasks:
  - name: say hello
    image: ubuntu:mantic
    run: echo hello world
```

## Using Inputs

Inputs can be used in `env` values and other fields (but NOT in `run`):

```yaml
name: input example
inputs:
  message: hello world
  count: 5
tasks:
  - name: use inputs
    image: alpine:latest
    env:
      MESSAGE: '{{ inputs.message }}'
      COUNT: '{{ inputs.count }}'
    run: |
      for i in $(seq 1 $COUNT); do
        echo "$MESSAGE"
      done
```

## Each Task (Loop)

Use `each` to run a task for each item in a list:

```yaml
name: process items
inputs:
  items: '[1, 2, 3, 4, 5]'
tasks:
  - name: process each
    each:
      list: '{{ fromJSON(inputs.items) }}'
      concurrency: 2
      task:
        image: alpine:latest
        env:
          ITEM: '{{ item.value }}'
          INDEX: '{{ item.index }}'
        run: echo "Item $ITEM at index $INDEX"
```

## Parallel Tasks

Run multiple tasks concurrently:

```yaml
name: parallel work
tasks:
  - name: parallel parent
    parallel:
      tasks:
        - name: task a
          image: alpine:latest
          run: echo "A done"
        - name: task b
          image: alpine:latest
          run: sleep 2 && echo "B done"
        - name: task c
          image: alpine:latest
          run: echo "C done"
```

## Conditional Execution

Use `if` to conditionally run tasks:

```yaml
name: conditional workflow
inputs:
  environment: production
tasks:
  - name: deploy
    if: "{{ job.state == 'SCHEDULED' }}"
    image: alpine:latest
    run: echo "Deploying..."
```

## Retry on Failure

```yaml
name: with retry
tasks:
  - name: may fail
    retry:
      limit: 3
    image: alpine:latest
    run: ./might-fail.sh
```

## Resource Limits

```yaml
name: limited task
tasks:
  - name: constrained
    limits:
      cpus: "0.5"
      memory: "256m"
    image: alpine:latest
    run: echo hello
```

## Mounts

Share data between pre/post tasks and main task:

```yaml
name: with mounts
tasks:
  - name: process
    image: jrottenberg/ffmpeg:3.4-alpine
    run: ffmpeg -i /tmp/input.mov /tmp/output.mp4
    mounts:
      - type: volume
        target: /tmp
    pre:
      - name: download
        image: alpine:latest
        run: wget http://example.com/video.mov -O /tmp/input.mov
```

## Scheduled Job

```yaml
name: daily backup
schedule:
  cron: "0 2 * * *"
tasks:
  - name: backup
    image: postgres:15
    run: pg_dump -a mydb > /backups/dump.sql
```

## Environment Variable Reference

| Variable | Description |
|----------|-------------|
| `TWERK_OUTPUT` | Write task output here |

## Supported Expressions

**Works in `env` values, `image`, `queue`, `name`, `var`, `if`:**
- `{{ inputs.key }}` — Job inputs
- `{{ secrets.key }}` — Job secrets  
- `{{ item.value }}` — Each loop item value
- `{{ item.index }}` — Each loop item index

**Built-in functions:**
- `fromJSON(string)` — Parse JSON string
- `toJSON(value)` — Convert to JSON
- `sequence(start, stop)` — Generate integer range
- `split(string, delimiter)` — Split string
- `len(array)` — Array length
- `contains(array, item)` — Check membership

**Works in `if` condition:**
- `job.state` — Current job state
- `job.id` — Job ID
- `job.name` — Job name
- `task.state` — Current task state
- `task.id` — Task ID

**Note:** The `run` field is NOT evaluated — it's passed as raw shell script to the container.

## See Also

- [Jobs](jobs.md) — Job reference
- [Tasks](tasks.md) — Task reference
- [REST API](rest-api.md) — API reference
