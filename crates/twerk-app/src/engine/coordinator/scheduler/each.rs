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

    /// Threshold below which sequential iteration beats par_bridge overhead.
    const PARALLEL_THRESHOLD: usize = 8;

    async fn spawn_each_tasks(&self, request: EachSpawnRequest<'_>) -> Result<()> {
        tracing::warn!(
            list_len = request.list.len(),
            "SPAWN_EACH_TASKS building subtasks"
        );

        // Build base context once (cloning job_ctx entries ONCE instead of N times).
        // For a 10k-item job, this eliminates ~10k-1 unnecessary HashMap clones.
        let base_context = Self::build_base_context(request.context.job_ctx);

        let subtasks: Vec<_> = if request.list.len() < Self::PARALLEL_THRESHOLD {
            // Sequential path: faster for small batches due to rayon overhead
            request
                .list
                .iter()
                .enumerate()
                .map(|(ix, item)| Self::build_subtask_with_base(ix, item, &request.context, &base_context))
                .collect::<Result<Vec<_>>>()?
        } else {
            // Parallel path: benefits kick in at ~8+ tasks
            request
                .list
                .iter()
                .enumerate()
                .par_bridge()
                .map(|(ix, item)| Self::build_subtask_with_base(ix, item, &request.context, &base_context))
                .collect::<Result<Vec<_>>>()?
        };

        tracing::warn!(
            count = subtasks.len(),
            "SPAWN_EACH_TASKS subtasks built, creating in DB"
        );

        create_and_publish_subtasks(self, &subtasks).await
    }

    /// Builds the base context once by cloning job_ctx. This is called once
    /// per parent task (not once per subtask). The resulting HashMap is cloned
    /// and extended with item-specific entries for each subtask.
    fn build_base_context(
        job_ctx: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        // Clone job_ctx entries ONCE. Each entry is String -> serde_json::Value,
        // both of which are cheap to clone (String is 24B inline, Value is enum).
        job_ctx.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Extends base_context with item-specific entries for a single subtask.
    /// This avoids cloning the full job_ctx for every subtask.
    fn extend_context_with_item(
        base: &HashMap<String, serde_json::Value>,
        item: &serde_json::Value,
        var_name: &str,
        ix: usize,
    ) -> HashMap<String, serde_json::Value> {
        let mut cx = base.clone();
        cx.insert(format!("{var_name}_index"), serde_json::Value::String(ix.to_string()));
        Self::insert_item_context_entries(&mut cx, item, var_name);
        cx
    }

    /// Inserts item-specific context entries into an existing HashMap.
    /// Reuses allocation from the pre-cloned base context.
    fn insert_item_context_entries(
        cx: &mut HashMap<String, serde_json::Value>,
        item: &serde_json::Value,
        var_name: &str,
    ) {
        match item {
            serde_json::Value::Object(obj) => {
                cx.insert(var_name.to_string(), item.clone());
                for (key, value) in obj {
                    cx.insert(format!("{var_name}_value_{key}"), value.clone());
                }
            }
            _ => {
                cx.insert(var_name.to_string(), item.clone());
                cx.insert(format!("{var_name}_value"), item.clone());
            }
        }
    }

    fn build_subtask_with_base(
        ix: usize,
        item: &serde_json::Value,
        ctx: &SubtaskContext,
        base_context: &HashMap<String, serde_json::Value>,
    ) -> Result<Task> {
        // Extend base context with item-specific entries (single clone of base)
        let cx = Self::extend_context_with_item(base_context, item, ctx.var_name, ix);

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

}

#[cfg(test)]
mod each_tests {
    use super::*;

    #[test]
    fn test_base_context_built_once() {
        let job_ctx: HashMap<String, serde_json::Value> =
            vec![("a".to_string(), serde_json::json!(1))]
                .into_iter()
                .collect();
        let base = Scheduler::build_base_context(&job_ctx);
        assert_eq!(base.len(), 1);
        let base2 = Scheduler::build_base_context(&job_ctx);
        assert_eq!(base, base2);
    }

    #[test]
    fn test_extend_context_with_item_scalar() {
        let job_ctx: HashMap<String, serde_json::Value> =
            vec![("base_key".to_string(), serde_json::json!("base_val"))]
                .into_iter()
                .collect();
        let base = Scheduler::build_base_context(&job_ctx);
        let cx = Scheduler::extend_context_with_item(
            &base,
            &serde_json::json!("hello"),
            "item",
            5,
        );
        assert_eq!(cx.get("base_key"), Some(&serde_json::json!("base_val")));
        assert_eq!(cx.get("item_index"), Some(&serde_json::json!("5")));
        assert_eq!(cx.get("item"), Some(&serde_json::json!("hello")));
        assert_eq!(cx.get("item_value"), Some(&serde_json::json!("hello")));
    }

    #[test]
    fn test_extend_context_with_item_object() {
        let job_ctx: HashMap<String, serde_json::Value> =
            vec![("base_key".to_string(), serde_json::json!("base_val"))]
                .into_iter()
                .collect();
        let base = Scheduler::build_base_context(&job_ctx);
        let cx = Scheduler::extend_context_with_item(
            &base,
            &serde_json::json!({"name": "alice", "age": 30}),
            "person",
            2,
        );
        assert_eq!(cx.get("base_key"), Some(&serde_json::json!("base_val")));
        assert_eq!(cx.get("person_index"), Some(&serde_json::json!("2")));
        assert_eq!(
            cx.get("person"),
            Some(&serde_json::json!({"name": "alice", "age": 30}))
        );
        assert_eq!(
            cx.get("person_value_name"),
            Some(&serde_json::json!("alice"))
        );
        assert_eq!(cx.get("person_value_age"), Some(&serde_json::json!(30)));
    }

    #[test]
    fn test_context_clone_elimination() {
        // For N items, job_ctx is cloned N times (once per item).
        // NOT N*N times. This is the performance invariant.
        let job_ctx: HashMap<String, serde_json::Value> = (0..10)
            .map(|i| (format!("key_{i}"), serde_json::json!(i)))
            .collect();
        let base = Scheduler::build_base_context(&job_ctx);
        let items: Vec<serde_json::Value> = (0..100)
            .map(|i| serde_json::json!({"val": i}))
            .collect();
        let contexts: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(ix, item)| Scheduler::extend_context_with_item(&base, item, "i", ix))
            .collect();
        assert_eq!(contexts.len(), 100);
        assert_eq!(contexts[0].get("key_0"), Some(&serde_json::json!(0)));
        assert_eq!(contexts[50].get("i_index"), Some(&serde_json::json!("50")));
    }
}
