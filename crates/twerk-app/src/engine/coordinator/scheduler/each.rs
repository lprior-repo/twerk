//! Each-loop task scheduling logic.

use super::Scheduler;
use anyhow::Result;
use std::collections::HashMap;
use twerk_core::eval::{evaluate_expr, evaluate_task};
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

impl Scheduler {
    /// Schedules tasks from an each-loop task definition.
    /// # Errors
    /// Returns error if list evaluation or task creation fails.
    pub async fn schedule_each_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("task ID required for each scheduling"))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("job ID required for each scheduling"))?;
        let now = time::OffsetDateTime::now_utc();

        let job = self.ds.get_job_by_id(&job_id).await?;
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
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
                    u.started_at = Some(now);
                    if let Some(ref mut e) = u.each {
                        e.size = size;
                    }
                    Ok(u)
                }),
            )
            .await?;

        let template = each
            .task
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing each task template"))?;
        self.spawn_each_tasks(template, list, &job_ctx_map, &task_id, &job_id, now)
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
        template: &twerk_core::task::Task,
        list: &[serde_json::Value],
        job_ctx: &HashMap<String, serde_json::Value>,
        task_id: &str,
        job_id: &str,
        now: time::OffsetDateTime,
    ) -> Result<()> {
        let var_name = "item";

        let subtasks: Vec<_> = list
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let mut cx = job_ctx.clone();
                cx.insert(
                    var_name.to_string(),
                    serde_json::json!({
                        "index": ix.to_string(),
                        "value": item
                    }),
                );

                let mut et = (*template).clone();
                et = evaluate_task(&et, &cx)
                    .map_err(|e| anyhow::anyhow!("failed to evaluate each item task: {e}"))?;

                et.id = Some(new_short_uuid().into());
                et.job_id = Some(job_id.to_string().into());
                et.parent_id = Some(task_id.to_string().into());
                et.state = twerk_core::task::TASK_STATE_PENDING.to_string();
                et.created_at = Some(now);
                Ok(et)
            })
            .collect::<Result<Vec<_>>>()?;

        if !subtasks.is_empty() {
            self.ds.create_tasks(&subtasks).await?;
            self.broker
                .publish_tasks(QUEUE_PENDING.to_string(), &subtasks)
                .await?;
        }

        Ok(())
    }
}
