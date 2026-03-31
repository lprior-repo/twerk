//! Task evaluation.
//!
//! Provides recursive task template evaluation for all task fields.

use std::collections::HashMap;

use super::template::evaluate_template;
use crate::eval::EvalError;
use crate::task::Task;

/// Evaluates all template expressions within a task recursively.
///
/// This function processes:
/// - String fields (name, var, image, queue, if)
/// - Maps (env, files)
/// - Task lists (pre, post, sidecars)
/// - Nested structures (parallel, each, subjob)
/// - Arrays (cmd, entrypoint)
///
/// # Note
/// The `run` field is NOT evaluated - it's treated as raw shell command
/// to match Go implementation behavior.
///
/// # Arguments
/// * `task` - The task to evaluate
/// * `context` - Variable bindings for template evaluation
///
/// # Returns
/// A new task with all template expressions evaluated.
#[allow(clippy::too_many_lines, clippy::implicit_hasher)]
pub fn evaluate_task(
    task: &Task,
    context: &HashMap<String, serde_json::Value>,
) -> Result<Task, EvalError> {
    // Evaluate an optional string field through the template engine.
    let eval_field = |field: &Option<String>| -> Result<Option<String>, EvalError> {
        field
            .as_ref()
            .map_or(Ok(None), |s| evaluate_template(s, context).map(Some))
    };

    // Evaluate a map of string → string through the template engine.
    let eval_map =
        |map: &Option<HashMap<String, String>>| -> Result<HashMap<String, String>, EvalError> {
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
        };

    // Evaluate a list of tasks recursively.
    let eval_tasks = |tasks: &Option<Vec<Task>>| -> Result<Option<Vec<Task>>, EvalError> {
        tasks
            .as_ref()
            .map(|ts| {
                ts.iter()
                    .map(|t| evaluate_task(t, context))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
    };

    // -- String fields --
    // NOTE: Go does NOT evaluate the `run` field - it's treated as raw shell command
    let name = eval_field(&task.name)?;
    let var = eval_field(&task.var)?;
    let image = eval_field(&task.image)?;
    let run = task.run.clone();
    let queue = eval_field(&task.queue)?;
    let r#if = eval_field(&task.r#if)?;

    // -- Maps --
    let env = eval_map(&task.env)?;
    let files = eval_map(&task.files)?;

    // -- Task lists (recursive) --
    let pre = eval_tasks(&task.pre)?;
    let post = eval_tasks(&task.post)?;
    let sidecars = eval_tasks(&task.sidecars)?;

    // -- Parallel --
    let parallel = task
        .parallel
        .as_ref()
        .map(|par| {
            let tasks = eval_tasks(&par.tasks)?;
            Ok(crate::task::ParallelTask {
                tasks,
                completions: par.completions,
            })
        })
        .transpose()?;

    // -- Each --
    let each = task
        .each
        .as_ref()
        .map(|each| {
            let var = eval_field(&each.var)?;
            let list = eval_field(&each.list)?;
            let inner_task = each
                .task
                .as_ref()
                .map(|t| evaluate_task(t, context))
                .transpose()?;
            Ok(crate::task::EachTask {
                var,
                list,
                task: inner_task.map(Box::new),
                size: each.size,
                completions: each.completions,
                concurrency: each.concurrency,
                index: each.index,
            })
        })
        .transpose()?;

    // -- CMD --
    let cmd = task
        .cmd
        .as_ref()
        .map(|c| {
            c.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // -- Entrypoint --
    let entrypoint = task
        .entrypoint
        .as_ref()
        .map(|ep| {
            ep.iter()
                .map(|s| evaluate_template(s, context))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    // -- SubJob --
    let subjob = task
        .subjob
        .as_ref()
        .map(|sj| {
            let subjob_name = eval_field(&sj.name)?;

            let inputs = eval_map(&sj.inputs)?;
            let secrets = eval_map(&sj.secrets)?;

            // Evaluate subjob tasks recursively (Go parity)
            let subjob_tasks = eval_tasks(&sj.tasks)?;

            let webhooks = sj
                .webhooks
                .as_ref()
                .map(|whs| {
                    whs.iter()
                        .map(|wh| {
                            let url = eval_field(&wh.url)?;
                            let headers = eval_map(&wh.headers)?;
                            let wh_if = eval_field(&wh.r#if)?;
                            Ok(crate::webhook::Webhook {
                                url,
                                headers: if headers.is_empty() {
                                    None
                                } else {
                                    Some(headers)
                                },
                                event: wh.event.clone(),
                                r#if: wh_if,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;

            Ok(crate::task::SubJobTask {
                id: sj.id.clone(),
                name: subjob_name,
                description: sj.description.clone(),
                tasks: subjob_tasks,
                inputs: if inputs.is_empty() {
                    None
                } else {
                    Some(inputs)
                },
                secrets: if secrets.is_empty() {
                    None
                } else {
                    Some(secrets)
                },
                auto_delete: sj.auto_delete.clone(),
                output: sj.output.clone(),
                detached: sj.detached,
                webhooks: webhooks.filter(|w| !w.is_empty()),
            })
        })
        .transpose()?;

    // -- Assemble new task (structural copy) --
    Ok(Task {
        id: task.id.clone(),
        job_id: task.job_id.clone(),
        parent_id: task.parent_id.clone(),
        position: task.position,
        name,
        description: task.description.clone(),
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
        timeout: task.timeout.clone(),
        result: task.result.clone(),
        var,
        r#if,
        parallel,
        each: each.map(Box::new),
        subjob,
        gpus: task.gpus.clone(),
        tags: task.tags.clone(),
        workdir: task.workdir.clone(),
        priority: task.priority,
        progress: task.progress,
        probe: task.probe.clone(),
    })
}
