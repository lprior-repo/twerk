//! In-memory implementation of the Repository trait.

use crate::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use crate::node::Node;
use crate::repository::{Error, Options, Page, Repository, Result};
use crate::role::Role;
use crate::stats::{JobMetrics, Metrics, NodeMetrics, TaskMetrics};
use crate::task::{Task, TaskLogPart, TaskState};
use crate::user::User;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};

pub struct InMemoryRepository {
    tasks: RwLock<HashMap<String, Task>>,
    task_logs: RwLock<HashMap<String, Vec<TaskLogPart>>>,
    nodes: RwLock<HashMap<String, Node>>,
    jobs: RwLock<HashMap<String, Job>>,
    job_logs: RwLock<HashMap<String, Vec<TaskLogPart>>>,
    scheduled_jobs: RwLock<HashMap<String, ScheduledJob>>,
    users: RwLock<HashMap<String, User>>,
    roles: RwLock<HashMap<String, Role>>,
    user_roles: RwLock<HashMap<String, HashSet<String>>>,
    #[allow(dead_code)]
    options: Options,
}

impl InMemoryRepository {
    pub fn new(options: Options) -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            task_logs: RwLock::new(HashMap::new()),
            nodes: RwLock::new(HashMap::new()),
            jobs: RwLock::new(HashMap::new()),
            job_logs: RwLock::new(HashMap::new()),
            scheduled_jobs: RwLock::new(HashMap::new()),
            users: RwLock::new(HashMap::new()),
            roles: RwLock::new(HashMap::new()),
            user_roles: RwLock::new(HashMap::new()),
            options,
        }
    }

    fn paginate_vec<T: Clone>(items: Vec<T>, page: i64, size: i64) -> Page<T> {
        let total_items = items.len() as i64;
        let total_pages = if size > 0 {
            (total_items + size - 1) / size
        } else {
            0
        };
        let start = ((page - 1) * size) as usize;
        let end = (start + size as usize).min(items.len());
        let page_items: Vec<T> = if start < items.len() {
            items[start..end].to_vec()
        } else {
            Vec::new()
        };
        Page {
            items: page_items,
            number: page,
            size,
            total_pages,
            total_items,
        }
    }
}

#[async_trait]
impl Repository for InMemoryRepository {
    async fn create_task(&self, task: &Task) -> Result<()> {
        let id_str = task
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("task has no id".to_string()));
        }
        let mut tasks = self.tasks.write();
        if tasks.contains_key(&id_str) {
            return Err(Error::Database(format!("task {} already exists", id_str)));
        }
        tasks.insert(id_str, task.clone());
        Ok(())
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> Result<Task> + Send>,
    ) -> Result<()> {
        let mut tasks = self.tasks.write();
        let task = tasks.get_mut(id).ok_or(Error::TaskNotFound)?;
        let updated = modify(task.clone())?;
        *task = updated;
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> Result<Task> {
        let tasks = self.tasks.read();
        tasks.get(id).cloned().ok_or(Error::TaskNotFound)
    }

    async fn get_active_tasks(&self, job_id: &str) -> Result<Vec<Task>> {
        let tasks = self.tasks.read();
        let job_id_str = job_id.to_string();
        Ok(tasks
            .values()
            .filter(|t| {
                t.job_id.as_ref().is_some_and(|j| j.as_str() == job_id_str) && t.state.is_active()
            })
            .cloned()
            .collect())
    }

    async fn get_all_tasks_for_job(&self, job_id: &str) -> Result<Vec<Task>> {
        let tasks = self.tasks.read();
        let job_id_str = job_id.to_string();
        Ok(tasks
            .values()
            .filter(|t| t.job_id.as_ref().is_some_and(|j| j.as_str() == job_id_str))
            .cloned()
            .collect())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> Result<Task> {
        let tasks = self.tasks.read();
        let parent = tasks.get(parent_task_id).ok_or(Error::TaskNotFound)?;
        let job_id_str = parent
            .job_id
            .as_ref()
            .map_or_else(String::new, |j| j.as_str().to_string());
        let parent_position = parent.position;
        Ok(tasks
            .values()
            .filter(|t| {
                t.job_id.as_ref().is_some_and(|j| j.as_str() == job_id_str)
                    && t.parent_id
                        .as_ref()
                        .is_some_and(|p| p.as_str() == parent_task_id)
                    && t.position > parent_position
            })
            .min_by_key(|t| t.position)
            .cloned()
            .ok_or(Error::TaskNotFound)?)
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> Result<()> {
        let task_id_str = part
            .task_id
            .as_ref()
            .map_or_else(String::new, |t| t.as_str().to_string());
        if task_id_str.is_empty() {
            return Err(Error::InvalidId("task log part has no task_id".to_string()));
        }
        let mut logs = self.task_logs.write();
        logs.entry(task_id_str).or_default().push(part.clone());
        Ok(())
    }

    async fn get_task_log_parts(
        &self,
        task_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>> {
        let logs = self.task_logs.read();
        let parts = match logs.get(task_id) {
            Some(v) => v.clone(),
            None => Vec::new(),
        };
        Ok(Self::paginate_vec(parts, page, size))
    }

    async fn create_node(&self, node: &Node) -> Result<()> {
        let id_str = node
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("node has no id".to_string()));
        }
        let mut nodes = self.nodes.write();
        if nodes.contains_key(&id_str) {
            return Err(Error::Database(format!("node {} already exists", id_str)));
        }
        nodes.insert(id_str, node.clone());
        Ok(())
    }

    async fn update_node(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> Result<Node> + Send>,
    ) -> Result<()> {
        let mut nodes = self.nodes.write();
        let node = nodes.get_mut(id).ok_or(Error::NodeNotFound)?;
        let updated = modify(node.clone())?;
        *node = updated;
        Ok(())
    }

    async fn get_node_by_id(&self, id: &str) -> Result<Node> {
        let nodes = self.nodes.read();
        nodes.get(id).cloned().ok_or(Error::NodeNotFound)
    }

    async fn get_active_nodes(&self) -> Result<Vec<Node>> {
        let nodes = self.nodes.read();
        Ok(nodes
            .values()
            .filter(|n| n.status.as_ref().is_some_and(|s| s.as_ref() == "UP"))
            .cloned()
            .collect())
    }

    async fn create_job(&self, job: &Job) -> Result<()> {
        let id_str = job
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("job has no id".to_string()));
        }
        let mut jobs = self.jobs.write();
        if jobs.contains_key(&id_str) {
            return Err(Error::Database(format!("job {} already exists", id_str)));
        }
        jobs.insert(id_str, job.clone());
        Ok(())
    }

    async fn update_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> Result<Job> + Send>,
    ) -> Result<()> {
        let mut jobs = self.jobs.write();
        let job = jobs.get_mut(id).ok_or(Error::JobNotFound)?;
        let updated = modify(job.clone())?;
        *job = updated;
        Ok(())
    }

    async fn get_job_by_id(&self, id: &str) -> Result<Job> {
        let jobs = self.jobs.read();
        jobs.get(id).cloned().ok_or(Error::JobNotFound)
    }

    async fn get_job_log_parts(
        &self,
        job_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<TaskLogPart>> {
        let logs = self.job_logs.read();
        let parts = match logs.get(job_id) {
            Some(v) => v.clone(),
            None => Vec::new(),
        };
        Ok(Self::paginate_vec(parts, page, size))
    }

    async fn get_jobs(
        &self,
        _current_user: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<JobSummary>> {
        let jobs = self.jobs.read();
        let summaries: Vec<JobSummary> = jobs.values().map(crate::job::new_job_summary).collect();
        Ok(Self::paginate_vec(summaries, page, size))
    }

    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> Result<()> {
        let id_str = sj
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("scheduled job has no id".to_string()));
        }
        let mut sjs = self.scheduled_jobs.write();
        if sjs.contains_key(&id_str) {
            return Err(Error::Database(format!(
                "scheduled job {} already exists",
                id_str
            )));
        }
        sjs.insert(id_str, sj.clone());
        Ok(())
    }

    async fn get_active_scheduled_jobs(&self) -> Result<Vec<ScheduledJob>> {
        let sjs = self.scheduled_jobs.read();
        Ok(sjs
            .values()
            .filter(|sj| sj.state == crate::job::ScheduledJobState::Active)
            .cloned()
            .collect())
    }

    async fn get_scheduled_jobs(
        &self,
        _current_user: &str,
        page: i64,
        size: i64,
    ) -> Result<Page<ScheduledJobSummary>> {
        let sjs = self.scheduled_jobs.read();
        let summaries: Vec<ScheduledJobSummary> = sjs
            .values()
            .map(crate::job::new_scheduled_job_summary)
            .collect();
        Ok(Self::paginate_vec(summaries, page, size))
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> Result<ScheduledJob> {
        let sjs = self.scheduled_jobs.read();
        sjs.get(id).cloned().ok_or(Error::ScheduledJobNotFound)
    }

    async fn update_scheduled_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob> + Send>,
    ) -> Result<()> {
        let mut sjs = self.scheduled_jobs.write();
        let sj = sjs.get_mut(id).ok_or(Error::ScheduledJobNotFound)?;
        let updated = modify(sj.clone())?;
        *sj = updated;
        Ok(())
    }

    async fn delete_scheduled_job(&self, id: &str) -> Result<()> {
        let mut sjs = self.scheduled_jobs.write();
        sjs.remove(id).ok_or(Error::ScheduledJobNotFound)?;
        Ok(())
    }

    async fn create_user(&self, user: &User) -> Result<()> {
        let id_str = user
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("user has no id".to_string()));
        }
        let mut users = self.users.write();
        if users.contains_key(&id_str) {
            return Err(Error::Database(format!("user {} already exists", id_str)));
        }
        users.insert(id_str, user.clone());
        Ok(())
    }

    async fn get_user(&self, username: &str) -> Result<User> {
        let users = self.users.read();
        users
            .values()
            .find(|u| u.username.as_deref() == Some(username))
            .cloned()
            .ok_or(Error::UserNotFound)
    }

    async fn create_role(&self, role: &Role) -> Result<()> {
        let id_str = role
            .id
            .as_ref()
            .map_or_else(String::new, |id| id.as_str().to_string());
        if id_str.is_empty() {
            return Err(Error::InvalidId("role has no id".to_string()));
        }
        let mut roles = self.roles.write();
        if roles.contains_key(&id_str) {
            return Err(Error::Database(format!("role {} already exists", id_str)));
        }
        roles.insert(id_str, role.clone());
        Ok(())
    }

    async fn get_role(&self, id: &str) -> Result<Role> {
        let roles = self.roles.read();
        roles.get(id).cloned().ok_or(Error::RoleNotFound)
    }

    async fn get_roles(&self) -> Result<Vec<Role>> {
        let roles = self.roles.read();
        Ok(roles.values().cloned().collect())
    }

    async fn get_user_roles(&self, user_id: &str) -> Result<Vec<Role>> {
        let user_roles = self.user_roles.read();
        let roles = self.roles.read();
        let role_ids = match user_roles.get(user_id) {
            Some(v) => v.clone(),
            None => HashSet::new(),
        };
        Ok(roles
            .values()
            .filter(|r| {
                role_ids.contains(
                    &r.id
                        .as_ref()
                        .map_or_else(String::new, |id| id.as_str().to_string()),
                )
            })
            .cloned()
            .collect())
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write();
        user_roles
            .entry(user_id.to_string())
            .or_default()
            .insert(role_id.to_string());
        Ok(())
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut user_roles = self.user_roles.write();
        if let Some(roles) = user_roles.get_mut(user_id) {
            roles.remove(role_id);
        }
        Ok(())
    }

    async fn get_metrics(&self) -> Result<Metrics> {
        let jobs = self.jobs.read();
        let tasks = self.tasks.read();
        let nodes = self.nodes.read();

        let jobs_running = jobs
            .values()
            .filter(|j| j.state == crate::job::JobState::Running)
            .count() as i32;
        let tasks_running = tasks
            .values()
            .filter(|t| t.state == TaskState::Running)
            .count() as i32;
        let nodes_online = nodes
            .values()
            .filter(|n| n.status.as_ref().is_some_and(|s| s.as_ref() == "UP"))
            .count() as i32;
        let avg_cpu = nodes
            .values()
            .filter_map(|n| n.cpu_percent)
            .fold(0.0, |acc, cpu| acc + cpu)
            / nodes.len().max(1) as f64;

        Ok(Metrics {
            jobs: JobMetrics {
                running: jobs_running,
            },
            tasks: TaskMetrics {
                running: tasks_running,
            },
            nodes: NodeMetrics {
                running: nodes_online,
                cpu_percent: avg_cpu,
            },
        })
    }

    async fn with_tx(
        &self,
        _f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Repository,
                ) -> futures_util::future::BoxFuture<'a, Result<()>>
                + Send,
        >,
    ) -> Result<()> {
        Err(Error::Transaction(
            "in-memory repository does not support transactions".to_string(),
        ))
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}
