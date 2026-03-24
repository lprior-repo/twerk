# Contract Specification: Fix middleware gaps

bead_id: twerk-741
bead_title: Fix middleware gaps: context types, expr evaluation, webhook events
phase: 1
updated_at: 2026-03-24T00:00:00Z

## Context

- **Feature**: Fix four middleware gaps identified during Go-to-Rust parity analysis
- **Domain terms**:
  - `Context` - Request context for cancellation, deadlines, and value propagation
  - `Datastore` - Trait for accessing persistent job data
  - `Webhook` - HTTP callback triggered on job/task state changes
  - `EventType` - Enum indicating the kind of event (StateChange, Progress, etc.)
  - `JobSummary` / `TaskSummary` - Aggregated view of job/task state for webhooks
  - `evalexpr` - Expression evaluation library (Rust) vs `expr-lang` (Go)
- **Assumptions**:
  - Go parity is the target: Rust behavior must match Go `tork` middleware semantics
  - The `eval` crate uses `evalexpr` which does not support struct dot-access
  - Job and Node middleware Context types are already proper structs (GAP1 only affects Task)
- **Open questions**:
  - Q1: Should Task Context support `tracing::Span` integration or be a pure custom enum?
  - Q2: Should expr evaluation use a different engine (e.g., `rhai`) that supports dot-access?
  - Q3: Is backward compatibility required for the flattened expr context keys?

## GAP 1: Task Context Type

### Current State
```rust
pub type Context = Arc<std::sync::RwLock<()>>;  // PLACEHOLDER - no real context
```

### Expected State
A proper context type supporting:
- Cancellation propagation (like `std::context::Context`)
- Deadline/timeout support
- Value storage and retrieval

### Contract

**Type Definition**:
```rust
pub enum Context {
    Cancelled,
    DeadlineExceeded,
    Values(HashMap<String, String>),
}
```

**Invariants**:
- `Context` must be `Send + Sync` (used in `Arc<Context>`)
- `Context::Values` variant must not contain `None` keys or values

**Constructors**:
- `Context::cancelled()` -> `Arc<Context>` - Creates a cancelled context
- `Context::deadline_exceeded()` -> `Arc<Context>` - Creates a context where deadline passed
- `Context::with_value(key, value)` -> `Arc<Context>` - Chainable context builder

**Observations**:
- `is_cancelled(&self) -> bool`
- `is_deadline_exceeded(&self) -> bool`
- `get(&self, key: &str) -> Option<&str>`

---

## GAP 5: Task Webhook `getJob` Missing Context Parameter

### Current State
```rust
fn get_job(
    job_id: &str,
    ds: &dyn Datastore,
    cache: &tork_cache::Cache<String, Job>,
) -> Result<Job, TaskMiddlewareError>

pub trait Datastore {
    fn get_job_by_id(&self, job_id: &str) -> Result<Job, TaskMiddlewareError>;
}
```

### Expected State
```rust
fn get_job(
    ctx: &Context,
    job_id: &str,
    ds: &dyn Datastore,
    cache: &tork_cache::Cache<String, Job>,
) -> Result<Job, TaskMiddlewareError>

pub trait Datastore {
    fn get_job_by_id(&self, ctx: &Context, job_id: &str) -> Result<Job, TaskMiddlewareError>;
}
```

### Contract

**Preconditions**:
- `ctx` must not be `Context::Cancelled` or `Context::DeadlineExceeded`
- `job_id` must be non-empty string

**Postconditions**:
- On `Ok(job)`: Job is returned and cached if fetched from datastore
- On `Err(TaskMiddlewareError::JobNotFound(...))`: Job does not exist
- On `Err(TaskMiddlewareError::ContextCancelled)`/`Err(ContextDeadlineExceeded)`: Context was cancelled/deadline exceeded
- Context errors propagate before any I/O

**Error Taxonomy**:
```rust
pub enum TaskMiddlewareError {
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("context cancelled")]
    ContextCancelled,
    #[error("context deadline exceeded")]
    ContextDeadlineExceeded,
    #[error("middleware error: {0}")]
    Middleware(String),
    #[error("datastore error: {0}")]
    Datastore(String),
}
```

---

## GAP 8: Job Webhook Event Matching

### Current State (Bug)
```rust
fn should_fire_webhook(et: EventType, wh: &Webhook) -> bool {
    match et {
        EventType::StateChange => {
            event.is_empty() || event == EVENT_JOB_STATE_CHANGE || event == EVENT_DEFAULT
        }
        EventType::Progress => event.is_empty() || event == EVENT_JOB_PROGRESS,  // BUG: empty matches Progress
        _ => false,
    }
}
```

### Expected State
```rust
fn should_fire_webhook(et: EventType, wh: &Webhook) -> bool {
    match et {
        EventType::StateChange => {
            event.is_empty() || event == EVENT_JOB_STATE_CHANGE || event == EVENT_DEFAULT
        }
        EventType::Progress => event == EVENT_JOB_PROGRESS,  // FIX: empty does NOT match Progress
        _ => false,
    }
}
```

### Contract

**Invariants**:
- Empty or `EVENT_DEFAULT` webhook event MUST only match `EventType::StateChange`
- Empty or `EVENT_DEFAULT` webhook event MUST NOT match `EventType::Progress`
- Explicit `EVENT_JOB_STATE_CHANGE` event only matches `EventType::StateChange`
- Explicit `EVENT_JOB_PROGRESS` event only matches `EventType::Progress`
- `EventType::Read` never triggers any webhook regardless of event setting

**Truth Table**:
| Webhook\Event | StateChange | Progress | Read |
|---------------|-------------|----------|------|
| None/""       | TRUE        | FALSE    | FALSE|
| EVENT_DEFAULT | TRUE        | FALSE    | FALSE|
| EVENT_STATE_CHANGE | TRUE   | FALSE    | FALSE|
| EVENT_PROGRESS     | FALSE| TRUE     | FALSE|

---

## GAP 3 & 4: Expression Evaluation Context

### Current State (Job Webhook - GAP3)
```rust
fn evaluate_condition(expr: &str, summary: &JobSummary) -> Result<bool, String> {
    let mut context = HashMap::new();
    context.insert("job_state".to_string(), serde_json::Value::String(summary.state.clone()));
    context.insert("job_id".to_string(), serde_json::json!(summary.id.as_deref().unwrap_or("")));
    // ... flattened fields
}
```
Expression: `job_state == "COMPLETED"` (flat, NOT `job.State == "COMPLETED"`)

### Current State (Task Webhook - GAP4)
```rust
eval_context.insert("task".to_string(), serde_json::to_value(&summary).unwrap_or(...));
eval_context.insert("job".to_string(), serde_json::to_value(&job_summary).unwrap_or(...));
```
Expression: Uses JSON serialization (loses type info), not struct access.

### Expected State
Either:
1. **Option A (Document)**: Keep flattened context, document that expressions must use flat names
2. **Option B (Rhai)**: Replace `evalexpr` with `rhai` engine that supports dot-access on maps

### Contract (Option A - Flattened Context)

**Invariants**:
- `evaluate_condition` context MUST contain `job_state` key when evaluating job webhook expressions
- `evaluate_condition` context MUST contain `job_id` key
- Task webhook context MUST contain `task` and `job` keys pointing to JSON objects
- All context values MUST be `serde_json::Value`

**Preconditions**:
- `expr` must be a valid evalexpr expression
- `summary` must be non-null

**Postconditions**:
- Returns `Ok(true)` if expression evaluates to boolean true
- Returns `Ok(false)` if expression evaluates to boolean false
- Returns `Err(String)` if expression is invalid or evaluates to non-boolean

**Error Taxonomy**:
```rust
enum EvalError {
    #[error("expression evaluation failed: {0}")]
    EvaluationFailed(String),
    #[error("expression did not evaluate to boolean: {0}")]
    NotBoolean(String),
    #[error("invalid expression syntax: {0}")]
    InvalidSyntax(String),
}
```

**Documented Flat Keys for Job Webhooks**:
| Key       | Type   | Description |
|-----------|--------|-------------|
| `job_state`  | String | Job state (e.g., "PENDING", "COMPLETED", "FAILED") |
| `job_id`     | String | Job ID |
| `job_name`   | String | Job name (optional) |
| `job_error`  | String | Error message (optional) |

**Documented Keys for Task Webhooks**:
| Key   | Type   | Description |
|-------|--------|-------------|
| `task` | Object | Task summary as JSON object |
| `job`  | Object | Job summary as JSON object |

---

## Contract Signatures

### Task Middleware Types
```rust
// GAP 1: Proper context type
pub enum Context { ... }
impl Context {
    pub fn cancelled() -> Arc<Context>
    pub fn deadline_exceeded() -> Arc<Context>
    pub fn with_value(self, key: impl Into<String>, value: impl Into<String>) -> Arc<Context>
    pub fn is_cancelled(&self) -> bool
    pub fn is_deadline_exceeded(&self) -> bool
    pub fn get(&self, key: &str) -> Option<&str>
}

pub type HandlerFunc = Arc<dyn Fn(Arc<Context>, EventType, &mut Task) -> Result<(), TaskMiddlewareError> + Send + Sync>;
pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;
```

### Task Webhook Datastore Trait
```rust
// GAP 5: Add ctx parameter
pub trait Datastore: Send + Sync {
    fn get_job_by_id(&self, ctx: &Context, job_id: &str) -> Result<Job, TaskMiddlewareError>;
}

fn get_job(
    ctx: &Context,
    job_id: &str,
    ds: &dyn Datastore,
    cache: &tork_cache::Cache<String, Job>,
) -> Result<Job, TaskMiddlewareError>;
```

### Job Webhook
```rust
// GAP 8: Fix event matching
fn should_fire_webhook(et: EventType, wh: &Webhook) -> bool {
    // empty/default only matches StateChange, NOT Progress
}
```

### Expression Evaluation
```rust
// GAP 3 & 4: Document flattened context behavior
fn evaluate_condition(expr: &str, summary: &JobSummary) -> Result<bool, String>;
fn evaluate_task_condition(expr: &str, task_summary: &TaskSummary, job_summary: &JobSummary) -> Result<bool, String>;
```

---

## Non-goals

- [ ] Implementing true `std::context::Context` compatibility (custom enum is acceptable for Go parity)
- [ ] Supporting dot-access in expressions (flattened context is acceptable with documentation)
- [ ] Changing the `evalexpr` library (staying with current library but documenting limitations)
- [ ] Modifying Node middleware Context (already has proper struct implementation)
- [ ] Adding new middleware functionality beyond fixing these gaps
