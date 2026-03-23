//! Completed handler for task completion events.
//!
//! Port of Go `internal/coordinator/handlers/completed.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives COMPLETED/SKIPPED tasks
//! 2. Routes to completeSubTask or completeTopLevelTask
//! 3. **completeEachTask**: tracks completions, handles concurrency limits,
//!    dispatches next batch, completes parent when done
//! 4. **completeParallelTask**: tracks completions, completes parent when done
//! 5. **completeTopLevelTask**: updates job progress, creates next task,
//!    evaluates it with job context, marks job as completed when all tasks are done
//!
//! # Task Evaluation
//!
//! After creating the next task, [`evaluate_task`] is called to interpolate
//! template expressions like `{{tasks.result}}` using the job context.
//! If evaluation fails, the task is marked as FAILED.
//!
//! # Known Limitations
//!
//! - The Go `ds.WithTx` transaction pattern is not supported by the current
//!   Rust `Datastore` trait. Operations are performed sequentially with
//!   read-modify-write patterns.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use evalexpr::{eval_with_context, ContextWithMutableVariables, HashMapContext, Value as EvalValue};
use regex::Regex;
use tork::broker::queue;
use tork::job::{Job, JobContext, JOB_STATE_COMPLETED};
use tork::task::{
    EachTask, ParallelTask, Task, TASK_STATE_COMPLETED, TASK_STATE_FAILED, TASK_STATE_PENDING,
    TASK_STATE_RUNNING, TASK_STATE_SCHEDULED, TASK_STATE_SKIPPED,
};
use tork::{Broker, Datastore};

use crate::handlers::HandlerError;

/// Regex to match `{{ expr }}` template patterns.
#[allow(clippy::expect_used)]
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid template regex"));

// ---------------------------------------------------------------------------
// Eval helpers (parity with Go eval.EvaluateTask)
// ---------------------------------------------------------------------------

/// Converts a [`serde_json::Value`] to an [`evalexpr::Value`].
fn json_to_eval_value(json: &serde_json::Value) -> Result<EvalValue, String> {
    match json {
        serde_json::Value::Null => Ok(EvalValue::Empty),
        serde_json::Value::Bool(b) => Ok(EvalValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(EvalValue::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(EvalValue::Float(f))
            } else {
                Err("unsupported number type".into())
            }
        }
        serde_json::Value::String(s) => Ok(EvalValue::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<EvalValue>, String> =
                arr.iter().map(json_to_eval_value).collect();
            Ok(EvalValue::Tuple(values?))
        }
        serde_json::Value::Object(obj) => {
            let pairs: Result<Vec<EvalValue>, String> = obj
                .iter()
                .map(|(k, v)| {
                    let val = json_to_eval_value(v)?;
                    Ok(EvalValue::Tuple(vec![EvalValue::String(k.clone()), val]))
                })
                .collect();
            Ok(EvalValue::Tuple(pairs?))
        }
    }
}

/// Creates an evalexpr context from a serde_json context map.
fn create_eval_context(
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMapContext, String> {
    let mut ctx = HashMapContext::new();
    for (key, value) in context {
        let eval_value = json_to_eval_value(value).unwrap_or(EvalValue::Empty);
        ctx.set_value(key.clone(), eval_value)
            .map_err(|e| format!("{key}: {e}"))?;
    }
    Ok(ctx)
}

/// Recursively flatten a JSON value into dot-separated key-value pairs.
fn flatten_json_value(
    prefix: &str,
    value: &serde_json::Value,
) -> Vec<(String, EvalValue)> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .flat_map(|(k, v)| {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json_value(&key, v)
            })
            .collect(),
        other => {
            let eval_val = json_to_eval_value(other).unwrap_or(EvalValue::Empty);
            vec![(prefix.to_string(), eval_val)]
        }
    }
}

/// Evaluates a single expression string with the given context.
fn evaluate_expr(
    expr_str: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    let sanitized = expr_str.trim();
    if sanitized.is_empty() {
        return Ok(String::new());
    }
    let ctx = create_eval_context(context)?;
    let result = eval_with_context(sanitized, &ctx)
        .map_err(|e| format!("error evaluating '{sanitized}': {e}"))?;
    Ok(match &result {
        EvalValue::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// Evaluates a template string, replacing all `{{ expression }}` patterns.
fn evaluate_template(
    template: &str,
    context: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    if template.is_empty() {
        return Ok(String::new());
    }

    let matches: Vec<_> = TEMPLATE_REGEX.find_iter(template).collect();

    if matches.is_empty() {
        return Ok(template.to_string());
    }

    let (result, last_end) = matches.iter().try_fold(
        (String::new(), 0usize),
        |(buf, loc), m| {
            let start_tag = m.start();
            let prefix = if loc < start_tag {
                template[loc..start_tag].to_string()
            } else {
                String::new()
            };
            let caps = TEMPLATE_REGEX
                .captures(m.as_str())
                .ok_or_else(|| format!("no capture in match: {}", m.as_str()))?;
            let expr_str = &caps[1];
            let replacement = evaluate_expr(expr_str, context)?;
            Ok::<(String, usize), String>((buf + &prefix + &replacement, m.end()))
        },
    )?;

    let tail = &template[last_end..];
    Ok(result + tail)
}

/// Evaluate an optional string field through the template engine.
fn eval_field(
    field: &Option<String>,
    context: &HashMap<String, serde_json::Value>,
) -> Result<Option<String>, String> {
    field
        .as_ref()
        .map_or(Ok(None), |s| evaluate_template(s, context).map(Some))
}

/// Evaluate a map of string → string through the template engine.
fn eval_map(
    map: &Option<HashMap<String, String>>,
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, String>, String> {
    match map.as_ref() {
        Some(m) => m
            .iter()
            .map(|(k, v)| {
                let result = evaluate_template(v, context)?;
                Ok((k.clone(), result))
            })
            .collect(),
        None => Ok(HashMap::new()),
    }
}

/// Recursively evaluate a list of tasks.
fn eval_tasks(
    tasks: &Option<Vec<Task>>,
    context: &HashMap<String, serde_json::Value>,
) -> Result<Option<Vec<Task>>, String> {
    tasks
        .as_ref()
        .map(|ts| {
            ts.iter()
                .map(|t| evaluate_task(t, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
}

/// Evaluates all template expressions in a task's fields.
///
/// Returns a new `Task` with all evaluated values.
/// Parity with Go `EvaluateTask(t *Task, c map[string]any) error`.
#[allow(clippy::too_many_lines)]
fn evaluate_task(
    task: &Task,
    context: &HashMap<String, serde_json::Value>,
) -> Result<Task, String> {
    let name = eval_field(&task.name, context)?;
    let var = eval_field(&task.var, context)?;
    let image = eval_field(&task.image, context)?;
    let run = eval_field(&task.run, context)?;
    let queue = eval_field(&task.queue, context)?;
    let r#if = eval_field(&task.r#if, context)?;
    let description = eval_field(&task.description, context)?;
    let workdir = eval_field(&task.workdir, context)?;
    let timeout = eval_field(&task.timeout, context)?;
    let gpus = eval_field(&task.gpus, context)?;

    let env = eval_map(&task.env, context)?;
    let files = eval_map(&task.files, context)?;

    let pre = eval_tasks(&task.pre, context)?;
    let post = eval_tasks(&task.post, context)?;
    let sidecars = eval_tasks(&task.sidecars, context)?;

    // Evaluate cmd array
    let cmd = task
        .cmd
        .as_ref()
        .map(|cmds| {
            cmds.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Evaluate entrypoint array
    let entrypoint = task
        .entrypoint
        .as_ref()
        .map(|eps| {
            eps.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // Evaluate parallel tasks
    let parallel = task.parallel.as_ref().map(|par| {
        let tasks = eval_tasks(&par.tasks, context)?;
        Ok::<tork::task::ParallelTask, String>(tork::task::ParallelTask {
            tasks,
            completions: par.completions,
        })
    }).transpose()?;

    // Evaluate each tasks
    let each = task.each.as_ref().map(|each| {
        let var = eval_field(&each.var, context)?;
        let list = eval_field(&each.list, context)?;
        let inner_task = each
            .task
            .as_ref()
            .map(|t| evaluate_task(t, context))
            .transpose()?;
        Ok::<tork::task::EachTask, String>(tork::task::EachTask {
            var,
            list,
            task: inner_task.map(Box::new),
            size: each.size,
            completions: each.completions,
            concurrency: each.concurrency,
            index: each.index,
        })
    }).transpose()?;

    // Evaluate subjob tasks
    let subjob = task.subjob.as_ref().map(|sj| {
        let subjob_name = eval_field(&sj.name, context)?;
        let inputs = eval_map(&sj.inputs, context)?;
        let secrets = eval_map(&sj.secrets, context)?;
        let subjob_tasks = eval_tasks(&sj.tasks, context)?;
        Ok::<tork::task::SubJobTask, String>(tork::task::SubJobTask {
            id: sj.id.clone(),
            name: subjob_name,
            description: sj.description.clone(),
            tasks: subjob_tasks,
            inputs: if inputs.is_empty() { None } else { Some(inputs) },
            secrets: if secrets.is_empty() { None } else { Some(secrets) },
            auto_delete: sj.auto_delete.clone(),
            output: sj.output.clone(),
            detached: sj.detached,
            webhooks: sj.webhooks.clone(),
        })
    }).transpose()?;

    Ok(Task {
        id: task.id.clone(),
        job_id: task.job_id.clone(),
        parent_id: task.parent_id.clone(),
        position: task.position,
        name,
        description,
        state: task.state.clone(),
        created_at: task.created_at,
        scheduled_at: task.scheduled_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        failed_at: task.failed_at,
        cmd,
        entrypoint,
        run,
        image,
        registry: task.registry.clone(),
        env: if env.is_empty() { None } else { Some(env) },
        files: if files.is_empty() { None } else { Some(files) },
        queue,
        redelivered: task.redelivered,
        error: task.error.clone(),
        pre,
        post,
        sidecars,
        mounts: task.mounts.clone(),
        networks: task.networks.clone(),
        node_id: task.node_id.clone(),
        retry: task.retry.clone(),
        limits: task.limits.clone(),
        timeout,
        result: task.result.clone(),
        var,
        r#if,
        parallel,
        each,
        subjob,
        gpus,
        tags: task.tags.clone(),
        workdir,
        priority: task.priority,
        progress: task.progress,
        probe: task.probe.clone(),
    })
}

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Validates that a task's current state allows completion.
/// Go: `u.State != tork.TaskStateRunning && u.State != tork.TaskStateScheduled
///       && u.State != tork.TaskStateSkipped`
#[must_use]
pub(crate) fn validate_task_can_complete(state: &str) -> bool {
    *state == *TASK_STATE_RUNNING
        || *state == *TASK_STATE_SCHEDULED
        || *state == *TASK_STATE_SKIPPED
}

/// Validates that a task is in a valid completion result state (COMPLETED or SKIPPED).
/// Go: `if t.State != tork.TaskStateCompleted && t.State != tork.TaskStateSkipped`
#[must_use]
pub(crate) fn is_completion_state(state: &str) -> bool {
    *state == *TASK_STATE_COMPLETED || *state == *TASK_STATE_SKIPPED
}

/// Calculates job progress as a percentage (0–100), rounded to 2 decimal places.
/// Go: `progress = math.Round(progress*100) / 100`
///     where `progress = float64(u.Position) / float64(u.TaskCount) * 100`
#[must_use]
pub(crate) fn calculate_progress(position: i64, task_count: i64) -> f64 {
    if task_count == 0 {
        return 0.0;
    }
    let raw = (position as f64) / (task_count as f64) * 100.0;
    (raw * 100.0).round() / 100.0
}

/// Checks if an each-task completion is the last one.
/// Go: `u.Each.Completions >= u.Each.Size`
#[must_use]
pub(crate) fn is_last_each_completion(completions: i64, size: i64) -> bool {
    completions >= size
}

/// Checks if a parallel-task completion is the last one.
/// Go: `u.Parallel.Completions >= len(u.Parallel.Tasks)`
#[must_use]
pub(crate) fn is_last_parallel_completion(completions: i64, task_count: usize) -> bool {
    completions >= task_count as i64
}

/// Checks if there's a next task to execute in the job.
/// Go: `j.Position <= len(j.Tasks)`
#[must_use]
pub(crate) fn has_next_task(position: i64, tasks_len: usize) -> bool {
    position <= tasks_len as i64
}

/// Checks if concurrency-limited dispatching should happen for an each-task.
/// Go: `!isLast && u.Each.Concurrency > 0 && u.Each.Index < u.Each.Size`
#[must_use]
pub(crate) fn should_dispatch_next(concurrency: i64, index: i64, size: i64, is_last: bool) -> bool {
    !is_last && concurrency > 0 && index < size
}

/// Functionally updates a HashMap with a new key-value pair.
/// Returns a new HashMap without mutating the original.
/// Go: `u.Context.Tasks[t.Var] = t.Result` (inside an update callback)
#[must_use]
pub(crate) fn update_context_map(
    existing: Option<HashMap<String, String>>,
    key: String,
    value: String,
) -> HashMap<String, String> {
    existing
        .unwrap_or_default()
        .into_iter()
        .chain(std::iter::once((key, value)))
        .collect()
}

/// Creates a new task for the next position in a job.
/// Go: `next := j.Tasks[j.Position-1]` + ID/JobID/State/Position/CreatedAt assignment
///
/// Note: Does not call `eval.EvaluateTask` because the eval crate is not
/// available in the coordinator. Task is created in PENDING state directly.
#[must_use]
pub(crate) fn create_next_task(
    task_def: &Task,
    job: &Job,
    position: i64,
    now: time::OffsetDateTime,
) -> Task {
    let new_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    Task {
        id: Some(new_id),
        job_id: job.id.clone(),
        state: TASK_STATE_PENDING.clone(),
        position,
        created_at: Some(now),
        ..task_def.clone()
    }
}

/// Increments each-task completions and index.
/// Go: `u.Each.Completions = u.Each.Completions + 1`
///     `u.Each.Index = u.Each.Index + 1`
#[must_use]
pub(crate) fn increment_each(each: &EachTask) -> EachTask {
    EachTask {
        completions: each.completions + 1,
        index: each.index + 1,
        ..each.clone()
    }
}

/// Increments parallel-task completions.
/// Go: `u.Parallel.Completions = u.Parallel.Completions + 1`
#[must_use]
pub(crate) fn increment_parallel(parallel: &ParallelTask) -> ParallelTask {
    ParallelTask {
        completions: parallel.completions + 1,
        ..parallel.clone()
    }
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Completed handler for processing task completion events.
///
/// Holds references to the datastore and broker for I/O operations.
/// Routes sub-task completions to each/parallel handlers.
pub struct CompletedHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for CompletedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedHandler").finish()
    }
}

impl CompletedHandler {
    /// Create a new completed handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    /// Handle a task completion event.
    ///
    /// Go parity (`handle`):
    /// 1. Validates state is COMPLETED or SKIPPED
    /// 2. Sets completed_at to now
    /// 3. Delegates to `completeTask`
    pub async fn handle(&self, task: &Task) -> Result<(), HandlerError> {
        if !is_completion_state(&task.state) {
            return Err(HandlerError::InvalidState(format!(
                "invalid completion state: {}",
                task.state
            )));
        }

        // Go: `t.CompletedAt = &now`
        let task = Task {
            completed_at: Some(time::OffsetDateTime::now_utc()),
            ..task.clone()
        };

        self.complete_task(&task).await
    }

    /// Routes to sub-task or top-level completion handler.
    /// Go: `if t.ParentID != "" { return h.completeSubTask(ctx, t) }`
    ///
    /// Returns a pinned boxed future to allow recursive calls from
    /// `complete_each_task` and `complete_parallel_task`.
    fn complete_task<'a>(
        &'a self,
        task: &'a Task,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), HandlerError>> + Send + 'a>> {
        Box::pin(async move {
            match &task.parent_id {
                Some(_) => self.complete_sub_task(task).await,
                None => self.complete_top_level_task(task).await,
            }
        })
    }

    /// Routes sub-task completion to parallel or each handler.
    /// Go: `if parent.Parallel != nil { return h.completeParallelTask(ctx, t) }`
    async fn complete_sub_task(&self, task: &Task) -> Result<(), HandlerError> {
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;

        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("parent task {parent_id} not found"))
            })?;

        match &parent.parallel {
            Some(_) => self.complete_parallel_task(task).await,
            None => self.complete_each_task(task).await,
        }
    }

    /// Handles each-task (loop) completion.
    ///
    /// Go parity (`completeEachTask`):
    /// 1. Validates and updates actual task state to completed
    /// 2. Increments parent's each.completions and each.index
    /// 3. If concurrency-limited and not last, dispatches next child task
    /// 4. Updates job context with var → result mapping
    /// 5. If last completion, completes the parent task recursively
    async fn complete_each_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // 1. Validate and update actual task
        // Go: `tx.UpdateTask(ctx, t.ID, func(u *tork.Task) error { ... })`
        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("task {task_id} not found"))
            })?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 2. Update parent task (each tracking)
        // Go: `tx.UpdateTask(ctx, t.ParentID, func(u *tork.Task) error { ... })`
        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("parent task {parent_id} not found"))
            })?;

        let each = parent
            .each
            .as_ref()
            .ok_or_else(|| HandlerError::Validation("parent has no each configuration".into()))?;

        // Go: `isLast = u.Each.Completions >= u.Each.Size`
        let is_last = is_last_each_completion(each.completions + 1, each.size);

        // Go: `!isLast && u.Each.Concurrency > 0 && u.Each.Index < u.Each.Size`
        let dispatch_next = should_dispatch_next(each.concurrency, each.index, each.size, is_last);

        // 3. If concurrency-limited, dispatch next child task
        // Go: `next, err := h.ds.GetNextTask(ctx, u.ID)` ... publish to QUEUE_PENDING
        if dispatch_next {
            if let Some(next_task) = self
                .ds
                .get_next_task(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
            {
                let next_id = next_task
                    .id
                    .clone()
                    .ok_or_else(|| HandlerError::Validation("next task has no ID".into()))?;
                let pending_task = Task {
                    state: TASK_STATE_PENDING.clone(),
                    ..next_task
                };
                self.ds
                    .update_task(next_id, pending_task.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;
                self.broker
                    .publish_task(queue::QUEUE_PENDING.to_string(), &pending_task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        // 4. Update parent task with incremented completions/index
        // Go: `u.Each.Completions = u.Each.Completions + 1`
        //     `u.Each.Index = u.Each.Index + 1`
        let updated_each = increment_each(each);
        let updated_parent = Task {
            each: Some(updated_each),
            ..parent
        };
        self.ds
            .update_task(parent_id.to_string(), updated_parent)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 5. Update job context with var → result
        // Go: `if t.Result != "" && t.Var != "" { tx.UpdateJob(...) }`
        if let (Some(var), Some(result)) = (&task.var, &task.result) {
            self.update_job_context(job_id, var.clone(), result.clone())
                .await?;
        }

        // 6. If last completion, complete the parent recursively
        // Go: `if isLast { parent.State = COMPLETED; parent.CompletedAt = &now;
        //            return h.completeTask(ctx, parent) }`
        if is_last {
            let parent = self
                .ds
                .get_task_by_id(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
                .ok_or_else(|| {
                    HandlerError::NotFound(format!("parent task {parent_id} not found"))
                })?;

            let now = time::OffsetDateTime::now_utc();
            let completed_parent = Task {
                state: TASK_STATE_COMPLETED.clone(),
                completed_at: Some(now),
                ..parent
            };
            return self.complete_task(&completed_parent).await;
        }

        Ok(())
    }

    /// Handles parallel-task completion.
    ///
    /// Go parity (`completeParallelTask`):
    /// 1. Validates and updates actual task state to completed
    /// 2. Increments parent's parallel.completions
    /// 3. Updates job context with var → result mapping
    /// 4. If last completion, completes the parent task recursively
    async fn complete_parallel_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // 1. Validate and update actual task
        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("task {task_id} not found"))
            })?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 2. Update parent task (parallel tracking)
        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("parent task {parent_id} not found"))
            })?;

        let parallel = parent
            .parallel
            .as_ref()
            .ok_or_else(|| {
                HandlerError::Validation("parent has no parallel configuration".into())
            })?;

        let parallel_task_count = parallel.tasks.as_ref().map_or(0, Vec::len);
        // Go: `isLast = u.Parallel.Completions >= len(u.Parallel.Tasks)`
        let is_last = is_last_parallel_completion(parallel.completions + 1, parallel_task_count);

        // Go: `u.Parallel.Completions = u.Parallel.Completions + 1`
        let updated_parallel = increment_parallel(parallel);
        let updated_parent = Task {
            parallel: Some(updated_parallel),
            ..parent
        };
        self.ds
            .update_task(parent_id.to_string(), updated_parent)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 3. Update job context with var → result
        if let (Some(var), Some(result)) = (&task.var, &task.result) {
            self.update_job_context(job_id, var.clone(), result.clone())
                .await?;
        }

        // 4. If last completion, complete the parent recursively
        if is_last {
            let parent = self
                .ds
                .get_task_by_id(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
                .ok_or_else(|| {
                    HandlerError::NotFound(format!("parent task {parent_id} not found"))
                })?;

            let now = time::OffsetDateTime::now_utc();
            let completed_parent = Task {
                state: TASK_STATE_COMPLETED.clone(),
                completed_at: Some(now),
                ..parent
            };
            return self.complete_task(&completed_parent).await;
        }

        Ok(())
    }

    /// Handles top-level task completion.
    ///
    /// Go parity (`completeTopLevelTask`):
    /// 1. Validates and updates task state to completed
    /// 2. Updates job progress and position
    /// 3. Updates job context with var → result mapping
    /// 4. If more tasks exist, creates the next task and publishes to QUEUE_PENDING
    /// 5. If no more tasks, marks job as COMPLETED
    async fn complete_top_level_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // 1. Validate and update task
        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("task {task_id} not found"))
            })?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 2. Update job progress and position
        // Go: `progress = float64(u.Position) / float64(u.TaskCount) * 100`
        //     `u.Position = u.Position + 1`
        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("job {job_id} not found"))
            })?;

        let progress = calculate_progress(job.position, job.task_count);
        let new_position = job.position + 1;

        // Go: `if t.Result != "" && t.Var != "" { u.Context.Tasks[t.Var] = t.Result }`
        let updated_context = if let (Some(var), Some(result)) = (&task.var, &task.result) {
            let new_tasks =
                update_context_map(job.context.tasks.clone(), var.clone(), result.clone());
            JobContext {
                tasks: Some(new_tasks),
                ..job.context.clone()
            }
        } else {
            job.context.clone()
        };

        let updated_job = Job {
            progress,
            position: new_position,
            context: updated_context,
            ..job.clone()
        };
        self.ds
            .update_job(job_id.to_string(), updated_job)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // 3. Re-read job to get updated state for routing
        // Go: `j, err := c.ds.GetJobByID(ctx, t.JobID)`
        let updated_job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("job {job_id} not found"))
            })?;

        // 4. Create next task or complete the job
        // Go: `if j.Position <= len(j.Tasks) { ... create next ... } else { ... complete job ... }`
        if has_next_task(updated_job.position, updated_job.tasks.len()) {
            let next_idx = usize::try_from(updated_job.position - 1)
                .map_err(|_| HandlerError::Validation("position overflow".into()))?;
            let task_def = updated_job
                .tasks
                .get(next_idx)
                .ok_or_else(|| HandlerError::NotFound("next task definition not found".into()))?;

            // Go: `next := j.Tasks[j.Position-1]`
            //     `next.ID = uuid.NewUUID()`
            //     `next.JobID = j.ID`
            //     `next.State = tork.TaskStatePending`
            //     `next.Position = j.Position`
            //     `next.CreatedAt = &now`
            let now = time::OffsetDateTime::now_utc();
            let next_task = create_next_task(task_def, &updated_job, new_position, now);

            // Go: `if err := eval.EvaluateTask(next, j.Context.AsMap()); err != nil { ... }`
            // Evaluate template expressions in the task against job context.
            // If evaluation fails, mark the task as FAILED.
            let ctx_map = updated_job.context.as_map();
            let evaluated_task = match evaluate_task(&next_task, &ctx_map) {
                Ok(t) => t,
                Err(eval_err) => {
                    let mut failed = next_task;
                    failed.error = Some(eval_err);
                    failed.state = TASK_STATE_FAILED.clone();
                    failed.failed_at = Some(now);
                    failed
                }
            };

            self.ds
                .create_task(evaluated_task.clone())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;
            self.broker
                .publish_task(queue::QUEUE_PENDING.to_string(), &evaluated_task)
                .await
                .map_err(|e| HandlerError::Broker(e.to_string()))?;
        } else {
            // Go: `j.State = tork.JobStateCompleted`
            //     `j.CompletedAt = &now`
            //     `return c.onJob(ctx, job.StateChange, j)`
            let now = time::OffsetDateTime::now_utc();
            let completed_job = Job {
                state: JOB_STATE_COMPLETED.to_string(),
                completed_at: Some(now),
                ..updated_job.clone()
            };
            self.ds
                .update_job(job_id.to_string(), completed_job)
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;
        }

        Ok(())
    }

    /// Updates the job context with a var → result mapping.
    /// Go: `if u.Context.Tasks == nil { u.Context.Tasks = make(map[string]string) }`
    ///     `u.Context.Tasks[t.Var] = t.Result`
    async fn update_job_context(
        &self,
        job_id: &str,
        var: String,
        result: String,
    ) -> Result<(), HandlerError> {
        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("job {job_id} not found"))
            })?;

        let new_tasks = update_context_map(job.context.tasks.clone(), var, result);
        let updated_context = JobContext {
            tasks: Some(new_tasks),
            ..job.context.clone()
        };
        let updated_job = Job {
            context: updated_context,
            ..job
        };
        self.ds
            .update_job(job_id.to_string(), updated_job)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tork::task::{TASK_STATE_CANCELLED, TASK_STATE_FAILED};

    // -- Validation tests --

    #[test]
    fn test_validate_task_can_complete_running() {
        assert!(validate_task_can_complete(TASK_STATE_RUNNING.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_scheduled() {
        assert!(validate_task_can_complete(TASK_STATE_SCHEDULED.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_skipped() {
        assert!(validate_task_can_complete(TASK_STATE_SKIPPED.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_rejects_completed() {
        assert!(!validate_task_can_complete(TASK_STATE_COMPLETED.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_rejects_failed() {
        assert!(!validate_task_can_complete(TASK_STATE_FAILED.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_rejects_pending() {
        assert!(!validate_task_can_complete(TASK_STATE_PENDING.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_rejects_cancelled() {
        assert!(!validate_task_can_complete(TASK_STATE_CANCELLED.as_ref()));
    }

    // -- Completion state tests --

    #[test]
    fn test_is_completion_state_completed() {
        assert!(is_completion_state(TASK_STATE_COMPLETED.as_ref()));
    }

    #[test]
    fn test_is_completion_state_skipped() {
        assert!(is_completion_state(TASK_STATE_SKIPPED.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_running() {
        assert!(!is_completion_state(TASK_STATE_RUNNING.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_failed() {
        assert!(!is_completion_state(TASK_STATE_FAILED.as_ref()));
    }

    // -- Progress calculation tests --

    #[test]
    fn test_calculate_progress_zero_task_count() {
        assert_eq!(calculate_progress(1, 0), 0.0);
    }

    #[test]
    fn test_calculate_progress_half() {
        assert_eq!(calculate_progress(1, 2), 50.0);
    }

    #[test]
    fn test_calculate_progress_full() {
        assert_eq!(calculate_progress(2, 2), 100.0);
    }

    #[test]
    fn test_calculate_progress_third() {
        let result = calculate_progress(1, 3);
        assert!((result - 33.33).abs() < 0.01);
    }

    #[test]
    fn test_calculate_progress_two_thirds() {
        let result = calculate_progress(2, 3);
        assert!((result - 66.67).abs() < 0.01);
    }

    // -- Completion detection tests --

    #[test]
    fn test_is_last_each_completion_exact() {
        assert!(is_last_each_completion(2, 2));
    }

    #[test]
    fn test_is_last_each_completion_over() {
        assert!(is_last_each_completion(3, 2));
    }

    #[test]
    fn test_is_last_each_completion_under() {
        assert!(!is_last_each_completion(1, 2));
    }

    #[test]
    fn test_is_last_parallel_completion_exact() {
        assert!(is_last_parallel_completion(2, 2));
    }

    #[test]
    fn test_is_last_parallel_completion_over() {
        assert!(is_last_parallel_completion(3, 2));
    }

    #[test]
    fn test_is_last_parallel_completion_under() {
        assert!(!is_last_parallel_completion(1, 2));
    }

    // -- Next task detection tests --

    #[test]
    fn test_has_next_task_more_remaining() {
        assert!(has_next_task(1, 2));
    }

    #[test]
    fn test_has_next_task_exact() {
        assert!(has_next_task(2, 2));
    }

    #[test]
    fn test_has_next_task_past_end() {
        assert!(!has_next_task(3, 2));
    }

    #[test]
    fn test_has_next_task_empty_tasks() {
        assert!(!has_next_task(1, 0));
    }

    // -- Concurrency dispatch tests --

    #[test]
    fn test_should_dispatch_next_all_conditions_met() {
        assert!(should_dispatch_next(1, 0, 3, false));
    }

    #[test]
    fn test_should_dispatch_next_is_last_blocks() {
        assert!(!should_dispatch_next(1, 0, 3, true));
    }

    #[test]
    fn test_should_dispatch_next_no_concurrency() {
        assert!(!should_dispatch_next(0, 0, 3, false));
    }

    #[test]
    fn test_should_dispatch_next_index_at_size() {
        assert!(!should_dispatch_next(1, 3, 3, false));
    }

    #[test]
    fn test_should_dispatch_next_index_past_size() {
        assert!(!should_dispatch_next(1, 4, 3, false));
    }

    // -- Context map tests --

    #[test]
    fn test_update_context_map_new_map() {
        let result = update_context_map(None, "key".into(), "value".into());
        assert_eq!(result.get("key").map(String::as_str), Some("value"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_update_context_map_append() {
        let mut existing = HashMap::new();
        existing.insert("old".into(), "val".into());
        let result = update_context_map(Some(existing), "new".into(), "val2".into());
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("old").map(String::as_str), Some("val"));
        assert_eq!(result.get("new").map(String::as_str), Some("val2"));
    }

    #[test]
    fn test_update_context_map_overwrite() {
        let mut existing = HashMap::new();
        existing.insert("key".into(), "old".into());
        let result = update_context_map(Some(existing), "key".into(), "new".into());
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("key").map(String::as_str), Some("new"));
    }

    // -- Next task creation tests --

    #[test]
    fn test_create_next_task_assigns_fields() {
        let task_def = Task {
            name: Some("echo-hello".into()),
            run: Some("echo hello".into()),
            ..Task::default()
        };
        let job = Job {
            id: Some("job-1".into()),
            ..Job::default()
        };
        let now = time::OffsetDateTime::now_utc();
        let next = create_next_task(&task_def, &job, 1, now);

        assert!(next.id.is_some());
        assert_ne!(next.id.as_deref(), Some(""));
        assert_eq!(next.job_id.as_deref(), Some("job-1"));
        assert_eq!(next.state, *TASK_STATE_PENDING);
        assert_eq!(next.position, 1);
        assert!(next.created_at.is_some());
        assert_eq!(next.name.as_deref(), Some("echo-hello"));
        assert_eq!(next.run.as_deref(), Some("echo hello"));
    }

    #[test]
    fn test_create_next_task_generates_unique_ids() {
        let task_def = Task::default();
        let job = Job {
            id: Some("job-1".into()),
            ..Job::default()
        };
        let now = time::OffsetDateTime::now_utc();
        let t1 = create_next_task(&task_def, &job, 1, now);
        let t2 = create_next_task(&task_def, &job, 2, now);
        assert_ne!(t1.id, t2.id);
    }

    // -- Each increment tests --

    #[test]
    fn test_increment_each() {
        let each = EachTask {
            completions: 1,
            index: 1,
            size: 3,
            concurrency: 1,
            var: Some("item".into()),
            list: Some("[1,2,3]".into()),
            task: None,
        };
        let incremented = increment_each(&each);
        assert_eq!(incremented.completions, 2);
        assert_eq!(incremented.index, 2);
        assert_eq!(incremented.size, 3);
        assert_eq!(incremented.concurrency, 1);
        assert_eq!(incremented.var.as_deref(), Some("item"));
    }

    #[test]
    fn test_increment_each_preserves_task() {
        let inner_task = Task {
            name: Some("inner".into()),
            ..Task::default()
        };
        let each = EachTask {
            completions: 0,
            index: 0,
            size: 1,
            concurrency: 0,
            var: None,
            list: None,
            task: Some(Box::new(inner_task.clone())),
        };
        let incremented = increment_each(&each);
        assert_eq!(incremented.completions, 1);
        assert_eq!(incremented.index, 1);
        assert_eq!(incremented.task.as_deref(), Some(&inner_task));
    }

    // -- Parallel increment tests --

    #[test]
    fn test_increment_parallel() {
        let parallel = ParallelTask {
            completions: 1,
            tasks: Some(vec![Task::default(), Task::default()]),
        };
        let incremented = increment_parallel(&parallel);
        assert_eq!(incremented.completions, 2);
        assert_eq!(incremented.tasks.as_ref().map(Vec::len), Some(2));
    }

    // -- Edge cases from Go test parity ------------------------------------

    // Go: Test_handleCompletedLastTask — position == task_count → 100% progress
    #[test]
    fn test_calculate_progress_last_task() {
        assert_eq!(calculate_progress(2, 2), 100.0);
    }

    // Go: Test_handleCompletedFirstTask — position 1 of 2 → 50% progress
    #[test]
    fn test_calculate_progress_first_of_two() {
        assert_eq!(calculate_progress(1, 2), 50.0);
    }

    // Go: 0-based position (single task) → 100%
    #[test]
    fn test_calculate_progress_single_task() {
        assert_eq!(calculate_progress(1, 1), 100.0);
    }

    // Go: large task count — verifies no floating point drift
    #[test]
    fn test_calculate_progress_large_task_count() {
        let result = calculate_progress(50, 100);
        assert_eq!(result, 50.0);
    }

    // Go: verify rounding for odd divisions
    #[test]
    fn test_calculate_progress_rounding() {
        // 1/7 ≈ 14.2857... → round to 14.29
        let result = calculate_progress(1, 7);
        let expected = (1.0_f64 / 7.0 * 100.0 * 100.0).round() / 100.0;
        assert_eq!(result, expected);
    }

    // Go: zero division guard
    #[test]
    fn test_calculate_progress_position_zero() {
        assert_eq!(calculate_progress(0, 2), 0.0);
    }

    // Go: negative task count — current impl doesn't guard negative, document behavior
    #[test]
    fn test_calculate_progress_negative_task_count() {
        // calculate_progress only guards against zero, not negative.
        // Negative task_count produces negative progress — that's the current behavior.
        let result = calculate_progress(1, -5);
        assert!(result < 0.0);
    }

    // -- validate_task_can_complete additional states -----------------------

    #[test]
    fn test_validate_task_can_complete_rejects_created() {
        assert!(!validate_task_can_complete(tork::task::TASK_STATE_CREATED.as_ref()));
    }

    #[test]
    fn test_validate_task_can_complete_rejects_stopped() {
        assert!(!validate_task_can_complete(tork::task::TASK_STATE_STOPPED.as_ref()));
    }

    // -- has_next_task additional edges --------------------------------------

    #[test]
    fn test_has_next_task_position_zero() {
        assert!(has_next_task(0, 2));
    }

    #[test]
    fn test_has_next_task_single_task_at_end() {
        assert!(has_next_task(1, 1));
    }

    #[test]
    fn test_has_next_task_single_task_past_end() {
        assert!(!has_next_task(2, 1));
    }

    // -- is_last_each_completion additional edges ---------------------------

    #[test]
    fn test_is_last_each_completion_zero_size() {
        // completions (0) >= size (0) → true
        assert!(is_last_each_completion(0, 0));
    }

    #[test]
    fn test_is_last_each_completion_negative_completions() {
        assert!(!is_last_each_completion(-1, 2));
    }

    // -- is_last_parallel_completion additional edges ------------------------

    #[test]
    fn test_is_last_parallel_completion_zero_tasks() {
        assert!(is_last_parallel_completion(0, 0));
    }

    #[test]
    fn test_is_last_parallel_completion_one_task() {
        assert!(is_last_parallel_completion(1, 1));
    }

    #[test]
    fn test_is_last_parallel_completion_negative_completions() {
        assert!(!is_last_parallel_completion(-1, 3));
    }

    // -- should_dispatch_next additional edges -------------------------------

    #[test]
    fn test_should_dispatch_next_negative_concurrency() {
        assert!(!should_dispatch_next(-1, 0, 3, false));
    }

    #[test]
    fn test_should_dispatch_next_negative_index() {
        // index < size but concurrency > 0 → true even with negative start
        assert!(should_dispatch_next(1, -1, 3, false));
    }

    #[test]
    fn test_should_dispatch_next_zero_size() {
        // index (0) < size (0) is false → no dispatch
        assert!(!should_dispatch_next(1, 0, 0, false));
    }

    // -- update_context_map additional edges ---------------------------------

    #[test]
    fn test_update_context_map_existing_with_same_key() {
        let existing = {
            let mut m = HashMap::new();
            m.insert("key".into(), "old".into());
            Some(m)
        };
        let result = update_context_map(existing, "key".into(), "new".into());
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("key").map(String::as_str), Some("new"));
    }

    #[test]
    fn test_update_context_map_many_keys() {
        let existing = {
            let mut m = HashMap::new();
            m.insert("a".into(), "1".into());
            m.insert("b".into(), "2".into());
            m.insert("c".into(), "3".into());
            Some(m)
        };
        let result = update_context_map(existing, "d".into(), "4".into());
        assert_eq!(result.len(), 4);
        assert_eq!(result.get("d").map(String::as_str), Some("4"));
    }

    // -- create_next_task additional edges -----------------------------------

    #[test]
    fn test_create_next_task_preserves_image() {
        let task_def = Task {
            name: Some("build".into()),
            image: Some("alpine:3.18".into()),
            ..Task::default()
        };
        let job = Job { id: Some("j1".into()), ..Job::default() };
        let now = time::OffsetDateTime::now_utc();
        let next = create_next_task(&task_def, &job, 1, now);
        assert_eq!(next.image.as_deref(), Some("alpine:3.18"));
    }

    #[test]
    fn test_create_next_task_preserves_env() {
        let env = {
            let mut m = HashMap::new();
            m.insert("FOO".into(), "bar".into());
            Some(m)
        };
        let task_def = Task { env, ..Task::default() };
        let job = Job { id: Some("j1".into()), ..Job::default() };
        let now = time::OffsetDateTime::now_utc();
        let next = create_next_task(&task_def, &job, 2, now);
        assert_eq!(next.env.as_ref().map(HashMap::len), Some(1));
        assert_eq!(next.position, 2);
    }

    #[test]
    fn test_create_next_task_preserves_queue() {
        let task_def = Task {
            queue: Some("my-queue".into()),
            ..Task::default()
        };
        let job = Job { id: Some("j1".into()), ..Job::default() };
        let now = time::OffsetDateTime::now_utc();
        let next = create_next_task(&task_def, &job, 3, now);
        assert_eq!(next.queue.as_deref(), Some("my-queue"));
    }

    // -- increment_each additional edges -------------------------------------

    #[test]
    fn test_increment_each_from_zero() {
        let each = EachTask {
            completions: 0,
            index: 0,
            size: 5,
            concurrency: 2,
            var: None,
            list: None,
            task: None,
        };
        let incremented = increment_each(&each);
        assert_eq!(incremented.completions, 1);
        assert_eq!(incremented.index, 1);
    }

    #[test]
    fn test_increment_each_preserves_var() {
        let each = EachTask {
            completions: 2,
            index: 2,
            size: 4,
            concurrency: 1,
            var: Some("myItem".into()),
            list: Some("[1,2,3,4]".into()),
            task: None,
        };
        let incremented = increment_each(&each);
        assert_eq!(incremented.var.as_deref(), Some("myItem"));
        assert_eq!(incremented.list.as_deref(), Some("[1,2,3,4]"));
    }

    // -- increment_parallel additional edges ---------------------------------

    #[test]
    fn test_increment_parallel_from_zero() {
        let parallel = ParallelTask {
            completions: 0,
            tasks: Some(vec![]),
        };
        let incremented = increment_parallel(&parallel);
        assert_eq!(incremented.completions, 1);
        assert_eq!(incremented.tasks.as_ref().map(Vec::len), Some(0));
    }

    #[test]
    fn test_increment_parallel_preserves_tasks() {
        let inner = vec![
            Task { name: Some("p1".into()), ..Task::default() },
            Task { name: Some("p2".into()), ..Task::default() },
            Task { name: Some("p3".into()), ..Task::default() },
        ];
        let parallel = ParallelTask {
            completions: 1,
            tasks: Some(inner.clone()),
        };
        let incremented = increment_parallel(&parallel);
        assert_eq!(incremented.tasks.as_ref().map(Vec::len), Some(3));
        assert_eq!(
            incremented.tasks.as_ref().and_then(|t| t.first()).and_then(|t| t.name.as_deref()),
            Some("p1")
        );
    }

    // -- is_completion_state additional edges --------------------------------

    #[test]
    fn test_is_completion_state_rejects_scheduled() {
        assert!(!is_completion_state(TASK_STATE_SCHEDULED.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_pending() {
        assert!(!is_completion_state(TASK_STATE_PENDING.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_cancelled() {
        assert!(!is_completion_state(TASK_STATE_CANCELLED.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_stopped() {
        assert!(!is_completion_state(tork::task::TASK_STATE_STOPPED.as_ref()));
    }

    #[test]
    fn test_is_completion_state_rejects_created() {
        assert!(!is_completion_state(tork::task::TASK_STATE_CREATED.as_ref()));
    }

    // -- CompletedHandler construction tests ---------------------------------

    #[test]
    fn test_completed_handler_debug() {
        let handler = CompletedHandler::new(
            std::sync::Arc::new(MockDs),
            std::sync::Arc::new(MockBrk),
        );
        let debug_str = format!("{handler:?}");
        assert!(debug_str.contains("CompletedHandler"));
    }

    #[test]
    fn test_completed_handler_rejects_invalid_state() {
        let handler = CompletedHandler::new(
            std::sync::Arc::new(MockDs),
            std::sync::Arc::new(MockBrk),
        );
        let task = Task {
            id: Some("t1".into()),
            state: TASK_STATE_RUNNING.clone(),
            ..Task::default()
        };
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(handler.handle(&task));
        assert!(result.is_err());
        match result.err() {
            Some(HandlerError::InvalidState(msg)) => {
                assert!(msg.contains("invalid completion state"));
            }
            other => panic!("expected InvalidState, got: {other:?}"),
        }
    }

    // -- Mock implementations for handler construction tests -----------------

    struct MockDs;

    impl tork::Datastore for MockDs {
        fn create_task(&self, _task: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_task(&self, _id: String, _task: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_tasks(&self, _job_id: String) -> tork::datastore::BoxedFuture<Vec<Task>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_next_task(&self, _parent_task_id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(&self, _part: tork::task::TaskLogPart) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self, _task_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn create_node(&self, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_node(&self, _id: String, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::node::Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<tork::node::Node>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_job(&self, _job: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, _id: String, _job: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Job>> {
            Box::pin(async { Ok(None) })
        }
        fn get_job_log_parts(
            &self, _job_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_jobs(
            &self, _current_user: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn create_scheduled_job(&self, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self, _current_user: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_scheduled_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(&self, _id: String, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _user: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(&self, _username: String) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
            Box::pin(async { Ok(None) })
        }
        fn create_role(&self, _role: tork::role::Role) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_role(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::role::Role>> {
            Box::pin(async { Ok(None) })
        }
        fn get_roles(&self) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn assign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_metrics(&self) -> tork::datastore::BoxedFuture<tork::stats::Metrics> {
            Box::pin(async { Ok(tork::stats::Metrics::default()) })
        }
        fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    struct MockBrk;

    impl tork::Broker for MockBrk {
        fn publish_task(&self, _qname: String, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(&self, _qname: String, _handler: tork::broker::TaskHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(&self, _handler: tork::broker::TaskProgressHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _node: tork::node::Node) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(&self, _handler: tork::broker::HeartbeatHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &tork::job::Job) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(&self, _handler: tork::broker::JobHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(&self, _topic: String, _event: serde_json::Value) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(&self, _pattern: String, _handler: tork::broker::EventHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(&self, _part: &tork::task::TaskLogPart) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(&self, _handler: tork::broker::TaskLogPartHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> {
            Box::pin(async { Ok(tork::broker::QueueInfo { name: String::new(), size: 0, subscribers: 0, unacked: 0 }) })
        }
        fn delete_queue(&self, _qname: String) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    // -- Integration tests (require real datastore, ignored by default) -----

    use crate::handlers::test_helpers::{new_uuid, TestEnv};

    /// Go parity: Test_handleCompletedLastTask — last task completes → 100% progress
    #[tokio::test]
    async fn test_handle_completed_last_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 2,
            task_count: 2,
            tasks: vec![
                Task { name: Some("task-1".into()), ..Task::default() },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            created_at: Some(now),
            job_id: Some(job_id.clone()),
            position: 2,
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle completed");

        let t2 = env.ds.get_task_by_id(completed_t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(t2.state, *TASK_STATE_COMPLETED);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.progress, 100.0);

        env.cleanup().await;
    }

    /// Go parity: Test_handleCompletedFirstTask — first task → 50% progress, job stays RUNNING
    #[tokio::test]
    async fn test_handle_completed_first_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 2,
            tasks: vec![
                Task { name: Some("task-1".into()), ..Task::default() },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            created_at: Some(now),
            job_id: Some(job_id.clone()),
            position: 1,
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle completed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let t2 = env.ds.get_task_by_id(completed_t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(t2.state, *TASK_STATE_COMPLETED);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.state, tork::job::JOB_STATE_RUNNING);
        assert_eq!(j2.progress, 50.0);

        env.cleanup().await;
    }

    /// Go parity: Test_handleSkippedTask — skipped task counts as completed
    #[tokio::test]
    async fn test_handle_skipped_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 2,
            task_count: 2,
            tasks: vec![
                Task { name: Some("task-1".into()), ..Task::default() },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_SKIPPED.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            created_at: Some(now),
            job_id: Some(job_id.clone()),
            position: 2,
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        let mut skipped_t1 = Task { state: TASK_STATE_SKIPPED.clone(), ..t1 };
        handler.handle(&skipped_t1).await.expect("handle skipped");

        let t2 = env.ds.get_task_by_id(skipped_t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(t2.state, *TASK_STATE_SKIPPED);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.progress, 100.0);

        env.cleanup().await;
    }

    /// Go parity: Test_handleCompletedLastSubJobTask — sub-job last task completion
    #[tokio::test]
    async fn test_handle_completed_last_subjob_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let parent_job_id = new_uuid();
        let parent_job = Job {
            id: Some(parent_job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 1,
            tasks: vec![Task { name: Some("task-1".into()), ..Task::default() }],
            ..Job::default()
        };
        env.ds.create_job(parent_job).await.expect("create parent job");

        let parent_task_id = new_uuid();
        let parent_task = Task {
            id: Some(parent_task_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(parent_job_id.clone()),
            position: 2,
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(parent_task).await.expect("create parent task");

        let sub_job_id = new_uuid();
        let sub_job = Job {
            id: Some(sub_job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 1,
            parent_id: Some(parent_task_id.clone()),
            tasks: vec![Task { name: Some("sub-task-1".into()), run: Some("echo hello".into()), ..Task::default() }],
            ..Job::default()
        };
        env.ds.create_job(sub_job).await.expect("create sub job");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(sub_job_id.clone()),
            position: 2,
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle completed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let t2 = env.ds.get_task_by_id(completed_t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(t2.state, *TASK_STATE_COMPLETED);

        let j2 = env.ds.get_job_by_id(sub_job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.state, tork::job::JOB_STATE_COMPLETED);

        env.cleanup().await;
    }

    /// Go parity: Test_handleCompletedParallelTask
    #[tokio::test]
    async fn test_handle_completed_parallel_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 2,
            tasks: vec![
                Task {
                    name: Some("task-1".into()),
                    parallel: Some(ParallelTask {
                        tasks: Some(vec![
                            Task { name: Some("parallel-task-1".into()), ..Task::default() },
                            Task { name: Some("parallel-task-2".into()), ..Task::default() },
                        ]),
                        completions: 0,
                    }),
                    ..Task::default()
                },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let pt_id = new_uuid();
        let pt = Task {
            id: Some(pt_id.clone()),
            job_id: Some(job_id.clone()),
            parallel: Some(ParallelTask {
                tasks: Some(vec![
                    Task { name: Some("parallel-task-1".into()), ..Task::default() },
                    Task { name: Some("parallel-task-2".into()), ..Task::default() },
                ]),
                completions: 0,
            }),
            state: TASK_STATE_RUNNING.clone(),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(pt).await.expect("create parent parallel task");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        let t5 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_SCHEDULED.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task t1");
        env.ds.create_task(t5.clone()).await.expect("create task t5");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle t1");

        let mut completed_t5 = Task { state: TASK_STATE_COMPLETED.clone(), ..t5 };
        handler.handle(&completed_t5).await.expect("handle t5");

        let pt1 = env.ds.get_task_by_id(pt_id).await.expect("get parent").expect("parent exists");
        assert_eq!(pt1.state, *TASK_STATE_COMPLETED);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.state, tork::job::JOB_STATE_RUNNING);

        env.cleanup().await;
    }

    /// Go parity: Test_handleCompletedEachTask
    #[tokio::test]
    async fn test_handle_completed_each_task_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 2,
            tasks: vec![
                Task {
                    name: Some("task-1".into()),
                    each: Some(EachTask {
                        size: 2,
                        list: Some("some expression".into()),
                        task: Some(Box::new(Task { name: Some("some task".into()), ..Task::default() })),
                        ..EachTask::default()
                    }),
                    ..Task::default()
                },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let pt_id = new_uuid();
        let pt = Task {
            id: Some(pt_id.clone()),
            job_id: Some(job_id.clone()),
            position: 1,
            name: Some("parent task".into()),
            each: Some(EachTask {
                size: 2,
                list: Some("some expression".into()),
                task: Some(Box::new(Task { name: Some("some task".into()), ..Task::default() })),
                ..EachTask::default()
            }),
            state: TASK_STATE_RUNNING.clone(),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(pt).await.expect("create parent each task");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        let t5 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_SCHEDULED.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task t1");
        env.ds.create_task(t5.clone()).await.expect("create task t5");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle t1");

        let mut completed_t5 = Task { state: TASK_STATE_COMPLETED.clone(), ..t5 };
        handler.handle(&completed_t5).await.expect("handle t5");

        let pt1 = env.ds.get_task_by_id(pt_id).await.expect("get parent").expect("parent exists");
        assert_eq!(pt1.state, *TASK_STATE_COMPLETED);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.state, tork::job::JOB_STATE_RUNNING);

        env.cleanup().await;
    }

    /// Go parity: Test_handleCompletedEachTaskWithNextTask
    #[tokio::test]
    async fn test_handle_completed_each_task_with_next_integration() {
        let env = TestEnv::new().await;
        let handler = CompletedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_RUNNING.to_string(),
            position: 1,
            task_count: 2,
            tasks: vec![
                Task {
                    name: Some("task-1".into()),
                    each: Some(EachTask {
                        size: 3,
                        concurrency: 1,
                        list: Some("some expression".into()),
                        task: Some(Box::new(Task { name: Some("some task".into()), ..Task::default() })),
                        ..EachTask::default()
                    }),
                    ..Task::default()
                },
                Task { name: Some("task-2".into()), ..Task::default() },
            ],
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let pt_id = new_uuid();
        let pt = Task {
            id: Some(pt_id.clone()),
            job_id: Some(job_id.clone()),
            position: 1,
            name: Some("parent task".into()),
            each: Some(EachTask {
                size: 3,
                concurrency: 1,
                list: Some("some expression".into()),
                task: Some(Box::new(Task { name: Some("some task".into()), ..Task::default() })),
                ..EachTask::default()
            }),
            state: TASK_STATE_RUNNING.clone(),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(pt).await.expect("create parent each task");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        let t6 = Task {
            id: Some(new_uuid()),
            state: tork::task::TASK_STATE_CREATED.clone(),
            started_at: Some(now),
            completed_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            position: 1,
            parent_id: Some(pt_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task t1");
        env.ds.create_task(t6.clone()).await.expect("create task t6");

        let mut completed_t1 = Task { state: TASK_STATE_COMPLETED.clone(), ..t1 };
        handler.handle(&completed_t1).await.expect("handle t1");

        let t3 = env.ds.get_task_by_id(t6.id.clone().unwrap()).await.expect("get t6").expect("t6 exists");
        assert_eq!(t3.state, *TASK_STATE_PENDING);

        let pt1 = env.ds.get_task_by_id(pt_id).await.expect("get parent").expect("parent exists");
        assert_eq!(pt1.state, *TASK_STATE_RUNNING);

        env.cleanup().await;
    }
}
