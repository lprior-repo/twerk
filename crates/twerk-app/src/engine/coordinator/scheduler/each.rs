//! Each-loop task scheduling logic.

use super::shared::{create_and_publish_subtasks, job_context_map, scheduler_ids};
use super::Scheduler;
use super::SchedulerError;
use anyhow::Result;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use std::collections::HashMap;
use twerk_core::eval::{evaluate_expr, evaluate_task};
use twerk_core::task::Task;
use twerk_core::uuid::new_short_uuid;

struct SubtaskContext<'a> {
    template: &'a Task,
    job_ctx: &'a HashMap<String, serde_json::Value>,
    var_name: &'a str,
    job_id: &'a str,
    task_id: &'a str,
    now: time::OffsetDateTime,
}

struct EachSpawnRequest<'a> {
    list: &'a [serde_json::Value],
    context: SubtaskContext<'a>,
}

impl Scheduler {
    /// Schedules tasks from an each-loop task definition.
    /// # Errors
    /// Returns error if list evaluation or task creation fails.
    pub async fn schedule_each_task(&self, task: Task) -> Result<()> {
        let ids = scheduler_ids(&task, "each")?;
        let now = time::OffsetDateTime::now_utc();

        tracing::warn!(task_id = %ids.task_id, "SCHEDULE_EACH_TASK called");

        let job = self.ds.get_job_by_id(ids.job_id).await?;
        let job_ctx_map = job_context_map(&job);

        let each = task
            .each
            .as_ref()
            .ok_or_else(|| SchedulerError::MissingConfig {
                scheduler: "each".to_string(),
            })?;
        let list_expr = each.list.as_deref().unwrap_or_default();

        let list_val = Self::eval_each_list(list_expr, &job_ctx_map)?;
        let list = list_val
            .as_array()
            .ok_or(SchedulerError::EachListMustBeArray)?;
        let size = list.len() as i64;

        self.mark_each_task_running(ids.task_id, size, now).await?;

        let template = each
            .task
            .as_ref()
            .ok_or(SchedulerError::MissingEachTemplate)?;
        self.spawn_each_tasks(EachSpawnRequest {
            list,
            context: SubtaskContext {
                template,
                job_ctx: &job_ctx_map,
                var_name: "item",
                job_id: ids.job_id,
                task_id: ids.task_id,
                now,
            },
        })
        .await
    }

    /// Evaluates an each-loop list expression to a JSON array.
    pub(super) fn eval_each_list(
        list_expr: &str,
        job_ctx: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let list_val = if list_expr.trim().starts_with('[') {
            serde_json::from_str(list_expr).map_or_else(
                |_| serde_json::Value::String(list_expr.to_string()),
                serde_json::Value::Array,
            )
        } else {
            evaluate_expr(list_expr, job_ctx).map_err(|e| SchedulerError::Evaluation {
                context: "each list".to_string(),
                error: e.to_string(),
            })?
        };

        if let Some(s) = list_val.as_str() {
            if let Ok(json_list) = serde_json::from_str(s) {
                return Ok(json_list);
            }
        }
        Ok(list_val)
    }

    async fn mark_each_task_running(
        &self,
        task_id: &str,
        size: i64,
        now: time::OffsetDateTime,
    ) -> Result<()> {
        self.ds
            .update_task(
                task_id,
                Box::new(move |task| {
                    let updated_each = task
                        .each
                        .map(|each| Box::new(twerk_core::task::EachTask { size, ..*each }));
                    Ok(Task {
                        state: twerk_core::task::TaskState::Running,
                        started_at: Some(now),
                        each: updated_each,
                        ..task
                    })
                }),
            )
            .await
            .map_err(anyhow::Error::from)
    }

    async fn spawn_each_tasks(&self, request: EachSpawnRequest<'_>) -> Result<()> {
        tracing::warn!(
            list_len = request.list.len(),
            "SPAWN_EACH_TASKS building subtasks"
        );

        let subtasks: Vec<_> = request
            .list
            .iter()
            .enumerate()
            .par_bridge()
            .map(|(ix, item)| Self::build_subtask(ix, item, &request.context))
            .collect::<Result<Vec<_>>>()?;

        tracing::warn!(
            count = subtasks.len(),
            "SPAWN_EACH_TASKS subtasks built, creating in DB"
        );

        create_and_publish_subtasks(self, &subtasks).await
    }

    fn build_subtask(ix: usize, item: &serde_json::Value, ctx: &SubtaskContext) -> Result<Task> {
        let cx = Self::build_context(item, ctx.job_ctx, ctx.var_name, ix);

        let evaluated =
            evaluate_task(ctx.template, &cx).map_err(|e| SchedulerError::Evaluation {
                context: "each item task".to_string(),
                error: e.to_string(),
            })?;

        Ok(Task {
            id: Some(new_short_uuid().into()),
            job_id: Some(twerk_core::id::JobId::new(ctx.job_id.to_string())?),
            parent_id: Some(ctx.task_id.to_string().into()),
            state: twerk_core::task::TaskState::Pending,
            created_at: Some(ctx.now),
            ..evaluated
        })
    }

    fn build_context(
        item: &serde_json::Value,
        job_ctx: &HashMap<String, serde_json::Value>,
        var_name: &str,
        ix: usize,
    ) -> HashMap<String, serde_json::Value> {
        job_ctx
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .chain(std::iter::once((
                format!("{var_name}_index"),
                serde_json::Value::String(ix.to_string()),
            )))
            .chain(Self::item_context_entries(item, var_name))
            .collect()
    }

    fn item_context_entries(
        item: &serde_json::Value,
        var_name: &str,
    ) -> impl Iterator<Item = (String, serde_json::Value)> {
        item.as_object().map_or_else(
            || {
                std::iter::once((var_name.to_string(), item.clone()))
                    .chain(std::iter::once((format!("{var_name}_value"), item.clone())))
                    .collect::<Vec<_>>()
                    .into_iter()
            },
            |object| {
                std::iter::once((var_name.to_string(), item.clone()))
                    .chain(
                        object
                            .iter()
                            .map(|(key, value)| (format!("{var_name}_value_{key}"), value.clone())),
                    )
                    .collect::<Vec<_>>()
                    .into_iter()
            },
        )
    }
}
