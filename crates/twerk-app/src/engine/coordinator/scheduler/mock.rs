//! Mock implementations for scheduler tests

#![allow(clippy::unwrap_used, clippy::expect_used)]

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use twerk_core::job::{Job, JobContext, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::stats::Metrics;
use twerk_core::task::Task;
use twerk_core::user::User;
use twerk_infrastructure::datastore::{
    Datastore, Error as DatastoreError, Page, Result as DatastoreResult,
};

pub(crate) struct MockDatastore {
    pub tasks: Arc<DashMap<twerk_core::id::TaskId, Task>>,
    pub jobs: Arc<DashMap<twerk_core::id::JobId, Job>>,
}

pub(crate) type FakeDatastore = MockDatastore;

impl MockDatastore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            jobs: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl Datastore for MockDatastore {
    async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
        let id = task
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.tasks.insert(id, task.clone());
        Ok(())
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>,
    ) -> DatastoreResult<()> {
        let task_id = twerk_core::id::TaskId::new(id).unwrap();
        let mut task = self
            .tasks
            .get(&task_id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)?;
        task = modify(task)?;
        self.tasks.insert(task_id, task);
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
        self.tasks
            .get(&twerk_core::id::TaskId::new(id).unwrap())
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)
    }

    async fn get_active_tasks(&self, _job_id: &str) -> DatastoreResult<Vec<Task>> {
        Ok(Vec::new())
    }

    async fn get_all_tasks_for_job(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let job_id = twerk_core::id::JobId::new(job_id).unwrap();
        Ok(self
            .tasks
            .iter()
            .filter(|entry| entry.value().job_id.as_ref() == Some(&job_id))
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn get_next_task(&self, _parent_task_id: &str) -> DatastoreResult<Task> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn create_task_log_part(
        &self,
        _part: &twerk_core::task::TaskLogPart,
    ) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_task_log_parts(
        &self,
        _task_id: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn create_node(&self, _node: &Node) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn update_node(
        &self,
        _id: &str,
        _modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>,
    ) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_node_by_id(&self, _id: &str) -> DatastoreResult<Node> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
        let id = job
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.jobs.insert(id, job.clone());
        Ok(())
    }

    async fn update_job(
        &self,
        _id: &str,
        _modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>,
    ) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
        self.jobs
            .get(&twerk_core::id::JobId::new(id).unwrap())
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::JobNotFound)
    }

    async fn get_job_log_parts(
        &self,
        _job_id: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_jobs(
        &self,
        _current_user: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<Page<JobSummary>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn delete_job(&self, _id: &str) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn create_scheduled_job(&self, _sj: &ScheduledJob) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<ScheduledJob>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_scheduled_jobs(
        &self,
        _current_user: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<Page<ScheduledJobSummary>> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn get_scheduled_job_by_id(&self, _id: &str) -> DatastoreResult<ScheduledJob> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn update_scheduled_job(
        &self,
        _id: &str,
        _modify: Box<dyn FnOnce(ScheduledJob) -> DatastoreResult<ScheduledJob> + Send>,
    ) -> DatastoreResult<()> {
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

    async fn with_tx(
        &self,
        _f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Datastore,
                )
                    -> futures_util::future::BoxFuture<'a, DatastoreResult<()>>
                + Send,
        >,
    ) -> DatastoreResult<()> {
        Err(DatastoreError::Database("not implemented".to_string()))
    }

    async fn health_check(&self) -> DatastoreResult<()> {
        Ok(())
    }
}

// ── Test Helper Functions ─────────────────────────────────────────

use std::collections::HashMap;

pub(crate) fn create_test_job() -> Job {
    Job {
        id: Some(twerk_core::id::JobId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        name: Some("Test Job".to_string()),
        state: twerk_core::job::JobState::Pending,
        context: Some(JobContext {
            inputs: Some(HashMap::new()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub(crate) fn create_test_task() -> Task {
    Task {
        id: Some(twerk_core::id::TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()),
        job_id: Some(twerk_core::id::JobId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        state: twerk_core::task::TaskState::Created,
        name: Some("Test Task".to_string()),
        ..Default::default()
    }
}
