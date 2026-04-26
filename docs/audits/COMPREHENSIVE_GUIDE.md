# Twerk Comprehensive Guide

> Complete documentation with architecture diagrams, code flow, YAML examples, and usage guide

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [System Components](#system-components)
3. [Code Flow Diagrams](#code-flow-diagrams)
4. [YAML Job Definition Guide](#yaml-job-definition-guide)
5. [Use Cases & Examples](#use-cases--examples)
6. [Running Twerk](#running-twerk)
7. [Advanced Features](#advanced-features)

---

## Architecture Overview

### High-Level Architecture

```mermaid
graph TB
    Client[Client/Application]
    Coordinator[Coordinator]
    Broker[Message Broker<br/>RabbitMQ/In-Memory]
    Worker1[Worker 1]
    Worker2[Worker 2]
    Worker3[Worker N]
    Datastore[(PostgreSQL<br/>Datastore)]
    Runtime1[Docker Runtime]
    Runtime2[Podman Runtime]
    Runtime3[Shell Runtime]
    
    Client -->|REST API| Coordinator
    Coordinator -->|Store Job| Datastore
    Coordinator -->|Publish Task| Broker
    Broker -->|Deliver Task| Worker1
    Broker -->|Deliver Task| Worker2
    Broker -->|Deliver Task| Worker3
    Worker1 -->|Execute| Runtime1
    Worker2 -->|Execute| Runtime2
    Worker3 -->|Execute| Runtime3
    Worker1 -->|Report Result| Broker
    Worker2 -->|Report Result| Broker
    Worker3 -->|Report Result| Broker
    Broker -->|Task Complete| Coordinator
    Coordinator -->|Update State| Datastore
    
    style Coordinator fill:#4A90E2
    style Broker fill:#50C878
    style Datastore fill:#FF6B6B
    style Worker1 fill:#FFD93D
    style Worker2 fill:#FFD93D
    style Worker3 fill:#FFD93D
```

### Component Interaction Flow

```mermaid
sequenceDiagram
    participant C as Client
    participant CO as Coordinator
    participant DB as PostgreSQL
    participant B as RabbitMQ
    participant W as Worker
    participant R as Runtime
    
    C->>CO: POST /jobs (YAML)
    CO->>CO: Parse & Validate Job
    CO->>DB: Store Job (PENDING)
    CO->>DB: Store Tasks
    CO->>DB: Update Job (SCHEDULED)
    CO->>B: Publish Task 1
    CO->>B: Publish Task 2
    
    B->>W: Deliver Task 1
    W->>DB: Update Task (RUNNING)
    W->>R: Execute Container
    R-->>W: Exit Code & Output
    W->>DB: Update Task (COMPLETED)
    W->>B: Task Complete
    
    B->>CO: Task Completion Event
    CO->>DB: Update Job Progress
    CO->>B: Publish Task 2
    
    Note over C,R: Process repeats for all tasks
    
    CO->>DB: Update Job (COMPLETED)
    C->>CO: GET /jobs/{id}
    CO-->>C: Job Status (COMPLETED)
```

---

## System Components

### Component Architecture

```mermaid
graph LR
    subgraph "Coordinator Process"
        API[REST API]
        Engine[Job Engine]
        Scheduler[Task Scheduler]
        CoordinatorDB[DB Client]
        CoordinatorBroker[Broker Client]
    end
    
    subgraph "Worker Process"
        TaskRunner[Task Runner]
        Runtime[Runtime Abstraction]
        WorkerBroker[Broker Client]
        WorkerDB[DB Client]
        Health[Health Monitor]
    end
    
    subgraph "Infrastructure"
        RabbitMQ[RabbitMQ Broker]
        PostgreSQL[(PostgreSQL)]
        Docker[Docker Engine]
        Podman[Podman Engine]
        Shell[Shell Process]
    end
    
    API --> Engine
    Engine --> Scheduler
    Scheduler --> CoordinatorDB
    Scheduler --> CoordinatorBroker
    
    TaskRunner --> Runtime
    TaskRunner --> WorkerBroker
    TaskRunner --> WorkerDB
    TaskRunner --> Health
    
    CoordinatorBroker --> RabbitMQ
    WorkerBroker --> RabbitMQ
    CoordinatorDB --> PostgreSQL
    WorkerDB --> PostgreSQL
    
    Runtime --> Docker
    Runtime --> Podman
    Runtime --> Shell
    
    style API fill:#4A90E2
    style TaskRunner fill:#50C878
    style RabbitMQ fill:#FFD93D
    style PostgreSQL fill:#FF6B6B
```

### Deployment Modes

```mermaid
graph TB
    subgraph "Standalone Mode"
        Standalone[Single Process<br/>Coordinator + Worker]
        StandaloneDB[(PostgreSQL)]
        StandaloneMQ[RabbitMQ]
        Standalone --> StandaloneDB
        Standalone --> StandaloneMQ
    end
    
    subgraph "Distributed Mode"
        Coord[Coordinator]
        W1[Worker 1]
        W2[Worker 2]
        W3[Worker N]
        DistDB[(PostgreSQL)]
        DistMQ[RabbitMQ]
        
        Coord --> DistDB
        Coord --> DistMQ
        W1 --> DistMQ
        W2 --> DistMQ
        W3 --> DistMQ
        W1 -.-> DistDB
        W2 -.-> DistDB
        W3 -.-> DistDB
    end
    
    style Standalone fill:#4A90E2
    style Coord fill:#50C878
    style W1 fill:#FFD93D
    style W2 fill:#FFD93D
    style W3 fill:#FFD93D
```

---

## Code Flow Diagrams

### Job Lifecycle State Machine

```mermaid
stateDiagram-v2
    [*] --> PENDING: Job Created
    PENDING --> SCHEDULED: Tasks Queued
    SCHEDULED --> RUNNING: First Task Starts
    RUNNING --> RUNNING: Task Progress
    RUNNING --> COMPLETED: All Tasks Done
    RUNNING --> FAILED: Task Failed (no retry)
    RUNNING --> CANCELLED: User Cancelled
    FAILED --> RESTART: Retry Job
    COMPLETED --> [*]
    CANCELLED --> [*]
    
    note right of PENDING
        Job exists in DB
        Tasks not yet queued
    end note
    
    note right of SCHEDULED
        Tasks published to broker
        Waiting for workers
    end note
    
    note right of RUNNING
        Tasks executing
        Progress updates
    end note
```

### Task Lifecycle State Machine

```mermaid
stateDiagram-v2
    [*] --> CREATED: Task Defined
    CREATED --> SCHEDULED: Queued to Broker
    SCHEDULED --> RUNNING: Worker Starts
    RUNNING --> COMPLETED: Success (exit 0)
    RUNNING --> FAILED: Error (exit != 0)
    FAILED --> SCHEDULED: Retry Available
    RUNNING --> STOPPED: Timeout
    RUNNING --> CANCELLED: Job Cancelled
    SCHEDULED --> SKIPPED: Condition False
    COMPLETED --> [*]
    STOPPED --> [*]
    CANCELLED --> [*]
    SKIPPED --> [*]
    
    note right of RUNNING
        Container executing
        Logs streaming
        Resource limits applied
    end note
    
    note right of FAILED
        Retry logic applied
        Error captured
    end note
```

### Task Execution Flow

```mermaid
flowchart TD
    Start[Worker Receives Task] --> Pull{Image Exists?}
    Pull -->|No| PullImage[Pull Container Image]
    Pull -->|Yes| CreateContainer
    PullImage --> CreateContainer[Create Container]
    CreateContainer --> SetupMounts[Setup Mounts/Volumes]
    SetupMounts --> SetupEnv[Inject Environment Variables]
    SetupEnv --> SetupFiles[Inject Files]
    SetupFiles --> RunPre[Run Pre-Tasks]
    RunPre --> StartContainer[Start Container]
    StartContainer --> Monitor{Monitor Execution}
    Monitor -->|Running| StreamLogs[Stream Logs]
    StreamLogs --> Monitor
    Monitor -->|Exit 0| RunPost[Run Post-Tasks]
    Monitor -->|Exit != 0| CheckRetry{Retry Available?}
    CheckRetry -->|Yes| IncrementRetry[Increment Retry Count]
    IncrementRetry --> Pull
    CheckRetry -->|No| MarkFailed[Mark Task FAILED]
    RunPost --> ReadOutput[Read Task Output]
    ReadOutput --> MarkCompleted[Mark Task COMPLETED]
    MarkCompleted --> Cleanup[Cleanup Container]
    MarkFailed --> Cleanup
    Cleanup --> End[Task Complete]
    
    style Start fill:#4A90E2
    style MarkCompleted fill:#50C878
    style MarkFailed fill:#FF6B6B
    style End fill:#FFD93D
```

### Parallel Task Execution

```mermaid
flowchart TD
    Start[Parallel Task Starts] --> CreateTasks[Create Child Tasks]
    CreateTasks --> QueueAll[Queue All Tasks to Broker]
    QueueAll --> Wait{Wait for Completion}
    
    Wait --> Task1[Task 1 Executing]
    Wait --> Task2[Task 2 Executing]
    Wait --> Task3[Task N Executing]
    
    Task1 --> Done1[Task 1 Complete]
    Task2 --> Done2[Task 2 Complete]
    Task3 --> Done3[Task N Complete]
    
    Done1 --> CheckAll{All Complete?}
    Done2 --> CheckAll
    Done3 --> CheckAll
    
    CheckAll -->|No| Wait
    CheckAll -->|Yes| Aggregate[Aggregate Results]
    Aggregate --> Continue[Continue to Next Task]
    
    style Start fill:#4A90E2
    style Continue fill:#50C878
```

### Each Task (Iterator) Execution

```mermaid
flowchart TD
    Start[Each Task Starts] --> ParseList[Parse List Expression]
    ParseList --> CreateIterator[Create Iterator]
    CreateIterator --> Loop{More Items?}
    
    Loop -->|Yes| CreateChild[Create Child Task<br/>with item context]
    CreateChild --> QueueChild[Queue Child Task]
    QueueChild --> ExecChild[Execute Child Task]
    ExecChild --> StoreResult[Store Result as var]
    StoreResult --> Loop
    
    Loop -->|No| AllDone[All Items Processed]
    AllDone --> Continue[Continue to Next Task]
    
    note right of CreateChild
        item.index = 0, 1, 2...
        item.value = current list item
        Concurrency controls parallelism
    end note
    
    style Start fill:#4A90E2
    style Continue fill:#50C878
```

---

## YAML Job Definition Guide

### Basic Structure

```yaml
name: <job name>
description: <optional description>
tags: [optional, tags]
inputs:
  <key>: <value>
secrets:
  <key>: <sensitive value>  # Auto-redacted in logs
output: "{{ tasks.<var> }}"  # Template expression
defaults:
  retry:
    limit: <number>
  timeout: <duration>
  queue: <queue name>
  priority: <number>
  limits:
    cpus: <cpu count>
    memory: <memory size>
tasks:
  - <task definition>
```

### Task Types

```mermaid
graph TB
    Task[Task Definition]
    
    Task --> Simple[Simple Task]
    Task --> Parallel[Parallel Task]
    Task --> Each[Each/Iterator Task]
    Task --> SubJob[Sub-Job Task]
    
    Simple --> Container[Container Execution]
    Simple --> ShellVar[Shell Command<br/>with var capture]
    
    Parallel --> Multiple[Multiple Tasks<br/>Concurrent Execution]
    
    Each --> Iterate[Iterate Over List<br/>Create Child Tasks]
    
    SubJob --> Nested[Nested Job<br/>Independent Execution]
    
    style Task fill:#4A90E2
    style Simple fill:#50C878
    style Parallel fill:#FFD93D
    style Each fill:#FF6B6B
    style SubJob fill:#9B59B6
```

### Common Task Properties

| Property | Type | Description | Example |
|----------|------|-------------|---------|
| `name` | string | Task name | `"my task"` |
| `description` | string | Task description | `"processes data"` |
| `var` | string | Variable name for output | `result` |
| `image` | string | Container image | `ubuntu:mantic` |
| `run` | string | Shell command to execute | `echo hello` |
| `entrypoint` | array | Override entrypoint | `["bash", "-c"]` |
| `cmd` | array | Override command | `["sleep", "10"]` |
| `env` | map | Environment variables | `FOO: bar` |
| `files` | map | Files to inject | `script.sh: \| ...` |
| `timeout` | duration | Task timeout | `30s`, `5m`, `1h` |
| `retry.limit` | number | Max retry attempts | `3` |
| `queue` | string | Queue name | `high-priority` |
| `priority` | number | Task priority (0-4) | `1` |
| `mounts` | array | Volume mounts | See examples |
| `networks` | array | Networks to attach | `["bridge"]` |
| `workdir` | string | Working directory | `/app` |
| `if` | string | Conditional expression | `"{{ inputs.run }}"` |
| `pre` | array | Pre-execution tasks | `[...]` |
| `post` | array | Post-execution tasks | `[...]` |
| `limits` | object | Resource limits | See defaults |
| `gpus` | string | GPU requirements | `"all"` |

---

## Use Cases & Examples

### 1. Simple Sequential Tasks

**Use Case**: Basic data processing pipeline

```yaml
name: data pipeline
description: Process data through multiple stages
tasks:
  - name: fetch data
    var: rawData
    image: curlimages/curl:latest
    run: |
      curl -s https://api.example.com/data > $TWERK_OUTPUT
  
  - name: transform data
    var: transformedData
    image: badouralix/curl-jq
    env:
      DATA: "{{ tasks.rawData }}"
    run: |
      echo -n $DATA | jq '.items[]' > $TWERK_OUTPUT
  
  - name: save results
    image: amazon/aws-cli:2.13.10
    env:
      DATA: "{{ tasks.transformedData }}"
    run: |
      echo $DATA | aws s3 cp - s3://my-bucket/results.json
```

**Flow**:
```mermaid
graph LR
    A[Fetch Data] --> B[Transform Data]
    B --> C[Save Results]
    style A fill:#4A90E2
    style B fill:#50C878
    style C fill:#FFD93D
```

---

### 2. Parallel Processing

**Use Case**: Process multiple independent items concurrently

```yaml
name: parallel processing
description: Process multiple data sources in parallel
tasks:
  - name: gather sources
    var: sources
    image: ubuntu:mantic
    run: |
      echo '["source1","source2","source3","source4"]' > $TWERK_OUTPUT
  
  - name: process all sources
    parallel:
      tasks:
        - name: process source 1
          image: ubuntu:mantic
          run: echo "processing source 1"
        
        - name: process source 2
          image: alpine:latest
          run: echo "processing source 2"
        
        - name: process source 3
          image: debian:stable
          run: echo "processing source 3"
        
        - name: process source 4
          image: fedora:latest
          run: echo "processing source 4"
  
  - name: aggregate results
    image: ubuntu:mantic
    run: echo "all sources processed"
```

**Flow**:
```mermaid
graph TB
    A[Gather Sources] --> B1[Process Source 1]
    A --> B2[Process Source 2]
    A --> B3[Process Source 3]
    A --> B4[Process Source 4]
    B1 --> C[Aggregate Results]
    B2 --> C
    B3 --> C
    B4 --> C
    
    style A fill:#4A90E2
    style B1 fill:#50C878
    style B2 fill:#50C878
    style B3 fill:#50C878
    style B4 fill:#50C878
    style C fill:#FFD93D
```

---

### 3. Dynamic Iteration with `each`

**Use Case**: Process a dynamic list of items

```yaml
name: batch processing
description: Process multiple files dynamically
inputs:
  fileCount: "10"
tasks:
  - name: generate file list
    var: fileList
    image: python:3-slim
    env:
      COUNT: "{{ inputs.fileCount }}"
    run: |
      python -c "import json; files=[f'file{i}.txt' for i in range(int('$COUNT'))]; print(json.dumps(files))" > $TWERK_OUTPUT
  
  - name: process each file
    each:
      list: "{{ fromJSON(tasks.fileList) }}"
      task:
        name: process file
        var: fileResult{{item.index}}
        image: ubuntu:mantic
        env:
          FILENAME: "{{ item.value }}"
          INDEX: "{{ item.index }}"
        run: |
          echo "Processing $FILENAME (index $INDEX)"
          echo "result for $FILENAME" > $TWERK_OUTPUT
  
  - name: summary
    image: ubuntu:mantic
    run: |
      echo "Processed all files"
      echo "File 0 result: {{ tasks.fileResult0 }}"
      echo "File 1 result: {{ tasks.fileResult1 }}"
```

**Flow**:
```mermaid
graph TB
    A[Generate File List] --> B[Each: Iterate Over List]
    B --> C1[Process file0]
    B --> C2[Process file1]
    B --> C3[Process fileN]
    C1 --> D[Summary]
    C2 --> D
    C3 --> D
    
    style A fill:#4A90E2
    style B fill:#9B59B6
    style C1 fill:#50C878
    style C2 fill:#50C878
    style C3 fill:#50C878
    style D fill:#FFD93D
```

---

### 4. Retry with Backoff

**Use Case**: Handle flaky services or transient failures

```yaml
name: retry example
description: Retry failed tasks automatically
tasks:
  - name: call flaky api
    var: apiResult
    image: curlimages/curl:latest
    retry:
      limit: 3
    run: |
      RESPONSE=$(curl -s -w "%{http_code}" https://api.example.com/flaky)
      HTTP_CODE=${RESPONSE: -3}
      BODY=${RESPONSE:0:-3}
      
      if [ "$HTTP_CODE" = "200" ]; then
        echo -n "$BODY" > $TWERK_OUTPUT
        exit 0
      else
        echo "API returned $HTTP_CODE, retrying..."
        exit 1
      fi
  
  - name: process result
    image: ubuntu:mantic
    env:
      RESULT: "{{ tasks.apiResult }}"
    run: |
      echo "API call succeeded with: $RESULT"
```

**Retry Flow**:
```mermaid
stateDiagram-v2
    [*] --> Attempt1: First Try
    Attempt1 --> Success: HTTP 200
    Attempt1 --> Check1: HTTP Error
    Check1 --> Attempt2: Retry < 3
    Check1 --> Failed: Retry >= 3
    Attempt2 --> Success: HTTP 200
    Attempt2 --> Check2: HTTP Error
    Check2 --> Attempt3: Retry < 3
    Check2 --> Failed: Retry >= 3
    Attempt3 --> Success: HTTP 200
    Attempt3 --> Failed: Max Retries
    Success --> [*]
    Failed --> [*]
```

---

### 5. Timeout Handling

**Use Case**: Prevent long-running tasks from hanging

```yaml
name: timeout example
description: Enforce time limits on tasks
tasks:
  - name: quick operation
    var: quickResult
    image: ubuntu:mantic
    timeout: 10s
    run: |
      echo "This will complete quickly"
      echo "done" > $TWERK_OUTPUT
  
  - name: potentially slow operation
    image: ubuntu:mantic
    timeout: 30s
    run: |
      echo "This might take a while"
      sleep 25
      echo "completed within timeout"
  
  - name: handle timeout
    image: ubuntu:mantic
    run: |
      echo "Previous task completed or timed out"
```

---

### 6. Sub-Jobs (Nested Jobs)

**Use Case**: Reusable job components or complex workflows

```yaml
name: main job with sub-jobs
description: Compose complex workflows from sub-jobs
tasks:
  - name: prepare environment
    image: ubuntu:mantic
    run: echo "Setting up environment"
  
  - name: run data pipeline
    var: pipelineResult
    subjob:
      name: data processing pipeline
      output: "{{ tasks.finalResult }}"
      tasks:
        - name: extract
          var: extractedData
          image: ubuntu:mantic
          run: echo "extracted" > $TWERK_OUTPUT
        
        - name: transform
          var: transformedData
          image: ubuntu:mantic
          env:
            DATA: "{{ tasks.extractedData }}"
          run: echo "transformed $DATA" > $TWERK_OUTPUT
        
        - name: load
          var: finalResult
          image: ubuntu:mantic
          env:
            DATA: "{{ tasks.transformedData }}"
          run: echo "loaded $DATA" > $TWERK_OUTPUT
  
  - name: parallel sub-jobs
    parallel:
      tasks:
        - name: branch a
          subjob:
            name: processing branch a
            tasks:
              - name: step 1a
                image: ubuntu:mantic
                run: echo "branch a step 1"
              - name: step 2a
                image: ubuntu:mantic
                run: echo "branch a step 2"
        
        - name: branch b
          subjob:
            name: processing branch b
            tasks:
              - name: step 1b
                image: ubuntu:mantic
                run: echo "branch b step 1"
              - name: step 2b
                image: ubuntu:mantic
                run: echo "branch b step 2"
  
  - name: finalize
    image: ubuntu:mantic
    env:
      PIPELINE_RESULT: "{{ tasks.pipelineResult }}"
    run: |
      echo "Pipeline result: $PIPELINE_RESULT"
      echo "All sub-jobs completed"
```

**Architecture**:
```mermaid
graph TB
    A[Main Job] --> B[Prepare Environment]
    B --> C[Run Data Pipeline Sub-Job]
    
    subgraph "Sub-Job: Data Pipeline"
        C1[Extract] --> C2[Transform]
        C2 --> C3[Load]
    end
    
    C --> C1
    C3 --> D[Parallel Sub-Jobs]
    
    D --> E1[Branch A Sub-Job]
    D --> E2[Branch B Sub-Job]
    
    subgraph "Sub-Job: Branch A"
        E1a[Step 1A] --> E1b[Step 2A]
    end
    
    subgraph "Sub-Job: Branch B"
        E2a[Step 1B] --> E2b[Step 2B]
    end
    
    E1 --> E1a
    E2 --> E2a
    E1b --> F[Finalize]
    E2b --> F
    
    style A fill:#4A90E2
    style C fill:#9B59B6
    style D fill:#9B59B6
    style F fill:#50C878
```

---

### 7. Conditional Execution

**Use Case**: Run tasks based on conditions

```yaml
name: conditional tasks
description: Execute tasks based on runtime conditions
inputs:
  environment: "production"
  runTests: "true"
tasks:
  - name: setup
    image: ubuntu:mantic
    run: echo "Setting up"
  
  - name: run unit tests
    if: "{{ inputs.runTests }}"
    image: ubuntu:mantic
    run: echo "Running unit tests"
  
  - name: run integration tests
    if: "{{ inputs.runTests }}"
    image: ubuntu:mantic
    run: echo "Running integration tests"
  
  - name: deploy to staging
    if: "{{ eq inputs.environment \"staging\" }}"
    image: ubuntu:mantic
    run: echo "Deploying to staging"
  
  - name: deploy to production
    if: "{{ eq inputs.environment \"production\" }}"
    image: ubuntu:mantic
    run: echo "Deploying to production with extra safety checks"
```

---

### 8. Pre and Post Tasks

**Use Case**: Setup and cleanup around main task execution

```yaml
name: database migration
description: Run database migrations with backup and verification
tasks:
  - name: migrate database
    image: postgres:15
    env:
      DB_HOST: "{{ inputs.dbHost }}"
      DB_NAME: "{{ inputs.dbName }}"
    pre:
      - name: create backup
        var: backupFile
        image: postgres:15
        run: |
          BACKUP=/tmp/backup-$(date +%s).sql
          pg_dump -h $DB_HOST $DB_NAME > $BACKUP
          echo -n $BACKUP > $TWERK_OUTPUT
      
      - name: verify backup
        image: ubuntu:mantic
        env:
          BACKUP: "{{ tasks.backupFile }}"
        run: |
          if [ -f "$BACKUP" ]; then
            echo "Backup created successfully"
          else
            echo "Backup failed!"
            exit 1
          fi
    
    run: |
      psql -h $DB_HOST -d $DB_NAME -f /migrations/schema.sql
    
    post:
      - name: verify migration
        image: postgres:15
        run: |
          psql -h $DB_HOST -d $DB_NAME -c "SELECT version FROM schema_migrations"
      
      - name: cleanup old backups
        image: ubuntu:mantic
        run: |
          find /tmp -name "backup-*.sql" -mtime +7 -delete
          echo "Cleaned up old backups"
```

**Flow**:
```mermaid
graph TB
    A[Main Task: Migrate DB] --> B1[Pre: Create Backup]
    B1 --> B2[Pre: Verify Backup]
    B2 --> C[Execute Migration]
    C --> D1[Post: Verify Migration]
    D1 --> D2[Post: Cleanup Backups]
    
    style A fill:#4A90E2
    style B1 fill:#50C878
    style B2 fill:#50C878
    style C fill:#FFD93D
    style D1 fill:#9B59B6
    style D2 fill:#9B59B6
```

---

### 9. Resource Limits and GPU Tasks

**Use Case**: Control resource usage for compute-intensive tasks

```yaml
name: ml training
description: Train ML model with resource constraints
tasks:
  - name: preprocess data
    image: python:3-slim
    limits:
      cpus: "2"
      memory: "4Gi"
    run: |
      python preprocess.py
  
  - name: train model
    image: pytorch/pytorch:latest
    gpus: "all"
    limits:
      cpus: "8"
      memory: "16Gi"
    timeout: 2h
    retry:
      limit: 1
    run: |
      python train.py --epochs 100
  
  - name: evaluate model
    image: python:3-slim
    limits:
      cpus: "2"
      memory: "4Gi"
    run: |
      python evaluate.py
```

---

### 10. Complete Video Processing Pipeline (Real-World Example)

See `examples/split_and_stitch.yaml` for a complete example that:
- Splits video into chunks
- Transcodes in parallel
- Stitches results back together

---

## Running Twerk

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Docker (for runtime)
# See: https://docs.docker.com/get-docker/

# Install PostgreSQL
# Ubuntu/Debian:
sudo apt-get install postgresql postgresql-contrib

# Install RabbitMQ (optional, for distributed mode)
# Ubuntu/Debian:
sudo apt-get install rabbitmq-server
```

### Building from Source

```bash
# Clone repository
git clone https://github.com/runabol/twerk.git
cd twerk

# Build release binary
cargo build --release -p twerk-cli

# Binary will be at: target/release/twerk
```

### Running in Standalone Mode

**Simplest option - all-in-one process**

```bash
# Using the repo-root config.toml
./target/release/twerk run standalone

# Using a custom config file
TWERK_CONFIG=/path/to/config.toml ./target/release/twerk run standalone

# Using environment variables
TWERK_LOGGING_LEVEL=info \
TWERK_BROKER_TYPE=inmemory \
TWERK_DATASTORE_TYPE=inmemory \
TWERK_RUNTIME_TYPE=shell \
./target/release/twerk run standalone
```

**Coordinator starts on** `http://localhost:8000`

### Running in Distributed Mode

**Terminal 1: Start PostgreSQL**
```bash
# Create database
sudo -u postgres psql -c "CREATE DATABASE twerk;"
sudo -u postgres psql -c "CREATE USER twerk WITH PASSWORD 'twerk';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE twerk TO twerk;"

# Run migrations
./target/release/twerk migration -y
```

**Terminal 2: Start RabbitMQ**
```bash
# Start RabbitMQ server
sudo systemctl start rabbitmq-server

# Or with Docker
docker run -d --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3-management
```

**Terminal 3: Start Coordinator**
```bash
TWERK_DATASTORE_TYPE=postgres \
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable" \
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
./target/release/twerk run coordinator
```

**Terminal 4: Start Worker(s)**
```bash
# Worker 1
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
TWERK_WORKER_NAME="worker-1" \
./target/release/twerk run worker

# Worker 2 (in another terminal)
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
TWERK_WORKER_NAME="worker-2" \
./target/release/twerk run worker
```

### Submitting Jobs

**Method 1: Using curl**

```bash
# Submit a job
curl -X POST http://localhost:8000/jobs \
  -H "Content-type: text/yaml" \
  --data-binary @examples/hello.yaml

# Response:
{
  "id": "abc123...",
  "state": "SCHEDULED",
  "name": "hello world",
  ...
}

# Check job status
curl http://localhost:8000/jobs/abc123...

# List all jobs
curl http://localhost:8000/jobs

# Get job logs
curl http://localhost:8000/jobs/abc123.../log
```

**Method 2: Using curl with blocking wait**

```bash
# Submit and wait for completion
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-type: text/yaml" \
  --data-binary @examples/hello-shell.yaml
```

**Method 3: Using REST API programmatically**

```python
import requests
import yaml

# Load job definition
with open('hello.yaml') as f:
    job_yaml = f.read()

# Submit job
response = requests.post(
    'http://localhost:8000/jobs',
    headers={'Content-type': 'text/yaml'},
    data=job_yaml
)

job = response.json()
job_id = job['id']

# Poll for completion
import time
while True:
    status = requests.get(f'http://localhost:8000/jobs/{job_id}').json()
    print(f"Job state: {status['state']}")
    
    if status['state'] in ['COMPLETED', 'FAILED', 'CANCELLED']:
        break
    
    time.sleep(2)

print(f"Job finished with state: {status['state']}")
print(f"Result: {status.get('result')}")
```

### Configuration File Reference

**config.toml example:**

```toml
[cli]
banner.mode = "console"

[client]
endpoint = "http://localhost:8000"

[logging]
level = "debug"  # trace, debug, info, warn, error
format = "pretty"  # pretty, json

[broker]
type = "rabbitmq"  # rabbitmq, inmemory

[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"
consumer.timeout = "30m"
durable.queues = false
queue.type = "classic"  # classic, quorum

[datastore]
type = "postgres"  # postgres, inmemory

[datastore.postgres]
dsn = "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable"
max_open_conns = 25
max_idle_conns = 25
conn_max_lifetime = "1h"
conn_max_idle_time = "5m"

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

[worker]
address = "localhost:8001"
name = "Worker"

[worker.queues]
default = 1  # queue_name = concurrency
high-priority = 5

[runtime]
type = "docker"  # docker, podman, shell

[runtime.docker]
config = ""  # path to docker config
privileged = false
image.ttl = "24h"  # cleanup unused images after 24h
```

### Environment Variables

All configuration can be overridden with environment variables:

```bash
# Format: TWERK_<SECTION>_<KEY>=value

TWERK_LOGGING_LEVEL=debug
TWERK_BROKER_TYPE=rabbitmq
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/"
TWERK_DATASTORE_TYPE=postgres
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk"
TWERK_RUNTIME_TYPE=docker
TWERK_WORKER_NAME=worker-1
TWERK_COORDINATOR_ADDRESS=localhost:8000
```

---

## Advanced Features

### Template Expressions

Twerk supports template expressions using Handlebars-like syntax:

```yaml
tasks:
  - var: greeting
    run: echo -n "hello" > $TWERK_OUTPUT
  
  - name: use template
    env:
      MESSAGE: "{{ tasks.greeting }}, world!"
      INPUT: "{{ inputs.myInput }}"
      SECRET: "{{ secrets.apiKey }}"
    run: echo "$MESSAGE"
```

**Available template functions:**

- `{{ inputs.<key> }}` - Access job inputs
- `{{ tasks.<var> }}` - Access previous task outputs
- `{{ secrets.<key> }}` - Access secrets (redacted in logs)
- `{{ job.id }}` - Current job ID
- `{{ sequence start end }}` - Generate number sequence
- `{{ fromJSON string }}` - Parse JSON string
- `{{ eq arg1 arg2 }}` - Equality comparison
- `{{ item.index }}` - Index in each loop
- `{{ item.value }}` - Value in each loop

### Secrets Management

```yaml
name: secure job
secrets:
  apiKey: "super-secret-key"
  dbPassword: "{{ secrets.dbPassword }}"
tasks:
  - name: use secrets
    image: ubuntu:mantic
    env:
      API_KEY: "{{ secrets.apiKey }}"
      DB_PASS: "{{ secrets.dbPassword }}"
    run: |
      # These values are redacted in logs
      curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

### Scheduled Jobs (Cron)

```bash
# Create a scheduled job
curl -X POST http://localhost:8000/scheduled-jobs \
  -H "Content-type: application/json" \
  -d '{
    "name": "daily backup",
    "cron": "0 2 * * *",
    "tasks": [
      {
        "name": "backup database",
        "image": "postgres:15",
        "run": "pg_dump > /backup.sql"
      }
    ]
  }'

# Pause scheduled job
curl -X POST http://localhost:8000/scheduled-jobs/{id}/pause

# Resume scheduled job
curl -X POST http://localhost:8000/scheduled-jobs/{id}/resume
```

### Health Checks

```bash
# Check coordinator health
./target/release/twerk health

# Or via API
curl http://localhost:8000/health

# Response:
{
  "status": "HEALTHY",
  "components": {
    "broker": "HEALTHY",
    "datastore": "HEALTHY"
  }
}
```

### Queue Management

```yaml
# Define queues with priorities
[worker.queues]
default = 1        # concurrency 1
high-priority = 5  # concurrency 5
low-priority = 2   # concurrency 2
```

```yaml
# Use specific queue in job
name: high priority job
tasks:
  - name: important task
    queue: high-priority
    priority: 1  # 0-4, lower is higher priority
    image: ubuntu:mantic
    run: echo "urgent work"
```

### Webhooks

```yaml
name: job with webhooks
webhooks:
  - url: https://hooks.example.com/job-started
    event: JOB_STARTED
  - url: https://hooks.example.com/job-completed
    event: JOB_COMPLETED
tasks:
  - name: main task
    image: ubuntu:mantic
    run: echo "work"
```

### Auto-Delete Jobs

```yaml
name: temporary job
auto_delete:
  after: "24h"  # Delete job and tasks after 24 hours
tasks:
  - name: temporary work
    image: ubuntu:mantic
    run: echo "this will be cleaned up"
```

---

## Monitoring & Observability

### Metrics Endpoint

```bash
# Prometheus metrics
curl http://localhost:8000/metrics
```

### Log Streaming

```bash
# Fetch task logs
curl http://localhost:8000/tasks/{task-id}/log
```

### Job Progress

```yaml
tasks:
  - name: long task
    image: ubuntu:mantic
    run: |
      for i in {1..100}; do
        echo "Progress: $i%"
        echo $i > $TWERK_PROGRESS
        sleep 1
      done
```

```bash
# Check progress
curl http://localhost:8000/jobs/{job-id}
# Response includes "progress": 0.75
```

---

## Troubleshooting

### Common Issues

**1. Container image pull failures**
```bash
# Check Docker is running
docker ps

# Pull image manually
docker pull ubuntu:mantic

# Configure registry credentials
# In config.toml:
[runtime.docker]
config = "/path/to/.docker/config.json"
```

**2. Task timeout issues**
```yaml
# Increase timeout
tasks:
  - name: slow task
    timeout: 1h  # Increase from default
    run: long-running-process
```

**3. Out of memory**
```yaml
# Increase memory limit
tasks:
  - name: memory intensive
    limits:
      memory: "8Gi"  # Increase memory
    run: memory-hungry-process
```

**4. RabbitMQ connection issues**
```bash
# Check RabbitMQ status
sudo rabbitmqctl status

# Check logs
sudo journalctl -u rabbitmq-server

# Restart RabbitMQ
sudo systemctl restart rabbitmq-server
```

**5. PostgreSQL connection issues**
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Test connection
psql -h localhost -U twerk -d twerk

# Check logs
sudo tail -f /var/log/postgresql/postgresql-*.log
```

### Debug Mode

```bash
# Enable debug logging
TWERK_LOGGING_LEVEL=debug ./target/release/twerk run standalone

# Enable trace logging (very verbose)
TWERK_LOGGING_LEVEL=trace ./target/release/twerk run standalone
```

---

## Best Practices

### 1. Task Design

- **Keep tasks small and focused** - Each task should do one thing well
- **Use meaningful task names** - Helps with debugging and monitoring
- **Capture outputs with `var`** - Enables data flow between tasks
- **Set appropriate timeouts** - Prevent hung tasks
- **Use retries for transient failures** - But set reasonable limits

### 2. Resource Management

- **Set resource limits** - Prevent runaway tasks
- **Use appropriate queue priorities** - Critical work first
- **Clean up with auto-delete** - Don't accumulate old jobs
- **Monitor resource usage** - Use `/metrics` endpoint

### 3. Security

- **Use secrets for sensitive data** - Never hardcode credentials
- **Limit network access** - Use networks carefully
- **Run with least privilege** - Avoid privileged mode
- **Scan container images** - Use trusted base images

### 4. Scalability

- **Use parallel tasks** - Speed up independent work
- **Scale workers horizontally** - Add more workers for throughput
- **Use appropriate queues** - Separate workloads
- **Monitor broker performance** - RabbitMQ management UI

### 5. Observability

- **Add meaningful tags** - Group and filter jobs
- **Use webhooks** - Integrate with external systems
- **Stream logs for debugging** - Real-time visibility
- **Track progress** - Update `$TWERK_PROGRESS`

---

## API Reference

### Jobs API

```bash
# Create job
POST /jobs
Content-Type: text/yaml

# Get job
GET /jobs/{id}

# List jobs
GET /jobs?state=RUNNING&limit=100

# Cancel job
PUT /jobs/{id}/cancel

# Get job logs
GET /jobs/{id}/log

# Get task logs
GET /tasks/{task-id}/log
```

### Scheduled Jobs API

```bash
# Create scheduled job
POST /scheduled-jobs
Content-Type: application/json

# List scheduled jobs
GET /scheduled-jobs

# Pause scheduled job
POST /scheduled-jobs/{id}/pause

# Resume scheduled job
POST /scheduled-jobs/{id}/resume

# Delete scheduled job
DELETE /scheduled-jobs/{id}
```

### Health API

```bash
# Health check
GET /health

# Readiness check
GET /ready
```

### Metrics API

```bash
# Prometheus metrics
GET /metrics
```

---

## Complete Example Workflows

### CI/CD Pipeline

```yaml
name: ci-cd-pipeline
description: Full CI/CD pipeline for application deployment
inputs:
  branch: "main"
  commit: "abc123"
secrets:
  dockerPassword: "registry-password"
  awsAccessKey: "AKIAIOSFODNN7EXAMPLE"
  awsSecretKey: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
tags:
  - ci
  - production
defaults:
  timeout: 30m
  retry:
    limit: 2
tasks:
  - name: checkout code
    var: codePath
    image: alpine/git:latest
    run: |
      git clone -b {{ inputs.branch }} https://github.com/myorg/myapp.git /app
      cd /app
      git checkout {{ inputs.commit }}
      echo -n "/app" > $TWERK_OUTPUT
  
  - name: run linting
    image: node:18
    workdir: "{{ tasks.codePath }}"
    run: |
      npm ci
      npm run lint
  
  - name: run unit tests
    image: node:18
    workdir: "{{ tasks.codePath }}"
    run: |
      npm run test:unit
  
  - name: run integration tests
    parallel:
      tasks:
        - name: api tests
          image: node:18
          workdir: "{{ tasks.codePath }}"
          run: npm run test:api
        
        - name: e2e tests
          image: cypress/included:latest
          workdir: "{{ tasks.codePath }}"
          run: npm run test:e2e
  
  - name: build docker image
    var: imageTag
    image: docker:latest
    env:
      DOCKER_PASSWORD: "{{ secrets.dockerPassword }}"
    run: |
      cd {{ tasks.codePath }}
      docker build -t myregistry.com/myapp:{{ inputs.commit }} .
      echo $DOCKER_PASSWORD | docker login -u myuser --password-stdin myregistry.com
      docker push myregistry.com/myapp:{{ inputs.commit }}
      echo -n "myregistry.com/myapp:{{ inputs.commit }}" > $TWERK_OUTPUT
    mounts:
      - type: bind
        source: /var/run/docker.sock
        target: /var/run/docker.sock
  
  - name: deploy to staging
    if: "{{ eq inputs.branch \"develop\" }}"
    image: amazon/aws-cli:2.13.10
    env:
      AWS_ACCESS_KEY_ID: "{{ secrets.awsAccessKey }}"
      AWS_SECRET_ACCESS_KEY: "{{ secrets.awsSecretKey }}"
      IMAGE_TAG: "{{ tasks.imageTag }}"
    run: |
      aws ecs update-service --cluster staging --service myapp --force-new-deployment
  
  - name: deploy to production
    if: "{{ eq inputs.branch \"main\" }}"
    image: amazon/aws-cli:2.13.10
    env:
      AWS_ACCESS_KEY_ID: "{{ secrets.awsAccessKey }}"
      AWS_SECRET_ACCESS_KEY: "{{ secrets.awsSecretKey }}"
      IMAGE_TAG: "{{ tasks.imageTag }}"
    pre:
      - name: create backup
        image: amazon/aws-cli:2.13.10
        run: |
          # Backup current production state
          aws s3 sync s3://myapp-prod s3://myapp-prod-backup-$(date +%s)
    run: |
      # Blue-green deployment
      aws ecs update-service --cluster production --service myapp-blue --force-new-deployment
      # Wait for health checks
      sleep 300
      # Switch traffic
      aws elbv2 modify-listener --listener-arn $LISTENER_ARN --default-actions Type=forward,TargetGroupArn=$BLUE_TG
    post:
      - name: notify team
        image: curlimages/curl:latest
        run: |
          curl -X POST $SLACK_WEBHOOK -d '{"text":"Production deployment completed for {{ inputs.commit }}"}'
```

---

## Summary

Twerk provides a powerful, flexible, and scalable task execution system with:

- **Multiple execution modes** - Standalone, distributed coordinator/worker
- **Flexible task types** - Sequential, parallel, iterator, sub-jobs
- **Production-ready features** - Retries, timeouts, resource limits, secrets
- **Enterprise capabilities** - Scheduling, webhooks, auto-cleanup, monitoring
- **Easy to use** - YAML job definitions, REST API, CLI tools

For more information:
- [GitHub Repository](https://github.com/runabol/twerk)
- [API Documentation](website/src/rest-api.md)
- [Examples Directory](examples/)
- [Configuration Guide](website/src/configuration.md)
