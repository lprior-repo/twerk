use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

use super::{Datastore, Error as DatastoreError, Page, Result};
use twerk_core::id::{JobId, NodeId, ScheduledJobId, TaskId};
use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::job::{JobState, ScheduledJobState};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_core::user::User;

pub struct InMemoryDatastore {
    tasks: Arc<DashMap<TaskId, Task>>,
    /// Secondary index: job_id -> list of task_ids for fast job-based queries
    tasks_by_job: Arc<DashMap<JobId, Vec<TaskId>>>,
    /// Secondary index: parent_task_id -> list of task_ids for fast child lookups
    tasks_by_parent: Arc<DashMap<String, Vec<TaskId>>>,
    nodes: Arc<DashMap<NodeId, Node>>,
    jobs: Arc<DashMap<JobId, Job>>,
    users: Arc<DashMap<String, User>>,
    roles: Arc<DashMap<String, Role>>,
    scheduled_jobs: Arc<DashMap<ScheduledJobId, ScheduledJob>>,
    task_log_parts: Arc<DashMap<TaskId, Vec<TaskLogPart>>>,
    user_roles: Arc<DashMap<String, Vec<String>>>,
}

impl InMemoryDatastore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            tasks_by_job: Arc::new(DashMap::new()),
            tasks_by_parent: Arc::new(DashMap::new()),
            nodes: Arc::new(DashMap::new()),
            jobs: Arc::new(DashMap::new()),
            users: Arc::new(DashMap::new()),
            roles: Arc::new(DashMap::new()),
            scheduled_jobs: Arc::new(DashMap::new()),
            task_log_parts: Arc::new(DashMap::new()),
            user_roles: Arc::new(DashMap::new()),
        }
    }

    fn paginate<T: Clone>(items: Vec<T>, page: i64, size: i64) -> Page<T> {
        let total = items.len() as i64;
        let skip = ((page - 1).max(0) * size) as usize;
        let paged: Vec<T> = items.into_iter().skip(skip).take(size as usize).collect();
        Page {
            items: paged,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        }
    }
}

#[async_trait]
impl Datastore for InMemoryDatastore {
    async fn create_task(&self, task: &Task) -> Result<()> {
        let id = task
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;

        // Index by job_id if present
        if let Some(ref job_id) = task.job_id {
            self.tasks_by_job
                .entry(job_id.clone())
                .or_default()
                .push(id.clone());
        }

        // Index by parent_id if present
        if let Some(ref parent_id) = task.parent_id {
            self.tasks_by_parent
                .entry(parent_id.to_string())
                .or_default()
                .push(id.clone());
        }

        self.tasks.insert(id, task.clone());
        Ok(())
    }

    async fn create_tasks(&self, tasks: &[Task]) -> Result<()> {
        for task in tasks {
            let id = task
                .id
                .clone()
                .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;

            // Index by job_id if present
            if let Some(ref job_id) = task.job_id {
                self.tasks_by_job
                    .entry(job_id.clone())
                    .or_default()
                    .push(id.clone());
            }

            // Index by parent_id if present
            if let Some(ref parent_id) = task.parent_id {
                self.tasks_by_parent
                    .entry(parent_id.to_string())
                    .or_default()
                    .push(id.clone());
            }

            self.tasks.insert(id, task.clone());
        }
        Ok(())
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> Result<Task> + Send>,
    ) -> Result<()> {
        let task = self
            .tasks
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)?;
        let task = modify(task)?;
        let task_id = TaskId::new(id).map_err(|e| DatastoreError::InvalidId(e.to_string()))?;
        self.tasks.insert(task_id, task);
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> Result<Task> {
        self.tasks
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)
    }

    async fn get_active_tasks(&self, job_id: &str) -> Result<Vec<Task>> {
        // Try to use index first
        if let Ok(job_id_parsed) = JobId::new(job_id) {
            if let Some(task_ids) = self.tasks_by_job.get(&job_id_parsed) {
                let tasks: Vec<Task> = task_ids
                    .value()
                    .iter()
                    .filter_map(|tid| {
                        self.tasks.get(tid).and_then(|t| {
                            let task = t.value();
                            if task.is_active() {
                                Some(task.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .collect();
                return Ok(tasks);
            }
        }
        // Fallback to scan if job_id not in index
        Ok(self
            .tasks
            .iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id) && e.value().is_active())
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_all_tasks_for_job(&self, job_id: &str) -> Result<Vec<Task>> {
        // Try to use index first
        if let Ok(job_id_parsed) = JobId::new(job_id) {
            if let Some(task_ids) = self.tasks_by_job.get(&job_id_parsed) {
                let tasks: Vec<Task> = task_ids
                    .value()
                    .iter()
                    .filter_map(|tid| self.tasks.get(tid).map(|t| t.value().clone()))
                    .collect();
                return Ok(tasks);
            }
        }
        // Fallback to scan if job_id not in index
        Ok(self
            .tasks
            .iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id))
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> Result<Task> {
        // Try to use parent index first
        if let Some(child_ids) = self.tasks_by_parent.get(parent_task_id) {
            for tid in child_ids.value().iter() {
                if let Some(task_entry) = self.tasks.get(tid) {
                    let task = task_entry.value();
                    if task.state == TaskState::Created {
                        return Ok(task.clone());
                    }
                }
            }
        }
        // Fallback to scan
        self.tasks
            .iter()
            .find(|e| {
                e.value().parent_id.as_deref() == Some(parent_task_id)
                    && e.value().state == TaskState::Created
            })
            .map(|e| e.value().clone())
            .ok_or(DatastoreError::TaskNotFound)
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> Result<()> {
        let task_id = part
            .task_id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("task_id required".to_string()))?;
        self.task_log_parts
            .entry(task_id)
            .or_default()
            .push(part.clone());
        Ok(())
    }

    async fn get_task_log_parts(
        &self,
        task_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>> {
        let parts = self
            .task_log_parts
            .get(task_id)
            .map_or_else(Vec::new, |r| r.value().clone());
        Ok(Self::paginate(parts, page, size))
    }

    async fn create_node(&self, node: &Node) -> Result<()> {
        let id = node
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.nodes.insert(id, node.clone());
        Ok(())
    }

    async fn update_node(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> Result<Node> + Send>,
    ) -> Result<()> {
        let mut node = self
            .nodes
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::NodeNotFound)?;
        node = modify(node)?;
        let node_id = NodeId::new(id).map_err(|e| DatastoreError::InvalidId(e.to_string()))?;
        self.nodes.insert(node_id, node);
        Ok(())
    }

    async fn get_node_by_id(&self, id: &str) -> Result<Node> {
        self.nodes
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::NodeNotFound)
    }

    async fn get_active_nodes(&self) -> Result<Vec<Node>> {
        Ok(self.nodes.iter().map(|e| e.value().clone()).collect())
    }

    async fn create_job(&self, job: &Job) -> Result<()> {
        let id = job
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.jobs.insert(id, job.clone());
        Ok(())
    }

    async fn update_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> Result<Job> + Send>,
    ) -> Result<()> {
        let mut job = self
            .jobs
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::JobNotFound)?;
        job = modify(job)?;
        let job_id = JobId::new(id).map_err(|e| DatastoreError::InvalidId(e.to_string()))?;
        self.jobs.insert(job_id, job);
        Ok(())
    }

    async fn get_job_by_id(&self, id: &str) -> Result<Job> {
        self.jobs
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::JobNotFound)
    }

    async fn get_job_log_parts(
        &self,
        job_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>> {
        let task_ids: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id))
            .filter_map(|e| e.value().id.clone())
            .collect();
        let all_parts = task_ids
            .iter()
            .filter_map(|tid| self.task_log_parts.get(tid))
            .flat_map(|parts| parts.value().clone())
            .collect();
        Ok(Self::paginate(all_parts, page, size))
    }

    async fn get_jobs(
        &self,
        _current_user: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<JobSummary>> {
        let summaries: Vec<JobSummary> = self
            .jobs
            .iter()
            .map(|e| twerk_core::job::new_job_summary(e.value()))
            .collect();
        Ok(Self::paginate(summaries, page, size))
    }

    async fn delete_job(&self, id: &str) -> Result<()> {
        self.jobs.remove(id);
        Ok(())
    }

    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> Result<()> {
        let id = sj
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.scheduled_jobs.insert(id, sj.clone());
        Ok(())
    }

    async fn get_active_scheduled_jobs(&self) -> Result<Vec<ScheduledJob>> {
        Ok(self
            .scheduled_jobs
            .iter()
            .filter(|e| e.value().state == ScheduledJobState::Active)
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_scheduled_jobs(
        &self,
        _current_user: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<ScheduledJobSummary>> {
        let summaries: Vec<ScheduledJobSummary> = self
            .scheduled_jobs
            .iter()
            .map(|e| twerk_core::job::new_scheduled_job_summary(e.value()))
            .collect();
        Ok(Self::paginate(summaries, page, size))
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> Result<ScheduledJob> {
        self.scheduled_jobs
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::ScheduledJobNotFound)
    }

    async fn update_scheduled_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob> + Send>,
    ) -> Result<()> {
        let mut sj = self
            .scheduled_jobs
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::ScheduledJobNotFound)?;
        sj = modify(sj)?;
        let sj_id =
            ScheduledJobId::new(id).map_err(|e| DatastoreError::InvalidId(e.to_string()))?;
        self.scheduled_jobs.insert(sj_id, sj);
        Ok(())
    }

    async fn delete_scheduled_job(&self, id: &str) -> Result<()> {
        self.scheduled_jobs.remove(id);
        Ok(())
    }

    async fn create_user(&self, user: &User) -> Result<()> {
        let id = user
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        if let Some(ref username) = user.username {
            self.users.insert(username.clone(), user.clone());
        }
        self.users.insert(id.to_string(), user.clone());
        Ok(())
    }

    async fn get_user(&self, username: &str) -> Result<User> {
        self.users
            .get(username)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::UserNotFound)
    }

    async fn create_role(&self, role: &Role) -> Result<()> {
        let id = role
            .id
            .clone()
            .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
        self.roles.insert(id.to_string(), role.clone());
        Ok(())
    }

    async fn get_role(&self, id: &str) -> Result<Role> {
        self.roles
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::RoleNotFound)
    }

    async fn get_roles(&self) -> Result<Vec<Role>> {
        Ok(self.roles.iter().map(|e| e.value().clone()).collect())
    }

    async fn get_user_roles(&self, user_id: &str) -> Result<Vec<Role>> {
        let role_ids = self
            .user_roles
            .get(user_id)
            .map_or_else(Vec::new, |r| r.value().clone());
        Ok(role_ids
            .iter()
            .filter_map(|rid| self.roles.get(rid).map(|r| r.value().clone()))
            .collect())
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        self.user_roles
            .entry(user_id.to_string())
            .or_default()
            .push(role_id.to_string());
        Ok(())
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        // SAFETY: DashMap::get_mut requires RefMut which necessitates internal mutation.
        // This is an inherent limitation of the dashmap API at the Actions boundary.
        if let Some(mut roles) = self.user_roles.get_mut(user_id) {
            roles.retain(|r| r != role_id);
        }
        Ok(())
    }

    async fn get_metrics(&self) -> Result<twerk_core::stats::Metrics> {
        let jobs_running = self
            .jobs
            .iter()
            .filter(|e| e.value().state == JobState::Running)
            .count() as i32;
        let tasks_running = self
            .tasks
            .iter()
            .filter(|e| e.value().state == TaskState::Running)
            .count() as i32;
        let nodes_running = self.nodes.len() as i32;
        Ok(twerk_core::stats::Metrics {
            jobs: twerk_core::stats::JobMetrics {
                running: jobs_running,
            },
            tasks: twerk_core::stats::TaskMetrics {
                running: tasks_running,
            },
            nodes: twerk_core::stats::NodeMetrics {
                running: nodes_running,
                cpu_percent: 0.0,
            },
        })
    }

    async fn with_tx(
        &self,
        f: Box<
            dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, Result<()>>
                + Send,
        >,
    ) -> Result<()> {
        f(self).await
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for InMemoryDatastore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::id::{RoleId, TaskId};
    use twerk_core::task::TaskState;

    fn make_task(id: &str) -> Task {
        Task {
            id: Some(TaskId::new(id).unwrap()),
            name: Some(format!("test-task-{id}")),
            state: TaskState::default(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_create_and_get_task() {
        let ds = InMemoryDatastore::new();
        let task = make_task("task1");
        ds.create_task(&task).await.unwrap();
        let retrieved = ds.get_task_by_id("task1").await.unwrap();
        assert_eq!(retrieved.id, task.id);
    }

    #[tokio::test]
    async fn test_get_task_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds.get_task_by_id("nonexistent").await.unwrap_err();
        assert!(matches!(err, DatastoreError::TaskNotFound));
    }

    #[tokio::test]
    async fn test_create_task_without_id_fails() {
        let ds = InMemoryDatastore::new();
        let task = Task::default();
        let err = ds.create_task(&task).await.unwrap_err();
        assert!(matches!(err, DatastoreError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_update_task() {
        let ds = InMemoryDatastore::new();
        let task = make_task("task1");
        ds.create_task(&task).await.unwrap();

        ds.update_task(
            "task1",
            Box::new(|mut t| {
                t.name = Some("updated".to_string());
                Ok(t)
            }),
        )
        .await
        .unwrap();

        let retrieved = ds.get_task_by_id("task1").await.unwrap();
        assert_eq!(retrieved.name, Some("updated".to_string()));
    }

    #[tokio::test]
    async fn test_update_task_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds
            .update_task("nonexistent", Box::new(Ok))
            .await
            .unwrap_err();
        assert!(matches!(err, DatastoreError::TaskNotFound));
    }

    #[tokio::test]
    async fn test_create_and_get_node() {
        let ds = InMemoryDatastore::new();
        let node = twerk_core::node::Node {
            id: Some(twerk_core::id::NodeId::new("node1").unwrap()),
            name: Some("test-node".to_string()),
            ..Default::default()
        };
        ds.create_node(&node).await.unwrap();
        let retrieved = ds.get_node_by_id("node1").await.unwrap();
        assert_eq!(retrieved.id, node.id);
    }

    #[tokio::test]
    async fn test_get_node_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds.get_node_by_id("nonexistent").await.unwrap_err();
        assert!(matches!(err, DatastoreError::NodeNotFound));
    }

    #[tokio::test]
    async fn test_create_and_get_job() {
        let ds = InMemoryDatastore::new();
        let job = twerk_core::job::Job {
            id: Some(twerk_core::id::JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap()),
            name: Some("test-job".to_string()),
            ..Default::default()
        };
        ds.create_job(&job).await.unwrap();
        let retrieved = ds
            .get_job_by_id("550e8400-e29b-41d4-a716-446655440000")
            .await
            .unwrap();
        assert_eq!(retrieved.id, job.id);
    }

    #[tokio::test]
    async fn test_get_job_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds.get_job_by_id("nonexistent").await.unwrap_err();
        assert!(matches!(err, DatastoreError::JobNotFound));
    }

    #[tokio::test]
    async fn test_create_and_get_user() {
        let ds = InMemoryDatastore::new();
        let user = twerk_core::user::User {
            id: Some(twerk_core::id::UserId::new("user1").unwrap()),
            username: Some("testuser".to_string()),
            ..Default::default()
        };
        ds.create_user(&user).await.unwrap();
        let retrieved = ds.get_user("testuser").await.unwrap();
        assert_eq!(retrieved.username, Some("testuser".to_string()));
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds.get_user("nonexistent").await.unwrap_err();
        assert!(matches!(err, DatastoreError::UserNotFound));
    }

    #[tokio::test]
    async fn test_create_and_get_role() {
        let ds = InMemoryDatastore::new();
        let role = twerk_core::role::Role {
            id: Some(RoleId::new("role1").unwrap()),
            name: Some("test-role".to_string()),
            ..Default::default()
        };
        ds.create_role(&role).await.unwrap();
        let retrieved = ds.get_role("role1").await.unwrap();
        assert_eq!(retrieved.id, role.id);
    }

    #[tokio::test]
    async fn test_get_role_not_found() {
        let ds = InMemoryDatastore::new();
        let err = ds.get_role("nonexistent").await.unwrap_err();
        assert!(matches!(err, DatastoreError::RoleNotFound));
    }

    #[tokio::test]
    async fn test_get_roles() {
        let ds = InMemoryDatastore::new();
        let role1 = twerk_core::role::Role {
            id: Some(RoleId::new("role1").unwrap()),
            name: Some("role1".to_string()),
            ..Default::default()
        };
        let role2 = twerk_core::role::Role {
            id: Some(RoleId::new("role2").unwrap()),
            name: Some("role2".to_string()),
            ..Default::default()
        };
        ds.create_role(&role1).await.unwrap();
        ds.create_role(&role2).await.unwrap();
        let roles = ds.get_roles().await.unwrap();
        assert_eq!(roles.len(), 2);
    }

    #[tokio::test]
    async fn test_active_nodes() {
        let ds = InMemoryDatastore::new();
        let node = twerk_core::node::Node {
            id: Some(twerk_core::id::NodeId::new("node1").unwrap()),
            name: Some("test-node".to_string()),
            ..Default::default()
        };
        ds.create_node(&node).await.unwrap();
        let nodes = ds.get_active_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_get_active_tasks() {
        let ds = InMemoryDatastore::new();
        let task = make_task("task1");
        ds.create_task(&task).await.unwrap();
        let tasks = ds.get_active_tasks("nonexistent-job").await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_health_check() {
        let ds = InMemoryDatastore::new();
        ds.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_metrics() {
        let ds = InMemoryDatastore::new();
        let metrics = ds.get_metrics().await.unwrap();
        assert_eq!(metrics.jobs.running, 0);
        assert_eq!(metrics.tasks.running, 0);
        assert_eq!(metrics.nodes.running, 0);
    }

    #[tokio::test]
    async fn test_paginate() {
        let items: Vec<i32> = (0..100).collect();
        let page = InMemoryDatastore::paginate(items.clone(), 1, 10);
        assert_eq!(page.items.len(), 10);
        assert_eq!(page.total_items, 100);
        assert_eq!(page.total_pages, 10);

        let page2 = InMemoryDatastore::paginate(items, 2, 10);
        assert_eq!(page2.items.len(), 10);
        assert_eq!(page2.number, 2);
    }

    #[tokio::test]
    async fn test_paginate_empty() {
        let items: Vec<i32> = vec![];
        let page = InMemoryDatastore::paginate(items, 1, 10);
        assert!(page.items.is_empty());
        assert_eq!(page.total_items, 0);
        assert_eq!(page.total_pages, 0);
    }

    #[tokio::test]
    async fn test_paginate_page_out_of_range() {
        let items: Vec<i32> = (0..5).collect();
        let page = InMemoryDatastore::paginate(items, 10, 10);
        assert!(page.items.is_empty());
    }
}
