//! Template evaluation helpers for task completion.

use std::collections::HashMap;

use evalexpr::{eval_with_context, ContextWithMutableVariables, HashMapContext, Value as EvalValue};
use regex::Regex;
use tork::task::Task;

#[allow(clippy::expect_used)]
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid template regex"));

pub fn json_to_eval_value(json: &serde_json::Value) -> Result<EvalValue, String> {
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

pub fn create_eval_context(
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

pub fn flatten_json_value(
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

pub fn evaluate_expr(
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

pub fn evaluate_template(
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

pub fn eval_field(
    field: &Option<String>,
    context: &HashMap<String, serde_json::Value>,
) -> Result<Option<String>, String> {
    field
        .as_ref()
        .map_or(Ok(None), |s| evaluate_template(s, context).map(Some))
}

pub fn eval_map(
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

pub fn eval_tasks(
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

#[allow(clippy::too_many_lines)]
pub fn evaluate_task(
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

    let cmd = task.cmd.as_ref().map(|cmds| {
        cmds.iter().map(|s| evaluate_template(s, context)).collect::<Result<Vec<_>, _>>()
    }).transpose()?;

    let entrypoint = task.entrypoint.as_ref().map(|eps| {
        eps.iter().map(|s| evaluate_template(s, context)).collect::<Result<Vec<_>, _>>()
    }).transpose()?;

    let parallel = task.parallel.as_ref().map(|par| {
        let tasks = eval_tasks(&par.tasks, context)?;
        Ok::<tork::task::ParallelTask, String>(tork::task::ParallelTask {
            tasks,
            completions: par.completions,
        })
    }).transpose()?;

    let each = task.each.as_ref().map(|each| {
        let var = eval_field(&each.var, context)?;
        let list = eval_field(&each.list, context)?;
        let inner_task = each.task.as_ref().map(|t| evaluate_task(t, context)).transpose()?;
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
