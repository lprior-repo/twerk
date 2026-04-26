# Type Safety Audit: Comprehensive Findings and Refactoring Plan

## Executive Summary

This audit examined the Twerk codebase (327 Rust files) for type safety opportunities. The codebase has **excellent foundations** with well-implemented ID newtypes (`JobId`, `TaskId`, `UserId`, etc.) and proper state enums (`JobState`, `TaskState`). However, significant opportunities exist for improving type safety across the domain.

**Key Findings:**
- ✅ IDs are well-typed (JobId, TaskId, NodeId, etc. are newtypes)
- ✅ State machines are properly implemented as enums
- ❌ Many domain quantities use raw primitives (i64, u16, f64)
- ❌ URLs, hostnames, and credentials use raw Strings
- ❌ HashMap<String, String> used extensively where typed alternatives exist
- ❌ Cron expressions, durations, and counts lack validation

---

## Category 1: Unvalidated Strings That Need Newtypes

### HIGH PRIORITY

#### 1.1 URLs
**Location:** `crates/twerk-core/src/webhook.rs:89`
```rust
// BAD
pub url: Option<String>
```
**Recommendation:** Create `Url` or `WebhookUrl` newtype with RFC 3986 validation
```rust
#[derive(Debug)]
pub struct WebhookUrl(String);

impl WebhookUrl {
    pub fn new(url: impl Into<String>) -> Result<Self, InvalidUrl>;
    pub fn as_str(&self) -> &str;
}
```
**Impact:** Prevents invalid webhook URLs from being constructed; enforces URL format at the boundary

#### 1.2 Hostnames
**Location:** `crates/twerk-core/src/node.rs:55`
```rust
// BAD
pub hostname: Option<String>
```
**Recommendation:** Create `Hostname` newtype with DNS validation
```rust
#[derive(Debug)]
pub struct Hostname(String);

impl Hostname {
    pub fn new(h: impl Into<String>) -> Result<Self, InvalidHostname>;
}
```
**Impact:** Ensures valid hostname format; prevents injection attacks

#### 1.3 Cron Expressions
**Location:** `crates/twerk-core/src/job.rs:305, 339, 405`
```rust
// BAD
pub cron: Option<String>
```
**Recommendation:** Create `CronExpression` newtype with cron parsing
```rust
#[derive(Debug)]
pub struct CronExpression(String);

impl CronExpression {
    pub fn new(expr: impl Into<String>) -> Result<Self, InvalidCron>;
    // Could use cron::Schedule internally
}
```
**Impact:** Prevents invalid cron schedules; provides compile-time safety

#### 1.4 Image Names
**Location:** `crates/twerk-core/src/task.rs:210`
```rust
// BAD
pub image: Option<String>
```
**Recommendation:** Create `ImageName` newtype with Docker image validation
```rust
#[derive(Debug)]
pub struct ImageName(String);

impl ImageName {
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidImageName>;
}
```
**Impact:** Ensures valid Docker image format; prevents runtime errors

#### 1.5 Queue Names
**Location:** `crates/twerk-core/src/job.rs:262 (JobDefaults.queue)`, `crates/twerk-core/src/task.rs:222`
```rust
// BAD
pub queue: Option<String>
```
**Recommendation:** Create `QueueName` newtype
```rust
#[derive(Debug)]
pub struct QueueName(String);

impl QueueName {
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidQueueName>;
}
```
**Impact:** Prevents invalid queue references

---

### MEDIUM PRIORITY

#### 1.6 Container Names
**Location:** Various Docker/Podman infrastructure files
**Recommendation:** Create `ContainerName` newtype with Docker naming validation
```rust
#[derive(Debug)]
pub struct ContainerName(String);
```

#### 1.7 Tags
**Location:** `crates/twerk-core/src/job.rs:238`, `crates/twerk-core/src/task.rs:279`
```rust
pub tags: Option<Vec<String>>
```
**Recommendation:** Create `TagName` newtype with length/character validation
```rust
#[derive(Debug)]
pub struct TagName(String);

pub type Tags = Vec<TagName>;
```

#### 1.8 Version Strings
**Location:** `crates/twerk-core/src/node.rs:61`
```rust
pub version: Option<String>
```
**Recommendation:** Create `Version` newtype (semver) or keep as String if arbitrary
```rust
#[derive(Debug)]
pub struct Version(String); // or use semver::Version
```

---

## Category 2: Primitive Quantities That Need Validation

### HIGH PRIORITY

#### 2.1 Port Numbers
**Location:** 
- `crates/twerk-core/src/task.rs:520` (Probe.port: i64)
- `crates/twerk-core/src/node.rs:57` (Node.port: i64)
- `crates/twerk-infrastructure/src/runtime/docker/container/probe.rs:port: u16`

```rust
// BAD
pub port: i64  // Could be 0, negative, or > 65535
```
**Recommendation:** Create `Port` newtype with range validation
```rust
#[derive(Debug, Clone, Copy)]
pub struct Port(u16);

impl Port {
    pub fn new(p: u16) -> Self { Self(p) }
    pub fn as_u16(&self) -> u16 { self.0 }
}

// Or with validation:
impl Port {
    pub fn new(p: u16) -> Result<Self, InvalidPort> {
        if p == 0 { return Err(InvalidPort); }
        Ok(Self(p))
    }
}
```
**Impact:** Prevents invalid port numbers; makes intent clear

#### 2.2 Retry Limits
**Location:** `crates/twerk-core/src/task.rs:484` (TaskRetry.limit: i64)
```rust
// BAD
pub limit: i64  // Could be negative
```
**Recommendation:** Create `RetryLimit` newtype
```rust
#[derive(Debug, Clone, Copy)]
pub struct RetryLimit(u32);

impl RetryLimit {
    pub fn new(l: u32) -> Self { Self(l) }
    pub fn as_u32(&self) -> u32 { self.0 }
}
```
**Impact:** Ensures non-negative retry counts

#### 2.3 Task Counts and Positions
**Location:** 
- `crates/twerk-core/src/job.rs:262` (task_count: i64)
- `crates/twerk-core/src/job.rs:373` (task_count: i64)
- `crates/twerk-core/src/job.rs:256` (position: i64)
- `crates/twerk-core/src/task.rs:175` (position: i64)
- `crates/twerk-core/src/node.rs:59` (task_count: i64)

```rust
// BAD
pub task_count: i64
pub position: i64
```
**Recommendation:** Create `TaskCount` and `TaskPosition` newtypes
```rust
#[derive(Debug, Clone, Copy)]
pub struct TaskCount(u32);

#[derive(Debug, Clone, Copy)]
pub struct TaskPosition(i64); // Can be negative for ordering
```
**Impact:** Makes intent clear; prevents confusion between counts and positions

#### 2.4 Retry Attempts
**Location:** `crates/twerk-core/src/task.rs:224` (Task.retry.attempts: i64)
```rust
pub attempts: i64
```
**Recommendation:** Create `RetryAttempt` newtype
```rust
#[derive(Debug, Clone, Copy)]
pub struct RetryAttempt(u32);
```

#### 2.5 Progress Percentage
**Location:** 
- `crates/twerk-core/src/job.rs:282` (progress: f64)
- `crates/twerk-core/src/task.rs:288` (progress: f64)

```rust
// BAD
pub progress: f64  // Could be < 0 or > 100
```
**Recommendation:** Create `Progress` newtype with range validation
```rust
#[derive(Debug, Clone, Copy)]
pub struct Progress(f64); // 0.0 to 100.0

impl Progress {
    pub fn new(p: f64) -> Result<Self, InvalidProgress> {
        if !(0.0..=100.0).contains(&p) {
            return Err(InvalidProgress);
        }
        Ok(Self(p))
    }
}
```
**Impact:** Ensures progress is always valid

#### 2.6 Priority Values
**Location:** 
- `crates/twerk-core/src/job.rs:267` (JobDefaults.priority: i64)
- `crates/twerk-core/src/task.rs:286` (Task.priority: i64)

```rust
// BAD
pub priority: i64  // No bounds defined
```
**Recommendation:** Create `Priority` newtype
```rust
#[derive(Debug, Clone, Copy)]
pub struct Priority(i32);

// Or use the existing twerk_common::Priority if it exists
```

---

### MEDIUM PRIORITY

#### 2.7 Duration/Timeout Strings
**Location:** 
- `crates/twak-core/src/job.rs:264` (JobDefaults.timeout: Option<String>)
- `crates/twerk-core/src/task.rs:257` (Task.timeout: Option<String>)
- `crates/twerk-core/src/task.rs:522` (Probe.timeout: Option<String>)

```rust
// BAD
pub timeout: Option<String>
```
**Recommendation:** Create `Duration` newtype using time::Duration or parse at boundary
```rust
pub timeout: Option<Duration>  // Using time::Duration
```
**Impact:** Eliminates string parsing errors; consistent duration handling

#### 2.8 Parallel Task Completions/Concurrency
**Location:** `crates/twerk-core/src/task.rs:450` (ParallelTask.completions: i64), `crates/twerk-core/src/task.rs:476` (EachTask.concurrency: i64)

```rust
pub completions: i64
pub concurrency: i64
```
**Recommendation:** Create `ConcurrentTasks` or `CompletionCount` newtype
```rust
#[derive(Debug, Clone, Copy)]
pub struct ConcurrentTasks(u32);
```

#### 2.9 EachTask Index/Size
**Location:** `crates/twerk-core/src/task.rs:473` (EachTask.index: i64), `crates/twerk-core/src/task.rs:470` (EachTask.size: i64)

```rust
pub index: i64
pub size: i64
```
**Recommendation:** Create `IterationIndex` and `IterationCount` newtypes

---

## Category 3: HashMap<String, T> Patterns

### HIGH PRIORITY

#### 3.1 Job/Task Inputs
**Location:** 
- `crates/twerk-core/src/job.rs:258` (Job.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/job.rs:309` (ScheduledJob.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/job.rs:354` (JobSummary.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/job.rs:394` (ScheduledJobSummary.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/task.rs:217` (Task.env: Option<HashMap<String, String>>)
- `crates/twerk-core/src/task.rs:220` (Task.files: Option<HashMap<String, String>>)
- `crates/twerk-core/src/task.rs:223` (Task.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/task.rs:424` (SubJobTask.inputs: Option<HashMap<String, String>>)
- `crates/twerk-core/src/subjob.rs:427` (SubJobTask.secrets: Option<HashMap<String, String>>)

```rust
// BAD - No schema enforcement
pub inputs: Option<HashMap<String, String>>
```
**Recommendation:** Create typed alternatives based on context:
```rust
pub struct JobInputs(HashMap<String, String>);
pub struct TaskEnv(HashMap<String, String>);
pub struct TaskFiles(HashMap<String, String>);
pub struct JobSecrets(HashMap<String, String>);
```
**Impact:** Makes it clear what kind of data is stored; prevents mixing inputs with env vars

#### 3.2 Job Context
**Location:** `crates/twerk-core/src/job.rs:411-420` (JobContext)
```rust
pub struct JobContext {
    pub job: Option<HashMap<String, String>>,
    pub inputs: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
    pub tasks: Option<HashMap<String, String>>,
}
```
**Recommendation:** Create typed context components
```rust
pub struct JobContext {
    pub job_vars: Option<JobVars>,
    pub inputs: Option<JobInputs>,
    pub secrets: Option<JobSecrets>,
    pub task_vars: Option<TaskVars>,
}
```

#### 3.3 Webhook Headers
**Location:** `crates/twerk-core/src/webhook.rs:92`
```rust
pub headers: Option<HashMap<String, String>>
```
**Recommendation:** Create `Headers` type
```rust
pub type Headers = HashMap<String, String>;
// Or more strictly:
#[derive(Debug, Default)]
pub struct Headers(HashMap<String, String>);
```

#### 3.4 Environment Variables
**Location:** 
- `crates/twerk-app/src/engine/coordinator/hostenv.rs` (multiple HashMap<String, String>)
- `crates/twerk-common/src/conf/lookup.rs` (string_map, int_map, bool_map functions)
- `crates/twerk-common/src/conf/types.rs` (string_map_for_key, etc.)

```rust
vars: HashMap<String, String>
```
**Recommendation:** Create `EnvironmentVars` type
```rust
#[derive(Debug, Default)]
pub struct EnvironmentVars(HashMap<String, String>);

impl EnvironmentVars {
    pub fn get(&self, key: &str) -> Option<&str>;
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>);
}
```

---

### MEDIUM PRIORITY

#### 3.5 Config Maps
**Location:** `crates/twerk-common/src/conf/lookup.rs`, `crates/twerk-common/src/conf/types.rs`

```rust
pub fn string_map(key: &str) -> HashMap<String, String>
```
**Recommendation:** Create strongly-typed config accessors
```rust
pub struct ConfigMap(HashMap<String, serde_json::Value>);

impl ConfigMap {
    pub fn string(&self, key: &str) -> Option<&str>;
    pub fn int(&self, key: &str) -> Option<i64>;
    pub fn bool(&self, key: &str) -> Option<bool>;
}
```

---

## Category 4: Credentials and Sensitive Data

### HIGH PRIORITY

#### 4.1 Passwords
**Location:** 
- `crates/twerk-core/src/user.rs:24` (User.password: Option<String>)
- `crates/twerk-core/src/user.rs:22` (User.password_hash: Option<String>)
- `crates/twerk-core/src/task.rs:509` (Registry.password: Option<String>)
- `crates/twerk-infrastructure/src/runtime/docker/auth/auth_config.rs:28`
- `crates/twerk-infrastructure/src/runtime/docker/twerk.rs:94`

```rust
// BAD - Passwords as plain Strings
pub password: Option<String>
```
**Recommendation:** Create `Password` newtype with zeroization
```rust
use zeroize::Zeroizing;

#[derive(Debug, Clone)]
pub struct Password(Zeroizing<String>);

impl Password {
    pub fn new(pw: impl Into<String>) -> Self {
        Self(Zeroizing::new(pw.into()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl Drop for Password {
    fn drop(&mut self) {
        // Zeroize the memory
        self.0.clear();
    }
}
```
**Impact:** Ensures passwords are zeroed from memory; makes sensitive data explicit

#### 4.2 API Keys and Secrets
**Location:** 
- `crates/twerk-core/src/redact.rs` (secrets: &HashMap<String, String>)
- `crates/twerk-core/src/job.rs:280` (Job.secrets: Option<HashMap<String, String>>)
- `crates/twerk-core/src/task.rs:226` (Task.secrets: Option<HashMap<String, String>>)

```rust
secrets: Option<HashMap<String, String>>
```
**Recommendation:** Create `Secrets` type with redaction support
```rust
#[derive(Debug, Clone)]
pub struct Secrets(HashMap<String, Zeroizing<String>>);

impl Secrets {
    pub fn get(&self, key: &str) -> Option<&str>;
    // Redact all values in Debug output
}
```

#### 4.3 Registry Credentials
**Location:** `crates/twerk-core/src/task.rs:503-510` (Registry struct)
```rust
pub struct Registry {
    pub username: Option<String>,
    pub password: Option<String>,
}
```
**Recommendation:** Create `RegistryAuth` with typed username/password
```rust
pub struct RegistryAuth {
    pub username: Option<String>,
    pub password: Password,  // Zeroizing
}
```

---

## Category 5: State Machine and Bool Flag Issues

### LOW PRIORITY (Mostly Already Fixed)

The codebase has **excellent state machine implementation**:
- ✅ `JobState` enum with proper transitions
- ✅ `TaskState` enum with proper transitions
- ✅ `ScheduledJobState` enum
- ✅ `NodeStatus` enum

**Minimal bool flags remaining:**
- `TaskRetry.attempts` and `limit` should be validated (see Category 2)
- `SubJobTask.detached: bool` - acceptable for binary state
- `User.disabled: bool` - acceptable for enabled/disabled flag

---

## Category 6: Command and Shell Patterns

### MEDIUM PRIORITY

#### 6.1 Command Vectors
**Location:** 
- `crates/twerk-core/src/task.rs:201` (Task.cmd: Option<Vec<String>>)
- `crates/twerk-core/src/task.rs:204` (Task.entrypoint: Option<Vec<String>>)
- `crates/twerk-app/src/engine/worker/runtime_adapter.rs:28` (shell_cmd: Vec<String>)
- `crates/twerk-app/src/engine/worker/shell.rs:108` (cmd: Vec<String>)

```rust
pub cmd: Option<Vec<String>>
```
**Recommendation:** Create typed command structures
```rust
#[derive(Debug, Clone)]
pub struct Command(Vec<String>);

#[derive(Debug, Clone)]
pub struct Entrypoint(Vec<String>);

// Or use more structured approach:
pub enum CommandSpec {
    Shell(Vec<String>),
    Exec(Vec<String>),  // Direct exec, no shell
}
```
**Impact:** Makes command execution intent clear; enables validation

---

## Category 7: Database Record Types

### MEDIUM PRIORITY

#### 7.1 Database String IDs
**Location:** `crates/twerk-infrastructure/src/datastore/postgres/records/job.rs:18`
```rust
pub struct JobRecord {
    pub id: String,  // Raw string from DB
    pub created_by: String,
    pub parent_id: Option<String>,
    pub scheduled_job_id: Option<String>,
}
```
**Recommendation:** Keep as strings in records (DB layer), but convert at boundary
```rust
// This is acceptable - records are DB-specific
// Conversion happens in JobRecordExt::to_job()
```
**Note:** The conversion layer already uses `JobId::new(self.id.clone())?` which is good

---

## Category 8: Web Layer Types

### MEDIUM PRIORITY

#### 8.1 Auth Request Types
**Location:** `crates/twerk-web/src/api/handlers/system.rs:51-52`
```rust
pub username: Option<String>,
pub password: Option<String>,
```
**Recommendation:** Use typed auth request
```rust
#[derive(Debug)]
pub struct LoginRequest {
    pub username: Username,
    pub password: Password,
}
```

#### 8.2 Engine Configuration
**Location:** `crates/twerk-app/src/engine/types.rs:151-156`
```rust
pub struct Config {
    pub engine_id: Option<String>,
    pub endpoints: HashMap<String, EndpointHandler>,
}
```
**Recommendation:** Create typed engine ID
```rust
pub struct Config {
    pub engine_id: Option<EngineId>,
    pub endpoints: HashMap<String, EndpointHandler>,
}
```

---

## Priority Summary

### HIGH PRIORITY (Do First)
1. **URL/Hostname types** - webhook.url, node.hostname, cron expressions
2. **Port validation** - probe.port, node.port
3. **Credential zeroization** - Password newtype with zeroization
4. **Retry/progress counts** - TaskRetry, progress percentages
5. **Secrets handling** - typed secrets with redaction

### MEDIUM PRIORITY (Do Next)
1. **HashMap types** - inputs, env, files, secrets typed wrappers
2. **Duration/timeout types** - replace String with Duration
3. **Command types** - typed command/entrypoint vectors
4. **Image/queue types** - validate image names, queue names
5. **Auth request types** - typed login requests

### LOW PRIORITY (Nice to Have)
1. **Version strings** - semver validation
2. **Tag types** - validate tag format
3. **Misc counts** - task_count, position, concurrency

---

## Implementation Strategy

### Phase 1: Core Domain Types (Weeks 1-2)
1. Create `Url`/`WebhookUrl` newtype
2. Create `Hostname` newtype
3. Create `Port` newtype
4. Create `CronExpression` newtype
5. Implement in webhook.rs and node.rs

### Phase 2: Credential Safety (Week 3)
1. Create `Password` type with zeroization
2. Create `Secrets` type
3. Update User struct
4. Update Registry struct
5. Add tests for zeroization

### Phase 3: Quantity Types (Week 4)
1. Create `RetryLimit`, `RetryAttempt`
2. Create `Progress` type
3. Create `TaskCount`, `TaskPosition`
4. Update TaskRetry, Job, Task structs

### Phase 4: HashMap Wrappers (Week 5-6)
1. Create `JobInputs`, `TaskEnv`, `TaskFiles`
2. Create `Secrets` wrapper
3. Update all HashMap<String, String> usages
4. Add type-safe accessors

### Phase 5: Infrastructure Cleanup (Week 7-8)
1. Update database record conversions
2. Update Docker/Podman runtime types
3. Update CLI types
4. Update web layer types

---

## Testing Strategy

For each newtype:
1. **Unit tests** - constructor validation, accessor tests
2. **Serialization tests** - JSON roundtrip
3. **Property tests** - proptest for edge cases
4. **Integration tests** - end-to-end with database
5. **Zeroization tests** - for sensitive types

---

## Dependencies to Add

```toml
[dependencies]
zeroize = "1.7"  # For secure credential handling
url = "2.5"      # For URL parsing/validation (optional, can implement custom)
cron = "0.12"    # For cron expression parsing (optional)
semver = "1.0"   # For version strings (optional)
thiserror = "1.0" # Error types (already used)
```

---

## Migration Notes

### Breaking Changes
- Many function signatures will change (String → NewType)
- Database migrations may be needed for some types
- API endpoints may need versioning

### Non-Breaking Patterns
- Newtypes implement `Deref` to `str` for backward compatibility
- `From<String>` and `From<&str>` implementations for easy conversion
- Serialization remains as string (transparent newtype)

### Recommended Approach
1. Create newtypes with minimal breaking changes
2. Add `From` implementations for easy migration
3. Update code incrementally, testing each phase
4. Use clippy to find remaining raw primitive usages

---

## Beads Created for This Audit

### High Priority Beads
- **twerk-vam**: Create Url, Hostname, and CronExpression newtype wrappers (120 min)
- **twerk-3q4**: Create Password and Secrets types with zeroization (180 min)
- **twerk-d7p**: Create Port, RetryLimit, Progress, and Quantity newtypes (150 min)

### Medium Priority Beads
- **twerk-057**: Replace HashMap<String, String> with typed alternatives (240 min)
- **twerk-8rq**: Implement remaining domain types (ImageName, QueueName, Duration, Command, etc.) (200 min)

### Audit Completion
- **twerk-sg5**: Complete type safety audit and create refactoring beads (60 min)

Total estimated effort: 950 minutes (approximately 16 hours)

---

## Success Metrics

- [ ] Zero raw `port: u16` or `port: i64` in domain types
- [ ] All passwords use zeroizing types
- [ ] All URLs validated at construction
- [ ] All cron expressions validated
- [ ] All progress values bounded 0-100
- [ ] All retry counts non-negative
- [ ] HashMap<String, String> replaced with typed alternatives where schema is known
- [ ] All clippy warnings resolved
- [ ] All tests passing
