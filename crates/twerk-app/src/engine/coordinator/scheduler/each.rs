//! Each-loop task scheduling logic.

use super::Scheduler;
use anyhow::Result;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use std::collections::HashMap;
use twerk_core::eval::{evaluate_expr, evaluate_task};
use twerk_core::task::Task;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

struct SubtaskContext<'a> {
    template: &'a Task,
    job_ctx: &'a HashMap<String, serde_json::Value>,
    var_name: &'a str,
    job_id: &'a str,
    task_id: &'a str,
    now: time::OffsetDateTime,
}

impl Scheduler {
    /// Schedules tasks from an each-loop task definition.
    /// # Errors
    /// Returns error if list evaluation or task creation fails.
    pub async fn schedule_each_task(&self, task: Task) -> Result<()> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("task ID required for each scheduling"))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("job ID required for each scheduling"))?;
        let now = time::OffsetDateTime::now_utc();

        tracing::warn!(task_id = %task_id, "SCHEDULE_EACH_TASK called");

        let job = self.ds.get_job_by_id(job_id).await?;
        let job_ctx_map = job
            .context
            .as_ref()
            .map(twerk_core::job::JobContext::as_map)
            .unwrap_or_default();

        let each = task
            .each
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing each config"))?;
        let list_expr = each.list.as_deref().unwrap_or_default();

        let list_val = Self::eval_each_list(list_expr, &job_ctx_map)?;
        let list = list_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("each list must be an array"))?;
        let size = list.len() as i64;

        self.ds
            .update_task(
                task_id,
                Box::new(move |u| {
                    let updated_each = u
                        .each
                        .map(|e| Box::new(twerk_core::task::EachTask { size, ..*e }));
                    Ok(Task {
                        state: twerk_core::task::TaskState::Running,
                        started_at: Some(now),
                        each: updated_each,
                        ..u
                    })
                }),
            )
            .await?;

        let template = each
            .task
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing each task template"))?;
        self.spawn_each_tasks(template, list, &job_ctx_map, task_id, job_id, now)
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
            evaluate_expr(list_expr, job_ctx)
                .map_err(|e| anyhow::anyhow!("failed to evaluate each list: {e}"))?
        };

        if let Some(s) = list_val.as_str() {
            if let Ok(json_list) = serde_json::from_str(s) {
                return Ok(json_list);
            }
        }
        Ok(list_val)
    }

    async fn spawn_each_tasks(
        &self,
        template: &Task,
        list: &[serde_json::Value],
        job_ctx: &HashMap<String, serde_json::Value>,
        task_id: &str,
        job_id: &str,
        now: time::OffsetDateTime,
    ) -> Result<()> {
        let var_name = "item";
        let ctx = SubtaskContext {
            template,
            job_ctx,
            var_name,
            job_id,
            task_id,
            now,
        };

        tracing::warn!(list_len = list.len(), "SPAWN_EACH_TASKS building subtasks");

        let subtasks: Vec<_> = list
            .iter()
            .enumerate()
            .par_bridge()
            .map(|(ix, item)| Self::build_subtask(ix, item, &ctx))
            .collect::<Result<Vec<_>>>()?;

        tracing::warn!(
            count = subtasks.len(),
            "SPAWN_EACH_TASKS subtasks built, creating in DB"
        );

        self.publish_and_handle_errors(&subtasks).await
    }

    fn build_subtask(ix: usize, item: &serde_json::Value, ctx: &SubtaskContext) -> Result<Task> {
        let cx = Self::build_context(item, ctx.job_ctx, ctx.var_name, ix);

        let evaluated = evaluate_task(ctx.template, &cx)
            .map_err(|e| anyhow::anyhow!("failed to evaluate each item task: {e}"))?;

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
        let mut m = job_ctx.clone();
        m.insert(
            format!("{var_name}_index"),
            serde_json::Value::String(ix.to_string()),
        );
        if let Some(obj) = item.as_object() {
            for (k, v) in obj {
                let flat_key = format!("{var_name}_value_{k}");
                m.insert(flat_key, v.clone());
            }
            m.insert(var_name.to_string(), item.clone());
        } else {
            m.insert(var_name.to_string(), item.clone());
            m.insert(format!("{var_name}_value"), item.clone());
        }
        m
    }

    async fn publish_and_handle_errors(&self, subtasks: &[Task]) -> Result<()> {
        if subtasks.is_empty() {
            return Ok(());
        }

        self.ds.create_tasks(subtasks).await?;

        tracing::warn!("SPAWN_EACH_TASKS DB insert OK, publishing to broker");
        if let Err(e) = self
            .broker
            .publish_tasks(QUEUE_PENDING.to_string(), subtasks)
            .await
        {
            self.rollback_failed_tasks(subtasks, &e).await;
            return Err(e);
        }

        Ok(())
    }

    async fn rollback_failed_tasks(&self, subtasks: &[Task], error: &anyhow::Error) {
        let error_msg = format!("broker publish failed: {error}");
        let compensating: Vec<_> = subtasks
            .iter()
            .filter_map(|s| s.id.as_deref())
            .map(|id| {
                let msg = error_msg.clone();
                self.ds.update_task(
                    id,
                    Box::new(move |t| {
                        Ok(Task {
                            state: twerk_core::task::TaskState::Failed,
                            error: Some(msg),
                            ..t
                        })
                    }),
                )
            })
            .collect();
        let _ = futures_util::future::join_all(compensating).await;
    }
}
