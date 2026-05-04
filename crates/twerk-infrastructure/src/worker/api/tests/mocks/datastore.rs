//! Mock datastore implementation for testing.

use async_trait::async_trait;

use twerk_core::node::Node;
use twerk_core::task::Task;

use crate::datastore::{Datastore, Result as DatastoreResult};

/// Mock datastore implementation for testing
#[derive(Debug, Clone, Default)]
pub struct MockDatastore;

#[async_trait]
impl Datastore for MockDatastore {
    async fn create_task(&self, _task: &Task) -> DatastoreResult<()> {
        Ok(())
    }
    async fn update_task(
        &self,
        _id: &str,
        _modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_task_by_id(&self, _id: &str) -> DatastoreResult<Task> {
        Ok(Task::default())
    }
    async fn get_active_tasks(&self, _job_id: &str) -> DatastoreResult<Vec<Task>> {
        Ok(Vec::new())
    }
    async fn get_all_tasks_for_job(&self, _job_id: &str) -> DatastoreResult<Vec<Task>> {
        Ok(Vec::new())
    }
    async fn get_next_task(&self, _parent_task_id: &str) -> DatastoreResult<Task> {
        Ok(Task::default())
    }
    async fn create_task_log_part(
        &self,
        _part: &twerk_core::task::TaskLogPart,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_task_log_parts(
        &self,
        _task_id: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<crate::datastore::Page<twerk_core::task::TaskLogPart>> {
        Ok(crate::datastore::Page {
            items: Vec::new(),
            number: 0,
            size: 0,
            total_pages: 0,
            total_items: 0,
        })
    }
    async fn create_node(&self, _node: &Node) -> DatastoreResult<()> {
        Ok(())
    }
    async fn update_node(
        &self,
        _id: &str,
        _modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_node_by_id(&self, _id: &str) -> DatastoreResult<Node> {
        Ok(Node::default())
    }
    async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
        Ok(Vec::new())
    }
    async fn create_job(&self, _job: &twerk_core::job::Job) -> DatastoreResult<()> {
        Ok(())
    }
    async fn update_job(
        &self,
        _id: &str,
        _modify: Box<
            dyn FnOnce(twerk_core::job::Job) -> DatastoreResult<twerk_core::job::Job> + Send,
        >,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_job_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::job::Job> {
        Ok(twerk_core::job::Job::default())
    }
    async fn get_job_log_parts(
        &self,
        _job_id: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<crate::datastore::Page<twerk_core::task::TaskLogPart>> {
        Ok(crate::datastore::Page {
            items: Vec::new(),
            number: 0,
            size: 0,
            total_pages: 0,
            total_items: 0,
        })
    }
    async fn get_jobs(
        &self,
        _current_user: &str,
        _q: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<crate::datastore::Page<twerk_core::job::JobSummary>> {
        Ok(crate::datastore::Page {
            items: Vec::new(),
            number: 0,
            size: 0,
            total_pages: 0,
            total_items: 0,
        })
    }
    async fn delete_job(&self, _id: &str) -> DatastoreResult<()> {
        Ok(())
    }
    async fn create_scheduled_job(
        &self,
        _sj: &twerk_core::job::ScheduledJob,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_active_scheduled_jobs(
        &self,
    ) -> DatastoreResult<Vec<twerk_core::job::ScheduledJob>> {
        Ok(Vec::new())
    }
    async fn get_scheduled_jobs(
        &self,
        _current_user: &str,
        _page: i64,
        _size: i64,
    ) -> DatastoreResult<crate::datastore::Page<twerk_core::job::ScheduledJobSummary>> {
        Ok(crate::datastore::Page {
            items: Vec::new(),
            number: 0,
            size: 0,
            total_pages: 0,
            total_items: 0,
        })
    }
    async fn get_scheduled_job_by_id(
        &self,
        _id: &str,
    ) -> DatastoreResult<twerk_core::job::ScheduledJob> {
        Ok(twerk_core::job::ScheduledJob::default())
    }
    async fn update_scheduled_job(
        &self,
        _id: &str,
        _modify: Box<
            dyn FnOnce(
                    twerk_core::job::ScheduledJob,
                ) -> DatastoreResult<twerk_core::job::ScheduledJob>
                + Send,
        >,
    ) -> DatastoreResult<()> {
        Ok(())
    }
    async fn delete_scheduled_job(&self, _id: &str) -> DatastoreResult<()> {
        Ok(())
    }
    async fn create_user(&self, _user: &twerk_core::user::User) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_user(&self, _username: &str) -> DatastoreResult<twerk_core::user::User> {
        Ok(twerk_core::user::User::default())
    }
    async fn create_role(&self, _role: &twerk_core::role::Role) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_role(&self, _id: &str) -> DatastoreResult<twerk_core::role::Role> {
        Ok(twerk_core::role::Role::default())
    }
    async fn get_roles(&self) -> DatastoreResult<Vec<twerk_core::role::Role>> {
        Ok(Vec::new())
    }
    async fn get_user_roles(&self, _user_id: &str) -> DatastoreResult<Vec<twerk_core::role::Role>> {
        Ok(Vec::new())
    }
    async fn assign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
        Ok(())
    }
    async fn unassign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
        Ok(())
    }
    async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
        Ok(twerk_core::stats::Metrics {
            jobs: twerk_core::stats::JobMetrics { running: 0 },
            tasks: twerk_core::stats::TaskMetrics { running: 0 },
            nodes: twerk_core::stats::NodeMetrics {
                running: 0,
                cpu_percent: 0.0,
            },
        })
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
        Ok(())
    }
    async fn health_check(&self) -> DatastoreResult<()> {
        Ok(())
    }
}
