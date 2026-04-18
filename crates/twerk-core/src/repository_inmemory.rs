//! In-memory implementation of the Repository trait.

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};

use crate::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use crate::node::Node;
use crate::repository::{Error, Options, Page, Repository, Result};
use crate::role::Role;
use crate::stats::{JobMetrics, Metrics, NodeMetrics, TaskMetrics};
use crate::task::{Task, TaskLogPart, TaskState};
use crate::user::User;

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
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
                t.job_id
                    .as_ref()
                    .map(|j| j.as_str() == job_id_str)
                    .unwrap_or(false)
                    && t.state.is_active()
            })
            .cloned()
            .collect())
    }

    async fn get_all_tasks_for_job(&self, job_id: &str) -> Result<Vec<Task>> {
        let tasks = self.tasks.read();
        let job_id_str = job_id.to_string();
        Ok(tasks
            .values()
            .filter(|t| {
                t.job_id
                    .as_ref()
                    .map(|j| j.as_str() == job_id_str)
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> Result<Task> {
        let tasks = self.tasks.read();
        let parent = tasks.get(parent_task_id).ok_or(Error::TaskNotFound)?;
        let job_id_str = parent
            .job_id
            .as_ref()
            .map(|j| j.as_str().to_string())
            .unwrap_or_default();
        let parent_position = parent.position;
        Ok(tasks
            .values()
            .filter(|t| {
                t.job_id
                    .as_ref()
                    .map(|j| j.as_str() == job_id_str)
                    .unwrap_or(false)
                    && t.parent_id
                        .as_ref()
                        .map(|p| p.as_str() == parent_task_id)
                        .unwrap_or(false)
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
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();
        if task_id_str.is_empty() {
            return Err(Error::InvalidId("task log part has no task_id".to_string()));
        }
        let mut logs = self.task_logs.write();
        logs.entry(task_id_str)
            .or_insert_with(Vec::new)
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
        let logs = self.task_logs.read();
        let parts = logs.get(task_id).cloned().unwrap_or_default();
        Ok(Self::paginate_vec(parts, page, size))
    }

    async fn create_node(&self, node: &Node) -> Result<()> {
        let id_str = node
            .id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
            .filter(|n| {
                n.status
                    .as_ref()
                    .map(|s| s.as_ref() == "UP")
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    async fn create_job(&self, job: &Job) -> Result<()> {
        let id_str = job
            .id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
        let parts = logs.get(job_id).cloned().unwrap_or_default();
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
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
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
        let role_ids = user_roles.get(user_id).cloned().unwrap_or_default();
        Ok(roles
            .values()
            .filter(|r| {
                role_ids.contains(
                    &r.id
                        .as_ref()
                        .map(|id| id.as_str().to_string())
                        .unwrap_or_default(),
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
            .filter(|n| {
                n.status
                    .as_ref()
                    .map(|s| s.as_ref() == "UP")
                    .unwrap_or(false)
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{JobId, NodeId, RoleId, ScheduledJobId, TaskId, UserId};
    use time::OffsetDateTime;

    fn create_test_task(id: &str, job_id: Option<&JobId>, state: TaskState) -> Task {
        Task {
            id: Some(TaskId::new(id).unwrap()),
            job_id: job_id.map(|j| j.clone()),
            state,
            ..Default::default()
        }
    }

    fn create_test_job(id: &str) -> Job {
        Job {
            id: Some(JobId::new(id).unwrap()),
            state: crate::job::JobState::Pending,
            ..Default::default()
        }
    }

    fn create_test_node(id: &str, status: Option<crate::node::NodeStatus>) -> Node {
        Node {
            id: Some(NodeId::new(id).unwrap()),
            status,
            ..Default::default()
        }
    }

    fn create_test_user(username: &str) -> User {
        User {
            id: Some(UserId::new("uid-1").unwrap()),
            username: Some(username.to_string()),
            ..Default::default()
        }
    }

    fn create_test_role(slug: &str) -> Role {
        Role {
            id: Some(RoleId::new("rid-1").unwrap()),
            slug: Some(slug.to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn create_and_get_task() {
        let repo = InMemoryRepository::new(Options::default());
        let task = create_test_task("task-1", None, TaskState::Created);
        repo.create_task(&task).await.unwrap();
        let retrieved = repo.get_task_by_id("task-1").await.unwrap();
        assert_eq!(retrieved.id, task.id);
    }

    #[tokio::test]
    async fn get_task_not_found() {
        let repo = InMemoryRepository::new(Options::default());
        let result = repo.get_task_by_id("nonexistent").await;
        assert!(matches!(result, Err(Error::TaskNotFound)));
    }

    #[tokio::test]
    async fn get_active_tasks() {
        let repo = InMemoryRepository::new(Options::default());
        let job_id = JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();

        repo.create_task(&create_test_task(
            "task-1",
            Some(&job_id),
            TaskState::Running,
        ))
        .await
        .unwrap();
        repo.create_task(&create_test_task(
            "task-2",
            Some(&job_id),
            TaskState::Completed,
        ))
        .await
        .unwrap();
        repo.create_task(&create_test_task(
            "task-3",
            Some(&job_id),
            TaskState::Pending,
        ))
        .await
        .unwrap();

        let active = repo.get_active_tasks(job_id.as_str()).await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn create_and_get_node() {
        let repo = InMemoryRepository::new(Options::default());
        let node = create_test_node("node-1", Some(crate::node::NodeStatus::UP));
        repo.create_node(&node).await.unwrap();
        let retrieved = repo.get_node_by_id("node-1").await.unwrap();
        assert_eq!(retrieved.id, node.id);
    }

    #[tokio::test]
    async fn get_active_nodes() {
        let repo = InMemoryRepository::new(Options::default());
        repo.create_node(&create_test_node(
            "node-1",
            Some(crate::node::NodeStatus::UP),
        ))
        .await
        .unwrap();
        repo.create_node(&create_test_node(
            "node-2",
            Some(crate::node::NodeStatus::DOWN),
        ))
        .await
        .unwrap();
        repo.create_node(&create_test_node(
            "node-3",
            Some(crate::node::NodeStatus::UP),
        ))
        .await
        .unwrap();

        let active = repo.get_active_nodes().await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn create_and_get_job() {
        let repo = InMemoryRepository::new(Options::default());
        let job = create_test_job("550e8400-e29b-41d4-a716-446655440000");
        repo.create_job(&job).await.unwrap();
        let retrieved = repo
            .get_job_by_id("550e8400-e29b-41d4-a716-446655440000")
            .await
            .unwrap();
        assert_eq!(retrieved.id, job.id);
    }

    #[tokio::test]
    async fn create_and_get_scheduled_job() {
        let repo = InMemoryRepository::new(Options::default());
        let sj = ScheduledJob {
            id: Some(ScheduledJobId::new("sj-1").unwrap()),
            state: crate::job::ScheduledJobState::Active,
            ..Default::default()
        };
        repo.create_scheduled_job(&sj).await.unwrap();
        let retrieved = repo.get_scheduled_job_by_id("sj-1").await.unwrap();
        assert_eq!(retrieved.id, sj.id);
    }

    #[tokio::test]
    async fn delete_scheduled_job() {
        let repo = InMemoryRepository::new(Options::default());
        let sj = ScheduledJob {
            id: Some(ScheduledJobId::new("sj-1").unwrap()),
            state: crate::job::ScheduledJobState::Active,
            ..Default::default()
        };
        repo.create_scheduled_job(&sj).await.unwrap();
        repo.delete_scheduled_job("sj-1").await.unwrap();
        let result = repo.get_scheduled_job_by_id("sj-1").await;
        assert!(matches!(result, Err(Error::ScheduledJobNotFound)));
    }

    #[tokio::test]
    async fn create_and_get_user() {
        let repo = InMemoryRepository::new(Options::default());
        let user = create_test_user("alice");
        repo.create_user(&user).await.unwrap();
        let retrieved = repo.get_user("alice").await.unwrap();
        assert_eq!(retrieved.username, user.username);
    }

    #[tokio::test]
    async fn user_not_found() {
        let repo = InMemoryRepository::new(Options::default());
        let result = repo.get_user("nobody").await;
        assert!(matches!(result, Err(Error::UserNotFound)));
    }

    #[tokio::test]
    async fn create_and_get_role() {
        let repo = InMemoryRepository::new(Options::default());
        let role = create_test_role("admin");
        repo.create_role(&role).await.unwrap();
        let retrieved = repo.get_role("rid-1").await.unwrap();
        assert_eq!(retrieved.slug, role.slug);
    }

    #[tokio::test]
    async fn assign_and_unassign_role() {
        let repo = InMemoryRepository::new(Options::default());
        let user = create_test_user("alice");
        let role = create_test_role("admin");
        repo.create_user(&user).await.unwrap();
        repo.create_role(&role).await.unwrap();

        repo.assign_role("uid-1", "rid-1").await.unwrap();
        let roles = repo.get_user_roles("uid-1").await.unwrap();
        assert_eq!(roles.len(), 1);

        repo.unassign_role("uid-1", "rid-1").await.unwrap();
        let roles = repo.get_user_roles("uid-1").await.unwrap();
        assert_eq!(roles.len(), 0);
    }

    #[tokio::test]
    async fn get_metrics() {
        let repo = InMemoryRepository::new(Options::default());
        let metrics = repo.get_metrics().await.unwrap();
        assert_eq!(metrics.jobs.running, 0);
        assert_eq!(metrics.tasks.running, 0);
        assert_eq!(metrics.nodes.running, 0);
    }

    #[tokio::test]
    async fn health_check() {
        let repo = InMemoryRepository::new(Options::default());
        repo.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn task_log_parts() {
        let repo = InMemoryRepository::new(Options::default());
        let log_part = TaskLogPart {
            id: Some("log-1".to_string()),
            task_id: Some(TaskId::new("task-1").unwrap()),
            number: 1,
            contents: Some("test output".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        repo.create_task_log_part(&log_part).await.unwrap();
        let page = repo.get_task_log_parts("task-1", "", 1, 10).await.unwrap();
        assert_eq!(page.items.len(), 1);
    }

    #[tokio::test]
    async fn paginate_jobs() {
        let repo = InMemoryRepository::new(Options::default());
        for i in 0..5 {
            let job = Job {
                id: Some(JobId::new(format!("550e8400-e29b-41d4-a716-44665544000{}", i)).unwrap()),
                ..Default::default()
            };
            repo.create_job(&job).await.unwrap();
        }
        let page = repo.get_jobs("", "", 1, 2).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total_items, 5);
        assert_eq!(page.total_pages, 3);
    }

    #[tokio::test]
    async fn update_task() {
        let repo = InMemoryRepository::new(Options::default());
        let task = create_test_task("task-1", None, TaskState::Created);
        repo.create_task(&task).await.unwrap();

        repo.update_task(
            "task-1",
            Box::new(|mut t| {
                t.state = TaskState::Running;
                Ok(t)
            }),
        )
        .await
        .unwrap();

        let updated = repo.get_task_by_id("task-1").await.unwrap();
        assert_eq!(updated.state, TaskState::Running);
    }

    #[tokio::test]
    async fn update_node() {
        let repo = InMemoryRepository::new(Options::default());
        let node = create_test_node("node-1", Some(crate::node::NodeStatus::UP));
        repo.create_node(&node).await.unwrap();

        repo.update_node(
            "node-1",
            Box::new(|mut n| {
                n.status = Some(crate::node::NodeStatus::DOWN);
                Ok(n)
            }),
        )
        .await
        .unwrap();

        let updated = repo.get_node_by_id("node-1").await.unwrap();
        assert_eq!(updated.status, Some(crate::node::NodeStatus::DOWN));
    }

    #[tokio::test]
    async fn update_scheduled_job() {
        let repo = InMemoryRepository::new(Options::default());
        let sj = ScheduledJob {
            id: Some(ScheduledJobId::new("sj-1").unwrap()),
            state: crate::job::ScheduledJobState::Active,
            ..Default::default()
        };
        repo.create_scheduled_job(&sj).await.unwrap();

        repo.update_scheduled_job(
            "sj-1",
            Box::new(|mut s| {
                s.state = crate::job::ScheduledJobState::Paused;
                Ok(s)
            }),
        )
        .await
        .unwrap();

        let updated = repo.get_scheduled_job_by_id("sj-1").await.unwrap();
        assert_eq!(updated.state, crate::job::ScheduledJobState::Paused);
    }
}
