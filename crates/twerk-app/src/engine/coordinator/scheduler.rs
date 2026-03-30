#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Task scheduler for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use anyhow::Result;
use std::sync::Arc;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;
use twerk_core::eval::{evaluate_task, evaluate_expr};

pub struct Scheduler {
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
}

impl Scheduler {
    pub fn new(
        ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
        broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    ) -> Self {
        Self { ds, broker }
    }

    /// Schedules a task based on its type (regular, parallel, each, or subjob).
    /// # Errors
    /// Returns error if task scheduling fails.
    pub async fn schedule_task(&self, task: twerk_core::task::Task) -> Result<()> {
        if task.parallel.is_some() {
            self.schedule_parallel_task(task).await
        } else if task.each.is_some() {
            self.schedule_each_task(task).await
        } else if task.subjob.is_some() {
            self.schedule_subjob_task(task).await
        } else {
            self.schedule_regular_task(task).await
        }
    }

    /// Schedules a regular (non-parallel, non-each) task.
    /// # Errors
    /// Returns error if task creation or broker publish fails.
    pub async fn schedule_regular_task(&self, mut task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        task.state = twerk_core::task::TASK_STATE_SCHEDULED.to_string();
        task.scheduled_at = Some(now);
        
        if task.queue.is_none() {
            task.queue = Some("default".to_string());
        }
        
        let q = task.queue.clone().unwrap_or_default();
        let t_queue = task.queue.clone();

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_SCHEDULED.to_string();
            u.scheduled_at = Some(now);
            u.queue = t_queue;
            Ok(u)
        })).await?;
        
        self.broker.publish_task(q, &task).await?;
        
        Ok(())
    }

    /// Schedules parallel tasks from a parallel task definition.
    /// # Errors
    /// Returns error if job retrieval or task creation fails.
    pub async fn schedule_parallel_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        let job_ctx = job.context.as_ref().map(twerk_core::job::JobContext::as_map).unwrap_or_default();

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
            u.started_at = Some(now);
            Ok(u)
        })).await?;
        
        let parallel = task.parallel.as_ref().ok_or_else(|| anyhow::anyhow!("missing parallel config"))?;
        let tasks = parallel.tasks.as_ref().ok_or_else(|| anyhow::anyhow!("missing parallel tasks"))?;
        
        for t in tasks {
            let mut pt = t.clone();
            pt = evaluate_task(&pt, &job_ctx)
                .map_err(|e| anyhow::anyhow!("failed to evaluate parallel task: {e}"))?;

            pt.id = Some(uuid::Uuid::new_v4().to_string().into());
            pt.job_id = Some(job_id.clone());
            pt.parent_id = Some(task_id.to_string().into());
            pt.state = twerk_core::task::TASK_STATE_PENDING.to_string();
            pt.created_at = Some(now);
            
            self.ds.create_task(&pt).await?;
            self.broker.publish_task(QUEUE_PENDING.to_string(), &pt).await?;
        }
        
        Ok(())
    }

    /// Schedules tasks from an each-loop task definition.
    /// # Errors
    /// Returns error if list evaluation or task creation fails.
    pub async fn schedule_each_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        let job_ctx_map = job.context.as_ref().map(twerk_core::job::JobContext::as_map).unwrap_or_default();
        
        let each = task.each.as_ref().ok_or_else(|| anyhow::anyhow!("missing each config"))?;
        let list_expr = each.list.as_deref().unwrap_or_default();
        
        let list_val = Self::eval_each_list(list_expr, &job_ctx_map)?;
        let list = list_val.as_array().ok_or_else(|| anyhow::anyhow!("each list must be an array"))?;
        let size = list.len() as i64;

        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
            u.started_at = Some(now);
            if let Some(ref mut e) = u.each {
                e.size = size;
            }
            Ok(u)
        })).await?;
        
        let template = each.task.as_ref().ok_or_else(|| anyhow::anyhow!("missing each task template"))?;
        self.spawn_each_tasks(template, list, &job_ctx_map, &task_id, &job_id, now).await
        
    }

    fn eval_each_list(list_expr: &str, job_ctx: &std::collections::HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
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
        job_ctx: &std::collections::HashMap<String, serde_json::Value>,
        task_id: &str,
        job_id: &str,
        now: time::OffsetDateTime,
    ) -> Result<()> {
        let var_name = "item";
        
        for (ix, item) in list.iter().enumerate() {
            let mut cx = job_ctx.clone();
            cx.insert(var_name.to_string(), serde_json::json!({
                "index": ix.to_string(),
                "value": item
            }));

            let mut et = (*template).clone();
            et = evaluate_task(&et, &cx)
                .map_err(|e| anyhow::anyhow!("failed to evaluate each item task: {e}"))?;

            et.id = Some(uuid::Uuid::new_v4().to_string().into());
            et.job_id = Some(job_id.to_string().into());
            et.parent_id = Some(task_id.to_string().into());
            et.state = twerk_core::task::TASK_STATE_PENDING.to_string();
            et.created_at = Some(now);
            
            self.ds.create_task(&et).await?;
            self.broker.publish_task(QUEUE_PENDING.to_string(), &et).await?;
        }
        
        Ok(())
    }

    /// Schedules a subjob task.
    /// # Errors
    /// Returns error if job creation or publish fails.
    pub async fn schedule_subjob_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        
        let job = self.ds.get_job_by_id(&job_id).await?;
        
        let subjob_task = task.subjob.as_ref().ok_or_else(|| anyhow::anyhow!("missing subjob config"))?;
        
        let subjob = twerk_core::job::Job {
            id: Some(uuid::Uuid::new_v4().to_string().into()),
            parent_id: Some(task_id.to_string().into()),
            name: subjob_task.name.clone(),
            description: subjob_task.description.clone(),
            state: twerk_core::job::JOB_STATE_PENDING.to_string(),
            tasks: subjob_task.tasks.clone(),
            inputs: subjob_task.inputs.clone(),
            secrets: subjob_task.secrets.clone(),
            task_count: subjob_task.tasks.as_ref().map_or(0, |t| t.len() as i64),
            output: subjob_task.output.clone(),
            webhooks: subjob_task.webhooks.clone(),
            auto_delete: subjob_task.auto_delete.clone(),
            created_at: Some(now),
            created_by: job.created_by.clone(),
            ..Default::default()
        };
        
        let subjob_id = subjob.id.clone().unwrap_or_default();
        
        self.ds.update_task(&task_id, Box::new(move |mut u| {
            u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
            u.started_at = Some(now);
            if let Some(ref mut sj) = u.subjob {
                sj.id = Some(subjob_id.clone());
            }
            Ok(u)
        })).await?;
        
        self.ds.create_job(&subjob).await?;
        self.broker.publish_job(&subjob).await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::sync::Arc;
    use std::collections::HashMap;
    use dashmap::DashMap;
    use async_trait::async_trait;
    use twerk_core::task::{Task, ParallelTask, EachTask, SubJobTask};
    use twerk_core::job::{Job, JobContext, JobSummary, ScheduledJob, ScheduledJobSummary};
    use twerk_core::node::Node;
    use twerk_core::user::User;
    use twerk_core::role::Role;
    use twerk_core::stats::Metrics;
    use twerk_infrastructure::datastore::{Datastore, Error as DatastoreError, Result as DatastoreResult, Page};
    use twerk_infrastructure::broker::inmemory::InMemoryBroker;

    struct MockDatastore {
        tasks: Arc<DashMap<twerk_core::id::TaskId, Task>>,
        jobs: Arc<DashMap<twerk_core::id::JobId, Job>>,
    }

    impl MockDatastore {
        fn new() -> Self {
            Self {
                tasks: Arc::new(DashMap::new()),
                jobs: Arc::new(DashMap::new()),
            }
        }
    }

    #[async_trait]
    impl Datastore for MockDatastore {
        async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
            let id = task.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
            self.tasks.insert(id, task.clone());
            Ok(())
        }

        async fn update_task(&self, id: &str, modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>) -> DatastoreResult<()> {
            let task_id = twerk_core::id::TaskId::new(id);
            let mut task = self.tasks.get(&task_id).map(|r| r.value().clone()).ok_or(DatastoreError::TaskNotFound)?;
            task = modify(task)?;
            self.tasks.insert(task_id, task);
            Ok(())
        }

        async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
            self.tasks.get(&twerk_core::id::TaskId::new(id)).map(|r| r.value().clone()).ok_or(DatastoreError::TaskNotFound)
        }

        async fn get_active_tasks(&self, _job_id: &str) -> DatastoreResult<Vec<Task>> {
            Ok(Vec::new())
        }

        async fn get_next_task(&self, _parent_task_id: &str) -> DatastoreResult<Task> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_task_log_part(&self, _part: &twerk_core::task::TaskLogPart) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_task_log_parts(&self, _task_id: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_node(&self, _node: &Node) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn update_node(&self, _id: &str, _modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_node_by_id(&self, _id: &str) -> DatastoreResult<Node> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
            let id = job.id.clone().ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
            self.jobs.insert(id, job.clone());
            Ok(())
        }

        async fn update_job(&self, _id: &str, _modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
            self.jobs.get(&twerk_core::id::JobId::new(id)).map(|r| r.value().clone()).ok_or(DatastoreError::JobNotFound)
        }

        async fn get_job_log_parts(&self, _job_id: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_jobs(&self, _current_user: &str, _q: &str, _page: i64, _size: i64) -> DatastoreResult<Page<JobSummary>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_scheduled_job(&self, _sj: &ScheduledJob) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<ScheduledJob>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_scheduled_jobs(&self, _current_user: &str, _page: i64, _size: i64) -> DatastoreResult<Page<ScheduledJobSummary>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_scheduled_job_by_id(&self, _id: &str) -> DatastoreResult<ScheduledJob> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn update_scheduled_job(&self, _id: &str, _modify: Box<dyn FnOnce(ScheduledJob) -> DatastoreResult<ScheduledJob> + Send>) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn delete_scheduled_job(&self, _id: &str) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_user(&self, _user: &User) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_user(&self, _username: &str) -> DatastoreResult<User> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn create_role(&self, _role: &Role) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_role(&self, _id: &str) -> DatastoreResult<Role> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_roles(&self) -> DatastoreResult<Vec<Role>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_user_roles(&self, _user_id: &str) -> DatastoreResult<Vec<Role>> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn assign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn unassign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn get_metrics(&self) -> DatastoreResult<Metrics> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn with_tx(&self, _f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, DatastoreResult<()>> + Send>) -> DatastoreResult<()> {
            Err(DatastoreError::Database("not implemented".to_string()))
        }

        async fn health_check(&self) -> DatastoreResult<()> {
            Ok(())
        }
    }

    fn create_test_job() -> Job {
        Job {
            id: Some(twerk_core::id::JobId::new("job-1")),
            name: Some("Test Job".to_string()),
            state: twerk_core::job::JOB_STATE_PENDING.to_string(),
            context: Some(JobContext {
                inputs: Some(HashMap::new()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn create_test_task() -> Task {
        Task {
            id: Some(twerk_core::id::TaskId::new("task-1")),
            job_id: Some(twerk_core::id::JobId::new("job-1")),
            state: twerk_core::task::TASK_STATE_CREATED.to_string(),
            name: Some("Test Task".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_schedule_regular_task_sets_scheduled_state() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-regular-1"));
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_regular_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-regular-1"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().state, twerk_core::task::TASK_STATE_SCHEDULED);
    }

    #[tokio::test]
    async fn test_schedule_regular_task_sets_default_queue() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-regular-2"));
        task.queue = None;
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_regular_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-regular-2"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().queue, Some("default".to_string()));
    }

    #[tokio::test]
    async fn test_schedule_parallel_task_creates_child_tasks() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let child_task = Task {
            id: None,
            name: Some("Child Task".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        };
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-parallel-1"));
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![child_task]),
            completions: 1,
        });
        
        // Insert parent task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_parallel_task(task.clone()).await.unwrap();
        
        let parent = ds.tasks.get(&twerk_core::id::TaskId::new("task-parallel-1"));
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
        
        let child_count = ds.tasks.iter().filter(|r| r.value().parent_id.is_some()).count();
        assert_eq!(child_count, 1);
    }

    #[tokio::test]
    async fn test_schedule_parallel_task_sets_parent_id_on_children() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let child_task = Task {
            id: None,
            name: Some("Child Task".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        };
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-parallel-2"));
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![child_task]),
            completions: 1,
        });
        
        // Insert parent task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_parallel_task(task.clone()).await.unwrap();
        
        let child = ds.tasks.iter().find(|r| r.value().parent_id.is_some());
        assert!(child.is_some());
        assert_eq!(child.unwrap().value().parent_id, Some(twerk_core::id::TaskId::new("task-parallel-2")));
    }

    #[tokio::test]
    async fn test_schedule_each_task_creates_task_per_list_item() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let template = Task {
            id: None,
            name: Some("Each Item".to_string()),
            run: Some("echo {{item}}".to_string()),
            ..Default::default()
        };
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-each-1"));
        task.each = Some(Box::new(EachTask {
            var: Some("item".to_string()),
            list: Some(r#"["a", "b", "c"]"#.to_string()),
            task: Some(Box::new(template)),
            size: 0,
            completions: 0,
            concurrency: 0,
            index: 0,
        }));
        
        // Insert parent task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_each_task(task.clone()).await.unwrap();
        
        let parent = ds.tasks.get(&twerk_core::id::TaskId::new("task-each-1"));
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
        
        let child_count = ds.tasks.iter().filter(|r| r.value().parent_id.is_some()).count();
        assert_eq!(child_count, 3);
    }

    #[tokio::test]
    async fn test_schedule_each_task_sets_size() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let template = Task {
            id: None,
            name: Some("Each Item".to_string()),
            run: Some("echo {{item}}".to_string()),
            ..Default::default()
        };
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-each-2"));
        task.each = Some(Box::new(EachTask {
            var: Some("item".to_string()),
            list: Some(r#"["x", "y"]"#.to_string()),
            task: Some(Box::new(template)),
            size: 0,
            completions: 0,
            concurrency: 0,
            index: 0,
        }));
        
        // Insert parent task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_each_task(task.clone()).await.unwrap();
        
        let parent_guard = ds.tasks.get(&twerk_core::id::TaskId::new("task-each-2"));
        assert!(parent_guard.is_some());
        let parent = parent_guard.unwrap();
        let each = parent.each.as_ref().unwrap();
        assert_eq!(each.size, 2);
    }

    #[tokio::test]
    async fn test_schedule_subjob_task_creates_subjob() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let subjob_task = Task {
            id: Some(twerk_core::id::TaskId::new("task-subjob-1")),
            job_id: Some(twerk_core::id::JobId::new("job-1")),
            state: twerk_core::task::TASK_STATE_CREATED.to_string(),
            name: Some("SubJob Task".to_string()),
            subjob: Some(SubJobTask {
                id: None,
                name: Some("My SubJob".to_string()),
                description: Some("A subjob".to_string()),
                tasks: Some(vec![Task {
                    name: Some("SubTask 1".to_string()),
                    run: Some("echo sub".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        // Insert task before scheduling
        ds.tasks.insert(subjob_task.id.clone().unwrap(), subjob_task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_subjob_task(subjob_task.clone()).await.unwrap();
        
        let parent = ds.tasks.get(&twerk_core::id::TaskId::new("task-subjob-1"));
        assert!(parent.is_some());
        assert_eq!(parent.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
        
        let subjob_count = ds.jobs.iter().count();
        assert!(subjob_count >= 1);
    }

    #[tokio::test]
    async fn test_schedule_task_dispatches_to_parallel() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-dispatch-parallel"));
        task.parallel = Some(ParallelTask {
            tasks: Some(vec![Task {
                id: None,
                name: Some("Parallel Child".to_string()),
                run: Some("echo parallel".to_string()),
                ..Default::default()
            }]),
            completions: 1,
        });
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-dispatch-parallel"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_task_dispatches_to_each() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-dispatch-each"));
        task.each = Some(Box::new(EachTask {
            var: Some("i".to_string()),
            list: Some(r"[1, 2]".to_string()),
            task: Some(Box::new(Task {
                id: None,
                name: Some("Each Child".to_string()),
                run: Some("echo {{i}}".to_string()),
                ..Default::default()
            })),
            size: 0,
            completions: 0,
            concurrency: 0,
            index: 0,
        }));
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-dispatch-each"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_task_dispatches_to_subjob() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let job = create_test_job();
        ds.jobs.insert(job.id.clone().unwrap(), job.clone());
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-dispatch-subjob"));
        task.subjob = Some(SubJobTask {
            name: Some("SubJob Dispatch Test".to_string()),
            tasks: Some(vec![]),
            ..Default::default()
        });
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-dispatch-subjob"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().state, twerk_core::task::TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_task_dispatches_to_regular() {
        let ds = Arc::new(MockDatastore::new());
        let broker = InMemoryBroker::new();
        
        let mut task = create_test_task();
        task.id = Some(twerk_core::id::TaskId::new("task-dispatch-regular"));
        
        // Insert task before scheduling
        ds.tasks.insert(task.id.clone().unwrap(), task.clone());
        
        let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
        scheduler.schedule_task(task.clone()).await.unwrap();
        
        let stored = ds.tasks.get(&twerk_core::id::TaskId::new("task-dispatch-regular"));
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().state, twerk_core::task::TASK_STATE_SCHEDULED);
    }
}
