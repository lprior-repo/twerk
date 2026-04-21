# Twerk YAML Quick Reference

> Quick guide to creating and running YAML job definitions

---

## Table of Contents

1. [Running Jobs](#running-jobs)
2. [YAML Syntax Reference](#yaml-syntax-reference)
3. [Task Types Cheat Sheet](#task-types-cheat-sheet)
4. [Template Functions](#template-functions)
5. [Complete Examples](#complete-examples)

---

## Running Jobs

### Method 1: curl (Recommended)

```bash
# Start twerk with no external dependencies
TWERK_BROKER_TYPE=inmemory \
TWERK_DATASTORE_TYPE=inmemory \
TWERK_RUNTIME_TYPE=shell \
./target/release/twerk run standalone

# Submit job
curl -X POST http://localhost:8000/jobs \
  -H "Content-type: text/yaml" \
  --data-binary @my-job.yaml

# Get job ID from response
# {"id":"abc123...","state":"SCHEDULED",...}

# Check status
curl http://localhost:8000/jobs/abc123...

# Get job logs
curl http://localhost:8000/jobs/abc123.../log
```

### Method 2: curl with blocking wait

```bash
# Submit and wait until the job finishes
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-type: text/yaml" \
  --data-binary @my-job.yaml
```

### Method 3: Python

```python
import requests
import time

# Submit job
with open('my-job.yaml') as f:
    response = requests.post(
        'http://localhost:8000/jobs',
        headers={'Content-type': 'text/yaml'},
        data=f.read()
    )

job = response.json()
job_id = job['id']
print(f"Job ID: {job_id}")

# Poll for completion
while True:
    status = requests.get(f'http://localhost:8000/jobs/{job_id}').json()
    print(f"State: {status['state']}, Progress: {status['progress']*100:.0f}%")
    
    if status['state'] in ['COMPLETED', 'FAILED', 'CANCELLED']:
        break
    
    time.sleep(2)

print(f"Final state: {status['state']}")
if status['state'] == 'COMPLETED':
    print(f"Result: {status.get('result')}")
```

---

## YAML Syntax Reference

### Minimal Job

```yaml
name: my job
tasks:
  - name: say hello
    image: ubuntu:mantic
    run: echo "hello world"
```

### Complete Job Structure

```yaml
name: job name
description: optional description
tags: [tag1, tag2]
inputs:
  param1: value1
  param2: value2
secrets:
  apiKey: secret-value
output: "{{ tasks.finalResult }}"
defaults:
  retry:
    limit: 3
  timeout: 30m
  queue: default
  priority: 1
  limits:
    cpus: "2"
    memory: "4Gi"
tasks:
  - <task definitions>
webhooks:
  - url: https://example.com/hook
    event: JOB_COMPLETED
auto_delete:
  after: 24h
```

### Task Properties

| Property | Required | Type | Description | Example |
|----------|----------|------|-------------|---------|
| `name` | No | string | Task name | `"process data"` |
| `var` | No | string | Variable name for output | `result` |
| `image` | Yes* | string | Docker image | `ubuntu:mantic` |
| `run` | Yes* | string | Shell command | `echo hello` |
| `entrypoint` | No | array | Override entrypoint | `["bash", "-c"]` |
| `cmd` | No | array | Override command | `["sleep", "10"]` |
| `env` | No | map | Environment variables | `FOO: bar` |
| `files` | No | map | Inject files | `script.sh: \| code` |
| `timeout` | No | duration | Task timeout | `30s`, `5m`, `1h` |
| `retry.limit` | No | number | Max retries | `3` |
| `queue` | No | string | Queue name | `high-priority` |
| `priority` | No | number | Priority (0-4) | `1` |
| `mounts` | No | array | Volume mounts | See examples |
| `networks` | No | array | Networks | `["bridge"]` |
| `workdir` | No | string | Working directory | `/app` |
| `if` | No | string | Condition | `"{{ inputs.run }}"` |
| `limits` | No | object | Resource limits | `cpus: "2"` |
| `gpus` | No | string | GPU config | `"all"` |
| `pre` | No | array | Pre-tasks | `[...]` |
| `post` | No | array | Post-tasks | `[...]` |
| `parallel` | No | object | Parallel tasks | See below |
| `each` | No | object | Iterator | See below |
| `subjob` | No | object | Sub-job | See below |

*Required for standard tasks, not for parallel/each/subjob

---

## Task Types Cheat Sheet

### 1. Simple Task

```yaml
tasks:
  - name: simple task
    var: output
    image: ubuntu:mantic
    env:
      FOO: bar
    run: |
      echo "processing"
      echo -n "result" > $TWERK_OUTPUT
```

### 2. Parallel Tasks

```yaml
tasks:
  - name: run in parallel
    parallel:
      tasks:
        - name: task 1
          image: ubuntu:mantic
          run: echo "task 1"
        
        - name: task 2
          image: alpine:latest
          run: echo "task 2"
        
        - name: task 3
          image: debian:stable
          run: echo "task 3"
```

### 3. Each (Iterator)

```yaml
tasks:
  - name: iterate over list
    each:
      list: "{{ sequence(1,5) }}"
      task:
        name: process item
        var: item{{item.index}}
        image: ubuntu:mantic
        env:
          VALUE: "{{ item.value }}"
          INDEX: "{{ item.index }}"
        run: |
          echo "Processing item $INDEX with value $VALUE"
          echo -n "result-$VALUE" > $TWERK_OUTPUT
```

### 4. Sub-Job

```yaml
tasks:
  - name: run sub-job
    var: subjobResult
    subjob:
      name: nested job
      output: "{{ tasks.final }}"
      tasks:
        - name: step 1
          var: step1
          image: ubuntu:mantic
          run: echo -n "data1" > $TWERK_OUTPUT
        
        - name: step 2
          var: final
          image: ubuntu:mantic
          env:
            DATA: "{{ tasks.step1 }}"
          run: |
            echo "Processing $DATA"
            echo -n "final-result" > $TWERK_OUTPUT
```

### 5. Pre/Post Tasks

```yaml
tasks:
  - name: main task
    image: ubuntu:mantic
    pre:
      - name: setup
        image: ubuntu:mantic
        run: echo "setting up"
    
    run: echo "main work"
    
    post:
      - name: cleanup
        image: ubuntu:mantic
        run: echo "cleaning up"
```

### 6. Conditional Task

```yaml
inputs:
  deploy: "true"

tasks:
  - name: conditional task
    if: "{{ inputs.deploy }}"
    image: ubuntu:mantic
    run: echo "deploying"
```

---

## Template Functions

### Variable Access

```yaml
# Access inputs
env:
  VALUE: "{{ inputs.myInput }}"

# Access previous task output
env:
  DATA: "{{ tasks.previousTask }}"

# Access secrets (redacted in logs)
env:
  KEY: "{{ secrets.apiKey }}"

# Access job metadata
env:
  JOB_ID: "{{ job.id }}"
```

### Built-in Functions

```yaml
# Generate sequence
list: "{{ sequence(1,10) }}"  # [1,2,3,...,10]

# Parse JSON
list: "{{ fromJSON(tasks.jsonData) }}"

# Equality check
if: "{{ eq inputs.env \"production\" }}"

# Item in each loop
env:
  INDEX: "{{ item.index }}"
  VALUE: "{{ item.value }}"
```

### Output Variables

```yaml
tasks:
  - name: task with output
    var: myResult  # Captures stdout to variable
    image: ubuntu:mantic
    run: |
      # Output to $TWERK_OUTPUT to capture
      echo -n "result data" > $TWERK_OUTPUT
  
  - name: use output
    image: ubuntu:mantic
    env:
      RESULT: "{{ tasks.myResult }}"
    run: echo "Previous result: $RESULT"
```

---

## Complete Examples

### Example 1: Hello World

```yaml
name: hello world
output: "{{ tasks.hello }}"
tasks:
  - var: hello
    name: simple task
    image: ubuntu:mantic
    run: echo -n "hello world" > $TWERK_OUTPUT
```

**Run it:**
```bash
curl -X POST http://localhost:8000/jobs \
  -H "Content-type: text/yaml" \
  --data-binary @hello.yaml
```

---

### Example 2: Data Pipeline

```yaml
name: data pipeline
description: Fetch, process, and save data
inputs:
  apiUrl: "https://jsonplaceholder.typicode.com/posts"
  outputFile: "output.json"
tasks:
  - name: fetch data
    var: rawData
    image: curlimages/curl:latest
    env:
      URL: "{{ inputs.apiUrl }}"
    run: |
      curl -s $URL > $TWERK_OUTPUT
  
  - name: filter data
    var: filteredData
    image: badouralix/curl-jq
    env:
      DATA: "{{ tasks.rawData }}"
    run: |
      echo -n $DATA | jq '[.[] | {id: .id, title: .title}]' > $TWERK_OUTPUT
  
  - name: save to file
    image: ubuntu:mantic
    env:
      DATA: "{{ tasks.filteredData }}"
      OUTPUT: "{{ inputs.outputFile }}"
    mounts:
      - type: bind
        source: /tmp
        target: /data
    run: |
      echo -n $DATA > /data/$OUTPUT
      echo "Saved to /data/$OUTPUT"
```

---

### Example 3: Parallel API Calls

```yaml
name: parallel api calls
description: Call multiple APIs in parallel
tasks:
  - name: fetch all apis
    parallel:
      tasks:
        - name: get users
          var: users
          image: curlimages/curl:latest
          run: |
            curl -s https://jsonplaceholder.typicode.com/users > $TWERK_OUTPUT
        
        - name: get posts
          var: posts
          image: curlimages/curl:latest
          run: |
            curl -s https://jsonplaceholder.typicode.com/posts > $TWERK_OUTPUT
        
        - name: get comments
          var: comments
          image: curlimages/curl:latest
          run: |
            curl -s https://jsonplaceholder.typicode.com/comments > $TWERK_OUTPUT
  
  - name: combine results
    image: badouralix/curl-jq
    env:
      USERS: "{{ tasks.users }}"
      POSTS: "{{ tasks.posts }}"
      COMMENTS: "{{ tasks.comments }}"
    run: |
      echo "Users: $(echo $USERS | jq 'length')"
      echo "Posts: $(echo $POSTS | jq 'length')"
      echo "Comments: $(echo $COMMENTS | jq 'length')"
```

---

### Example 4: Batch Processing with Each

```yaml
name: batch processing
description: Process multiple files
inputs:
  files: '["file1.txt", "file2.txt", "file3.txt"]'
tasks:
  - name: process each file
    each:
      list: "{{ fromJSON(inputs.files) }}"
      task:
        name: process file
        var: result{{item.index}}
        image: ubuntu:mantic
        env:
          FILENAME: "{{ item.value }}"
        run: |
          echo "Processing $FILENAME"
          # Simulate processing
          echo -n "processed-$FILENAME" > $TWERK_OUTPUT
  
  - name: summary
    image: ubuntu:mantic
    run: |
      echo "Processed all files:"
      echo "  File 0: {{ tasks.result0 }}"
      echo "  File 1: {{ tasks.result1 }}"
      echo "  File 2: {{ tasks.result2 }}"
```

---

### Example 5: Retry with Backoff

```yaml
name: retry example
description: Handle transient failures
tasks:
  - name: flaky operation
    var: result
    image: ubuntu:mantic
    retry:
      limit: 3
    run: |
      # Simulate flaky service
      RANDOM_NUM=$((RANDOM % 4))
      
      if [ $RANDOM_NUM -eq 0 ]; then
        echo "Success!"
        echo -n "success" > $TWERK_OUTPUT
        exit 0
      else
        echo "Failed (attempt will be retried)"
        exit 1
      fi
  
  - name: continue
    image: ubuntu:mantic
    env:
      RESULT: "{{ tasks.result }}"
    run: |
      echo "Operation completed with result: $RESULT"
```

---

### Example 6: Sub-Jobs for Reusable Workflows

```yaml
name: main workflow
description: Compose complex workflows from sub-jobs
tasks:
  - name: run data pipeline
    var: pipelineResult
    subjob:
      name: etl pipeline
      output: "{{ tasks.load }}"
      tasks:
        - name: extract
          var: extract
          image: ubuntu:mantic
          run: echo -n "raw-data" > $TWERK_OUTPUT
        
        - name: transform
          var: transform
          image: ubuntu:mantic
          env:
            DATA: "{{ tasks.extract }}"
          run: |
            echo "Transforming $DATA"
            echo -n "transformed-data" > $TWERK_OUTPUT
        
        - name: load
          var: load
          image: ubuntu:mantic
          env:
            DATA: "{{ tasks.transform }}"
          run: |
            echo "Loading $DATA"
            echo -n "final-result" > $TWERK_OUTPUT
  
  - name: use result
    image: ubuntu:mantic
    env:
      RESULT: "{{ tasks.pipelineResult }}"
    run: echo "Pipeline completed with: $RESULT"
```

---

### Example 7: Conditional Deployment

```yaml
name: conditional deployment
description: Deploy based on environment
inputs:
  environment: "staging"
  commit: "abc123"
secrets:
  deployToken: "secret-token"
tasks:
  - name: run tests
    image: node:18
    run: |
      npm test
      echo "Tests passed"
  
  - name: deploy to staging
    if: "{{ eq inputs.environment \"staging\" }}"
    image: ubuntu:mantic
    env:
      COMMIT: "{{ inputs.commit }}"
      TOKEN: "{{ secrets.deployToken }}"
    run: |
      echo "Deploying commit $COMMIT to staging"
      # curl -H "Authorization: Bearer $TOKEN" https://api.staging.example.com/deploy
  
  - name: deploy to production
    if: "{{ eq inputs.environment \"production\" }}"
    image: ubuntu:mantic
    env:
      COMMIT: "{{ inputs.commit }}"
      TOKEN: "{{ secrets.deployToken }}"
    pre:
      - name: approval gate
        image: ubuntu:mantic
        run: |
          echo "Waiting for approval..."
          # In production, integrate with approval system
    run: |
      echo "Deploying commit $COMMIT to production"
      # curl -H "Authorization: Bearer $TOKEN" https://api.production.example.com/deploy
```

---

### Example 8: Resource-Intensive Task

```yaml
name: ml training
description: Train ML model with resource limits
tasks:
  - name: preprocess
    image: python:3-slim
    limits:
      cpus: "2"
      memory: "4Gi"
    timeout: 30m
    run: |
      python -c "print('Preprocessing data...')"
      # python preprocess.py
  
  - name: train
    image: pytorch/pytorch:latest
    gpus: "all"
    limits:
      cpus: "8"
      memory: "16Gi"
    timeout: 2h
    retry:
      limit: 1
    run: |
      python -c "print('Training model...')"
      # python train.py --epochs 100
  
  - name: evaluate
    image: python:3-slim
    limits:
      cpus: "2"
      memory: "4Gi"
    run: |
      python -c "print('Evaluating model...')"
      # python evaluate.py
```

---

## Common Patterns

### Pattern 1: Map-Reduce

```yaml
name: map reduce
tasks:
  - name: generate data
    var: data
    image: python:3-slim
    run: |
      python -c "import json; print(json.dumps(list(range(10))))" > $TWERK_OUTPUT
  
  - name: map phase
    each:
      list: "{{ fromJSON(tasks.data) }}"
      task:
        name: process item
        var: result{{item.index}}
        image: python:3-slim
        env:
          VALUE: "{{ item.value }}"
        run: |
          python -c "print(int('$VALUE') ** 2, end='')" > $TWERK_OUTPUT
  
  - name: reduce phase
    image: python:3-slim
    env:
      RESULTS: "[{{ tasks.result0 }}, {{ tasks.result1 }}, {{ tasks.result2 }}, {{ tasks.result3 }}, {{ tasks.result4 }}, {{ tasks.result5 }}, {{ tasks.result6 }}, {{ tasks.result7 }}, {{ tasks.result8 }}, {{ tasks.result9 }}]"
    run: |
      python -c "import os; results=eval(os.environ['RESULTS']); print(f'Sum: {sum(results)}')"
```

### Pattern 2: Fan-Out/Fan-In

```yaml
name: fan out fan in
tasks:
  - name: generate work items
    var: items
    image: ubuntu:mantic
    run: echo -n '["a","b","c","d","e"]' > $TWERK_OUTPUT
  
  - name: fan out
    parallel:
      tasks:
        - name: process a
          image: ubuntu:mantic
          run: echo "processing a"
        
        - name: process b
          image: ubuntu:mantic
          run: echo "processing b"
        
        - name: process c
          image: ubuntu:mantic
          run: echo "processing c"
  
  - name: fan in
    image: ubuntu:mantic
    run: echo "all work items processed"
```

### Pattern 3: Pipeline with Stages

```yaml
name: staged pipeline
description: Execute stages in sequence
tasks:
  - name: stage 1 - build
    var: buildOutput
    image: node:18
    run: |
      npm run build
      echo -n "build-complete" > $TWERK_OUTPUT
  
  - name: stage 2 - test
    parallel:
      tasks:
        - name: unit tests
          image: node:18
          run: npm run test:unit
        
        - name: integration tests
          image: node:18
          run: npm run test:integration
  
  - name: stage 3 - package
    image: docker:latest
    env:
      BUILD: "{{ tasks.buildOutput }}"
    run: |
      docker build -t myapp:latest .
      echo "Packaged with build: $BUILD"
  
  - name: stage 4 - deploy
    image: ubuntu:mantic
    run: |
      echo "Deploying to environment"
```

---

## Tips & Tricks

### 1. Debugging Tasks

```yaml
tasks:
  - name: debug task
    image: ubuntu:mantic
    env:
      DEBUG: "1"
    run: |
      set -x  # Enable command tracing
      echo "Debug info:"
      env | sort
      pwd
      ls -la
      # Your actual work here
```

### 2. Passing Large Data

```yaml
# Don't pass large data via environment variables
# Use files instead:

tasks:
  - name: generate large data
    image: ubuntu:mantic
    mounts:
      - type: volume
        target: /data
    run: |
      # Write to shared volume
      dd if=/dev/urandom of=/data/large-file.bin bs=1M count=100
  
  - name: process large data
    image: ubuntu:mantic
    mounts:
      - type: volume
        target: /data
    run: |
      # Read from shared volume
      md5sum /data/large-file.bin
```

### 3. Time-Based Operations

```yaml
tasks:
  - name: time-sensitive task
    image: ubuntu:mantic
    timeout: 5m
    run: |
      START=$(date +%s)
      
      while true; do
        # Do work
        sleep 10
        
        # Check elapsed time
        NOW=$(date +%s)
        ELAPSED=$((NOW - START))
        
        if [ $ELAPSED -gt 250 ]; then
          echo "Approaching timeout, wrapping up..."
          break
        fi
      done
```

### 4. Dynamic Task Generation

```yaml
tasks:
  - name: generate task list
    var: taskList
    image: python:3-slim
    run: |
      python -c "
      import json
      tasks = [f'task-{i}' for i in range(5)]
      print(json.dumps(tasks))
      " > $TWERK_OUTPUT
  
  - name: execute dynamic tasks
    each:
      list: "{{ fromJSON(tasks.taskList) }}"
      task:
        name: dynamic task
        image: ubuntu:mantic
        env:
          TASK_NAME: "{{ item.value }}"
        run: echo "Executing $TASK_NAME"
```

---

## Common Errors & Solutions

### Error: Task timeout

```yaml
# Solution: Increase timeout
tasks:
  - name: slow task
    timeout: 1h  # Increase from default
    run: long-running-process
```

### Error: Out of memory

```yaml
# Solution: Increase memory limit
tasks:
  - name: memory intensive
    limits:
      memory: "8Gi"
    run: memory-hungry-process
```

### Error: Image pull failure

```yaml
# Solution: Add registry credentials
tasks:
  - name: private image
    image: myregistry.com/myimage:latest
    registry:
      username: myuser
      password: "{{ secrets.dockerPassword }}"
    run: echo "using private image"
```

### Error: Variable not found

```yaml
# Common mistake:
tasks:
  - name: task1
    var: myResult
    run: echo "hello" > $TWERK_OUTPUT
  
  - name: task2
    env:
      # WRONG: task name, not var
      # VALUE: "{{ tasks.task1 }}"
      
      # CORRECT: use var name
      VALUE: "{{ tasks.myResult }}"
    run: echo $VALUE
```

---

## Summary

1. **Submit jobs** via curl, CLI, or API client
2. **Define tasks** in YAML with image, run, and optional properties
3. **Use templates** to pass data between tasks
4. **Control execution** with parallel, each, subjob
5. **Handle failures** with retry and timeout
6. **Secure secrets** with automatic redaction
7. **Monitor progress** via API or logs

For more examples, see:
- [examples/](examples/) directory
- [COMPREHENSIVE_GUIDE.md](COMPREHENSIVE_GUIDE.md)
- [website/src/](website/src/) documentation
