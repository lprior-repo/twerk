//! Job handler for job state change events.
//!
//! Ported from Go `internal/coordinator/handlers/job.go`.
//! Manages the full job lifecycle: start, complete, fail, restart, cancel.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use evalexpr::{eval_with_context, ContextWithMutableVariables, HashMapContext, Value as EvalValue};
use regex::Regex;
use time::OffsetDateTime;
use tracing::{debug, error};

use tork::broker::queue::{QUEUE_COMPLETED, QUEUE_ERROR, QUEUE_PENDING};
use tork::broker::Broker;
use tork::datastore::Datastore;
use tork::job::{Job, JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_PENDING,
    JOB_STATE_RESTART, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};
use tork::task::{Task, TASK_STATE_CANCELLED, TASK_STATE_FAILED, TASK_STATE_PENDING};

use crate::handlers::{HandlerError, JobEventType};

// Topic constants matching Go broker package
const TOPIC_JOB_COMPLETED: &str = "job.completed";
const TOPIC_JOB_FAILED: &str = "job.failed";

/// Regex to match `{{ expr }}` template patterns.
#[allow(clippy::expect_used)]
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid template regex"));

// ---------------------------------------------------------------------------
// Eval helpers (pure calculation)
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
///
/// Flattens nested objects so that `inputs.var1` becomes a top-level
/// variable, matching how Go's `expr` library handles dot notation.
fn create_eval_context(
    context: &HashMap<String, serde_json::Value>,
) -> Result<HashMapContext, String> {
    let mut ctx = HashMapContext::new();
    for (key, value) in context {
        for (flat_key, eval_value) in flatten_json_value(key, value) {
            let key_name = flat_key.clone();
            ctx.set_value(flat_key, eval_value)
                .map_err(|e| format!("{key_name}: {e}"))?;
        }
    }
    Ok(ctx)
}

/// Recursively flatten a JSON value into dot-separated key-value pairs.
fn flatten_json_value(
    prefix: &str,
    value: &serde_json::Value,
) -> Vec<(String, EvalValue)> {
    match value {
        serde_json::Value::Object(map) => {
            map.iter()
                .flat_map(|(k, v)| {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    flatten_json_value(&key, v)
                })
                .collect()
        }
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
///
/// Parity with Go `EvaluateTemplate(ex string, c map[string]any) (string, error)`.
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
            let caps = TEMPLATE_REGEX.captures(m.as_str())
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
    let cmd = task.cmd.as_ref().map(|cmds| {
        cmds.iter()
            .map(|s| evaluate_template(s, context))
            .collect::<Result<Vec<_>, _>>()
    }).transpose()?;

    // Evaluate entrypoint array
    let entrypoint = task.entrypoint.as_ref().map(|eps| {
        eps.iter()
            .map(|s| evaluate_template(s, context))
            .collect::<Result<Vec<_>, _>>()
    }).transpose()?;

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
        let inner_task = each.task.as_ref()
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

/// Parse a Go-style duration string (e.g. "1m", "2h30m", "500ms").
fn parse_duration(s: &str) -> Result<StdDuration, String> {
    let s = s.trim();
    // Handle "500ms" style
    if let Some(ms_str) = s.strip_suffix("ms") {
        let ms: u64 = ms_str.parse().map_err(|_| format!("invalid duration ms: {s}"))?;
        return Ok(StdDuration::from_millis(ms));
    }
    // Parse "1m", "2h30m", "30s" etc. via regex
    let mut total_secs: u64 = 0;
    let re = Regex::new(r"(\d+)(h|m|s)")
        .map_err(|e| format!("duration regex error: {e}"))?;
    for cap in re.captures_iter(s) {
        let n: u64 = cap[1].parse().map_err(|_| format!("invalid duration: {s}"))?;
        total_secs += match &cap[2] {
            "h" => n.checked_mul(3600),
            "m" => n.checked_mul(60),
            "s" => Some(n),
            _ => None,
        }.ok_or_else(|| format!("duration overflow in: {s}"))?;
    }
    if total_secs == 0 && !s.is_empty() {
        return Err(format!("invalid duration: {s}"));
    }
    Ok(StdDuration::from_secs(total_secs))
}

// ---------------------------------------------------------------------------
// JobHandler
// ---------------------------------------------------------------------------

/// Job handler for processing job state change events.
///
/// Holds datastore and broker references for I/O operations.
/// Ported from Go `internal/coordinator/handlers/job.go`.
pub struct JobHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for JobHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandler").finish_non_exhaustive()
    }
}

impl JobHandler {
    /// Create a new job handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    /// Handle a job event, dispatching to the appropriate sub-handler.
    ///
    /// Only `StateChange` events are processed; all others are silently ignored.
    /// Parity with Go `func (h *jobHandler) handle(...)`.
    pub fn handle<'a>(
        &'a self,
        et: JobEventType,
        job: &'a mut Job,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), HandlerError>> + Send + 'a>> {
        Box::pin(async move {
            if et != JobEventType::StateChange {
                return Ok(());
            }
            match job.state.as_str() {
                s if s == JOB_STATE_PENDING => self.start_job(job).await,
                s if s == JOB_STATE_CANCELLED => self.cancel_job(job).await,
                s if s == JOB_STATE_RESTART => self.restart_job(job).await,
                s if s == JOB_STATE_COMPLETED => self.complete_job(job).await,
                s if s == JOB_STATE_FAILED => self.fail_job(job).await,
                s if s == JOB_STATE_RUNNING => self.mark_job_as_running(job).await,
                other => Err(HandlerError::InvalidState(format!(
                    "invalid job state: {other}"
                ))),
            }
        })
    }

    /// Start a pending job: create first task, evaluate it, persist, transition to SCHEDULED.
    ///
    /// If task evaluation fails, the task is created as FAILED and the job
    /// transitions to FAILED via recursive handle.
    /// Parity with Go `func (h *jobHandler) startJob(...)`.
    async fn start_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        debug!("starting job {}", job.id.as_deref().unwrap_or("(no id)"));
        let now = OffsetDateTime::now_utc();
        let job_id = job.id.clone().unwrap_or_default();

        if job.tasks.is_empty() {
            return Err(HandlerError::Handler("job has no tasks".to_string()));
        }

        // Clone and prepare the first task
        let ctx_map = job.context.as_map();
        let base_task = &job.tasks[0];
        let mut task = match evaluate_task(base_task, &ctx_map) {
            Ok(t) => t,
            Err(eval_err) => {
                let mut failed = base_task.clone();
                failed.error = Some(eval_err);
                failed.state = TASK_STATE_FAILED.clone();
                failed.failed_at = Some(now);
                failed
            }
        };
        task.id = Some(uuid::Uuid::new_v4().to_string());
        task.job_id = Some(job_id.clone());
        task.position = 1;
        task.created_at = Some(now);
        if task.state != *TASK_STATE_FAILED {
            task.state = TASK_STATE_PENDING.clone();
        }

        // Persist the task
        let task_clone = task.clone();
        self.ds.create_task(task_clone).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Transition job to SCHEDULED
        let mut updated_job = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;
        updated_job.state = JOB_STATE_SCHEDULED.to_string();
        updated_job.started_at = Some(OffsetDateTime::now_utc());
        updated_job.position = 1;
        let updated_job_id = updated_job.id.clone().unwrap_or_default();
        self.ds.update_job(updated_job_id, updated_job).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // If task eval failed, mark job as FAILED and recurse
        if task.state == *TASK_STATE_FAILED {
            let now = OffsetDateTime::now_utc();
            job.failed_at = Some(now);
            job.state = JOB_STATE_FAILED.to_string();
            job.error = task.error.clone();
            return self.handle(JobEventType::StateChange, job).await;
        }

        // Publish the task to the pending queue (scheduler picks it up)
        self.broker.publish_task(QUEUE_PENDING.to_string(), &task).await
            .map_err(|e| HandlerError::Broker(e.to_string()))?;

        Ok(())
    }

    /// Complete a job: evaluate output, handle auto-delete, handle parent task.
    ///
    /// Parity with Go `func (h *jobHandler) completeJob(...)`.
    async fn complete_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        // Read current state from datastore
        let current = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;

        if current.state != JOB_STATE_RUNNING && current.state != JOB_STATE_SCHEDULED {
            return Err(HandlerError::InvalidState(format!(
                "job {job_id} is {} and can not be completed",
                current.state
            )));
        }

        let now = OffsetDateTime::now_utc();
        let ctx_map = job.context.as_map();

        // Evaluate the job's output template
        let (new_state, result, err_msg, delete_at) = match &job.output {
            None => (
                JOB_STATE_COMPLETED.to_string(),
                None,
                None,
                None,
            ),
            Some(output) => match evaluate_template(output, &ctx_map) {
                Ok(evaluated) => {
                    let delete_at = job.auto_delete.as_ref().and_then(|ad| {
                        ad.after.as_ref().and_then(|after| {
                            parse_duration(after).ok().map(|dur| {
                                OffsetDateTime::now_utc() + dur
                            })
                        })
                    });
                    (
                        JOB_STATE_COMPLETED.to_string(),
                        Some(evaluated),
                        None,
                        delete_at,
                    )
                }
                Err(eval_err) => {
                    error!(error = %eval_err, job_id = %job_id, "error evaluating job output");
                    (
                        JOB_STATE_FAILED.to_string(),
                        None,
                        Some(eval_err),
                        None,
                    )
                }
            },
        };

        // Apply state changes
        job.state = new_state.clone();
        job.result = result.clone();
        job.error = err_msg.clone();

        let mut updated = current;
        updated.state = new_state.clone();
        if new_state == JOB_STATE_COMPLETED {
            updated.completed_at = Some(now);
            updated.result = result.clone();
            updated.delete_at = delete_at;
        } else {
            updated.failed_at = Some(now);
            updated.error = err_msg.clone();
        }

        let upd_id = updated.id.clone().unwrap_or_default();
        self.ds.update_job(upd_id, updated).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Handle sub-job: complete/fail the parent task
        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent = self.ds.get_task_by_id(parent_id.clone()).await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("parent task not found: {parent_id}")))?;

                let mut updated_parent = parent.clone();
                if new_state == JOB_STATE_FAILED {
                    updated_parent.state = TASK_STATE_FAILED.clone();
                    updated_parent.failed_at = Some(now);
                    updated_parent.error = err_msg.clone();
                } else {
                    updated_parent.state = tork::task::TASK_STATE_COMPLETED.clone();
                    updated_parent.completed_at = Some(now);
                    updated_parent.result = result.clone();
                }

                let queue = if new_state == JOB_STATE_FAILED {
                    QUEUE_ERROR
                } else {
                    QUEUE_COMPLETED
                };
                return self.broker.publish_task(queue.to_string(), &updated_parent).await
                    .map_err(|e| HandlerError::Broker(e.to_string()));
            }
        }

        // Publish job completed/failed event
        if new_state == JOB_STATE_FAILED {
            let event = serde_json::to_value(job)
                .map_err(|e| HandlerError::Handler(e.to_string()))?;
            self.broker.publish_event(TOPIC_JOB_FAILED.to_string(), event).await
                .map_err(|e| HandlerError::Broker(e.to_string()))
        } else {
            let event = serde_json::to_value(job)
                .map_err(|e| HandlerError::Handler(e.to_string()))?;
            self.broker.publish_event(TOPIC_JOB_COMPLETED.to_string(), event).await
                .map_err(|e| HandlerError::Broker(e.to_string()))
        }
    }

    /// Mark a job as RUNNING (transition from SCHEDULED).
    ///
    /// Parity with Go `func (h *jobHandler) markJobAsRunning(...)`.
    async fn mark_job_as_running(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        let current = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let Some(mut current) = current else {
            return Ok(());
        };

        if current.state != JOB_STATE_SCHEDULED {
            return Ok(());
        }

        current.state = JOB_STATE_RUNNING.to_string();
        current.failed_at = None;
        job.state = current.state.clone();

        let upd_id = current.id.clone().unwrap_or_default();
        self.ds.update_job(upd_id, current).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))
    }

    /// Restart a failed or cancelled job: create new task at position 0.
    ///
    /// Parity with Go `func (h *jobHandler) restartJob(...)`.
    async fn restart_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        // Verify job can be restarted
        let current = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;

        if current.state != JOB_STATE_FAILED && current.state != JOB_STATE_CANCELLED {
            return Err(HandlerError::InvalidState(format!(
                "job {job_id} is in {} state and can't be restarted",
                current.state
            )));
        }

        let mut updated = current;
        updated.state = JOB_STATE_RUNNING.to_string();
        updated.failed_at = None;
        let upd_id = updated.id.clone().unwrap_or_default();
        self.ds.update_job(upd_id, updated).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Create new task at current position
        let now = OffsetDateTime::now_utc();
        let position = job.position.max(1) as usize;
        let task_index = position.saturating_sub(1);

        if task_index >= job.tasks.len() {
            return Err(HandlerError::Handler(format!(
                "task index {task_index} out of range (job has {} tasks)",
                job.tasks.len()
            )));
        }

        let ctx_map = job.context.as_map();
        let base_task = &job.tasks[task_index];
        let mut task = match evaluate_task(base_task, &ctx_map) {
            Ok(t) => t,
            Err(eval_err) => {
                let mut failed = base_task.clone();
                failed.error = Some(eval_err);
                failed.state = TASK_STATE_FAILED.clone();
                failed.failed_at = Some(now);
                failed
            }
        };
        task.id = Some(uuid::Uuid::new_v4().to_string());
        task.job_id = Some(job_id);
        task.state = TASK_STATE_PENDING.clone();
        task.position = job.position;
        task.created_at = Some(now);

        self.ds.create_task(task.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        self.broker.publish_task(QUEUE_PENDING.to_string(), &task).await
            .map_err(|e| HandlerError::Broker(e.to_string()))
    }

    /// Fail a job: mark as FAILED, handle parent, cancel active tasks.
    ///
    /// Parity with Go `func (h *jobHandler) failJob(...)`.
    async fn fail_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        debug!(job_id = ?job.id, error = ?job.error, "job failed");
        let job_id = job.id.clone().unwrap_or_default();

        // Only transition to FAILED if currently RUNNING or SCHEDULED
        let current = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(mut current) = current {
            if current.state == JOB_STATE_RUNNING || current.state == JOB_STATE_SCHEDULED {
                current.state = JOB_STATE_FAILED.to_string();
                current.failed_at = job.failed_at;
                let upd_id = current.id.clone().unwrap_or_default();
                self.ds.update_job(upd_id, current).await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;
            }
        }

        // If sub-job: FAIL the parent task
        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent = self.ds.get_task_by_id(parent_id.clone()).await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("parent task not found: {parent_id}")))?;

                let mut updated_parent = parent;
                updated_parent.state = TASK_STATE_FAILED.clone();
                updated_parent.failed_at = job.failed_at;
                updated_parent.error = job.error.clone();

                return self.broker.publish_task(QUEUE_ERROR.to_string(), &updated_parent).await
                    .map_err(|e| HandlerError::Broker(e.to_string()));
            }
        }

        // Cancel all active tasks
        self.cancel_active_tasks(&job_id).await?;

        // Re-fetch job and publish failed event if still FAILED
        let refreshed = self.ds.get_job_by_id(job_id).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(refreshed) = refreshed {
            if refreshed.state == JOB_STATE_FAILED {
                job.state = refreshed.state.clone();
                job.error = refreshed.error.clone();
                let event = serde_json::to_value(&refreshed)
                    .map_err(|e| HandlerError::Handler(e.to_string()))?;
                self.broker.publish_event(TOPIC_JOB_FAILED.to_string(), event).await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Cancel a job: mark as CANCELLED, notify parent, cancel active tasks.
    ///
    /// Parity with Go `func (h *cancelHandler) handle(...)`.
    async fn cancel_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        // Only cancel if RUNNING or SCHEDULED
        let current = self.ds.get_job_by_id(job_id.clone()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let Some(mut current) = current else {
            return Ok(());
        };

        if current.state != JOB_STATE_RUNNING && current.state != JOB_STATE_SCHEDULED {
            return Ok(());
        }

        current.state = JOB_STATE_CANCELLED.to_string();
        job.state = current.state.clone();
        let upd_id = current.id.clone().unwrap_or_default();
        self.ds.update_job(upd_id, current).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // If sub-job: notify parent job to cancel
        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent_task = self.ds.get_task_by_id(parent_id.clone()).await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("parent task not found: {parent_id}")))?;

                let parent_job_id = parent_task.job_id.clone()
                    .ok_or_else(|| HandlerError::Handler("parent task has no job_id".to_string()))?;

                let mut parent_job = self.ds.get_job_by_id(parent_job_id.clone()).await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("parent job not found: {parent_job_id}")))?;

                parent_job.state = JOB_STATE_CANCELLED.to_string();
                self.broker.publish_job(&parent_job).await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        // Cancel all active tasks
        self.cancel_active_tasks(&job_id).await
    }

    /// Cancel all currently active tasks for a job.
    ///
    /// For each active task: marks it CANCELLED in the datastore.
    /// If the task is a sub-job, publishes a cancel event for it.
    /// If the task has a node, publishes the cancellation to the node's queue.
    /// Parity with Go `func cancelActiveTasks(...)`.
    async fn cancel_active_tasks(&self, job_id: &str) -> Result<(), HandlerError> {
        let tasks = self.ds.get_active_tasks(job_id.to_string()).await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        for mut task in tasks {
            task.state = TASK_STATE_CANCELLED.clone();
            let task_id = task.id.clone().unwrap_or_default();
            let task_clone = task.clone();
            self.ds.update_task(task_id, task_clone).await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;

            if let Some(ref sj) = task.subjob {
                if let Some(ref sj_id) = sj.id {
                    if !sj_id.is_empty() {
                        let subjob = self.ds.get_job_by_id(sj_id.clone()).await
                            .map_err(|e| HandlerError::Datastore(e.to_string()))?
                            .ok_or_else(|| HandlerError::NotFound(format!("sub-job not found: {sj_id}")))?;

                        let mut cancelled_subjob = subjob;
                        cancelled_subjob.state = JOB_STATE_CANCELLED.to_string();
                        self.broker.publish_job(&cancelled_subjob).await
                            .map_err(|e| HandlerError::Broker(e.to_string()))?;
                    }
                }
            } else if let Some(ref node_id) = task.node_id {
                if !node_id.is_empty() {
                    let node = self.ds.get_node_by_id(node_id.clone()).await
                        .map_err(|e| HandlerError::Datastore(e.to_string()))?
                        .ok_or_else(|| HandlerError::NotFound(format!("node not found: {node_id}")))?;

                    let queue = node.queue.clone().unwrap_or_default();
                    if !queue.is_empty() {
                        self.broker.publish_task(queue, &task).await
                            .map_err(|e| HandlerError::Broker(e.to_string()))?;
                    }
                }
            }
        }

        Ok(())
    }

    // ---- Pure state transition (synchronous, for testing / backward compat) ----

    /// Process a job state transition based on the current state.
    ///
    /// Returns the transition type without performing I/O.
    /// Useful for testing and for callers that need to inspect transitions
    /// before executing them.
    pub fn process_state_transition(
        &self,
        job: &Job,
    ) -> Result<JobStateTransition, HandlerError> {
        let transition = match job.state.as_str() {
            s if s == JOB_STATE_PENDING => JobStateTransition::Start,
            s if s == JOB_STATE_SCHEDULED => JobStateTransition::Schedule,
            s if s == JOB_STATE_RUNNING => JobStateTransition::Run,
            s if s == JOB_STATE_CANCELLED => JobStateTransition::Cancel,
            s if s == JOB_STATE_COMPLETED => JobStateTransition::Complete,
            s if s == JOB_STATE_FAILED => JobStateTransition::Fail,
            s if s == JOB_STATE_RESTART => JobStateTransition::Restart,
            other => {
                return Err(HandlerError::InvalidState(format!(
                    "unknown job state: {other}"
                )));
            }
        };
        Ok(transition)
    }
}



/// Represents a job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStateTransition {
    /// Job is being started
    Start,
    /// Job is being scheduled
    Schedule,
    /// Job is running
    Run,
    /// Job is being cancelled
    Cancel,
    /// Job has completed
    Complete,
    /// Job has failed
    Fail,
    /// Job is being restarted
    Restart,
}

impl std::fmt::Display for JobStateTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStateTransition::Start => write!(f, "START"),
            JobStateTransition::Schedule => write!(f, "SCHEDULE"),
            JobStateTransition::Run => write!(f, "RUN"),
            JobStateTransition::Cancel => write!(f, "CANCEL"),
            JobStateTransition::Complete => write!(f, "COMPLETE"),
            JobStateTransition::Fail => write!(f, "FAIL"),
            JobStateTransition::Restart => write!(f, "RESTART"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert("inputs".to_string(), serde_json::json!({"var1": "hello"}));
        map
    }

    /// Create a test handler with in-memory datastore and broker implementations.
    fn new_test_handler() -> JobHandler {
        JobHandler::new(
            Arc::new(MockDatastore),
            Arc::new(MockBroker),
        )
    }

    /// Minimal mock datastore for testing state transitions.
    struct MockDatastore;

    impl Datastore for MockDatastore {
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
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_next_task(&self, _parent_task_id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(&self, _part: tork::task::TaskLogPart) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(&self, _task_id: String, _q: String, _page: i64, _size: i64) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 1, size: 20 }) })
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
            Box::pin(async { Ok(Vec::new()) })
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
        fn get_job_log_parts(&self, _job_id: String, _q: String, _page: i64, _size: i64) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 1, size: 20 }) })
        }
        fn get_jobs(&self, _current_user: String, _q: String, _page: i64, _size: i64) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 1, size: 20 }) })
        }
        fn create_scheduled_job(&self, _sj: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_scheduled_jobs(&self, _current_user: String, _page: i64, _size: i64) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 1, size: 20 }) })
        }
        fn get_scheduled_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(&self, _id: String, _sj: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
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
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(Vec::new()) })
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

    /// Minimal mock broker for testing.
    struct MockBroker;

    impl Broker for MockBroker {
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
        fn publish_job(&self, _job: &Job) -> tork::broker::BoxedFuture<()> {
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
            Box::pin(async { Ok(Vec::new()) })
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

    // ---- Eval helper tests ----

    #[test]
    fn test_evaluate_template_no_expression() {
        let ctx = test_context();
        assert_eq!(evaluate_template("plain text", &ctx).unwrap(), "plain text");
    }

    #[test]
    fn test_evaluate_template_with_expression() {
        let ctx = test_context();
        assert_eq!(
            evaluate_template("{{ inputs.var1 }}", &ctx).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_evaluate_template_empty() {
        let ctx = test_context();
        assert_eq!(evaluate_template("", &ctx).unwrap(), "");
    }

    #[test]
    fn test_evaluate_template_bad_expression() {
        let ctx = test_context();
        assert!(evaluate_template("{{ bad_expression }}", &ctx).is_err());
    }

    #[test]
    fn test_evaluate_task_with_bad_env() {
        let ctx = test_context();
        let mut task = Task::default();
        task.env = Some(HashMap::from([(
            "SOMEVAR".to_string(),
            "{{ bad_expression }}".to_string(),
        )]));
        assert!(evaluate_task(&task, &ctx).is_err());
    }

    #[test]
    fn test_evaluate_task_with_good_env() {
        let ctx = test_context();
        let mut task = Task::default();
        task.env = Some(HashMap::from([(
            "SOMEVAR".to_string(),
            "{{ inputs.var1 }}".to_string(),
        )]));
        let result = evaluate_task(&task, &ctx).unwrap();
        assert_eq!(
            result.env.as_ref().and_then(|e| e.get("SOMEVAR")).map(String::as_str),
            Some("hello")
        );
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1m").unwrap(), StdDuration::from_secs(60));
        assert_eq!(parse_duration("2h").unwrap(), StdDuration::from_secs(7200));
        assert_eq!(parse_duration("30s").unwrap(), StdDuration::from_secs(30));
        assert_eq!(parse_duration("1h30m").unwrap(), StdDuration::from_secs(5400));
        assert!(parse_duration("invalid").is_err());
    }

    // ---- State transition tests ----

    #[test]
    fn test_process_state_transition_pending() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_PENDING.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Start);
    }

    #[test]
    fn test_process_state_transition_completed() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_COMPLETED.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Complete);
    }

    #[test]
    fn test_process_state_transition_failed() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_FAILED.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Fail);
    }

    #[test]
    fn test_process_state_transition_restart() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_RESTART.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Restart);
    }

    #[test]
    fn test_process_state_transition_unknown() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = "UNKNOWN".to_string();
        let result = handler.process_state_transition(&job);
        assert!(result.is_err());
    }

    // ---- Display test ----

    #[test]
    fn test_display_job_state_transition() {
        assert_eq!(JobStateTransition::Start.to_string(), "START");
        assert_eq!(JobStateTransition::Cancel.to_string(), "CANCEL");
        assert_eq!(JobStateTransition::Restart.to_string(), "RESTART");
    }
}
