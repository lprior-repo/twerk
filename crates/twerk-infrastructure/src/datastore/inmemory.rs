use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

use super::{Datastore, Error as DatastoreError, Page, Result};
use twerk_core::id::{JobId, NodeId, ScheduledJobId, TaskId};
use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::user::User;

pub struct InMemoryDatastore {
    tasks: Arc<DashMap<TaskId, Task>>,
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
        self.tasks.insert(id, task.clone());
        Ok(())
    }

    async fn create_tasks(&self, tasks: &[Task]) -> Result<()> {
        for task in tasks {
            let id = task
                .id
                .clone()
                .ok_or_else(|| DatastoreError::InvalidInput("id required".to_string()))?;
            self.tasks.insert(id, task.clone());
        }
        Ok(())
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> Result<Task> + Send>,
    ) -> Result<()> {
        let mut task = self
            .tasks
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)?;
        task = modify(task)?;
        self.tasks.insert(TaskId::new(id), task);
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> Result<Task> {
        self.tasks
            .get(id)
            .map(|r| r.value().clone())
            .ok_or(DatastoreError::TaskNotFound)
    }

    async fn get_active_tasks(&self, job_id: &str) -> Result<Vec<Task>> {
        Ok(self
            .tasks
            .iter()
            .filter(|e| e.value().job_id.as_deref() == Some(job_id) && e.value().is_active())
            .map(|e| e.value().clone())
            .collect())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> Result<Task> {
        self.tasks
            .iter()
            .find(|e| {
                e.value().parent_id.as_deref() == Some(parent_task_id)
                    && e.value().state == "CREATED"
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
            .map(|r| r.value().clone())
            .unwrap_or_default();
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
        self.nodes.insert(NodeId::new(id), node);
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
        self.jobs.insert(JobId::new(id), job);
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
        let mut all_parts = Vec::new();
        for tid in task_ids {
            if let Some(parts) = self.task_log_parts.get(&tid) {
                all_parts.extend(parts.value().clone());
            }
        }
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
            .filter(|e| e.value().state == "ACTIVE")
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
        self.scheduled_jobs.insert(ScheduledJobId::new(id), sj);
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
            .map(|r| r.value().clone())
            .unwrap_or_default();
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
        if let Some(mut roles) = self.user_roles.get_mut(user_id) {
            roles.retain(|r| r != role_id);
        }
        Ok(())
    }

    async fn get_metrics(&self) -> Result<twerk_core::stats::Metrics> {
        let jobs_running = self
            .jobs
            .iter()
            .filter(|e| e.value().state == "RUNNING")
            .count() as i32;
        let tasks_running = self
            .tasks
            .iter()
            .filter(|e| e.value().state == "RUNNING")
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
