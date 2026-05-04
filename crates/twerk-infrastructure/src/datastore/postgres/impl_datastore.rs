//! `PostgresDatastore` `Datastore` trait implementation.
//!
//! This file is a thin shim that delegates to the `impl_*` submodules.

use async_trait::async_trait;

use twerk_core::job::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};
use twerk_core::node::Node;
use twerk_core::role::Role;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::user::User;

use super::super::{Datastore, Page, Result as DatastoreResult};
use crate::datastore::postgres::PostgresDatastore;

#[async_trait]
impl Datastore for PostgresDatastore {
    // Task operations
    async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
        self.create_task_impl(task).await
    }

    async fn create_tasks(&self, tasks: &[Task]) -> DatastoreResult<()> {
        self.create_tasks_impl(tasks).await
    }

    async fn update_task(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>,
    ) -> DatastoreResult<()> {
        self.update_task_impl(id, modify).await
    }

    async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
        self.get_task_by_id_impl(id).await
    }

    async fn get_active_tasks(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        self.get_active_tasks_impl(job_id).await
    }

    async fn get_all_tasks_for_job(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        self.get_all_tasks_for_job_impl(job_id).await
    }

    async fn get_next_task(&self, parent_task_id: &str) -> DatastoreResult<Task> {
        self.get_next_task_impl(parent_task_id).await
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> DatastoreResult<()> {
        self.create_task_log_part_impl(part).await
    }

    async fn get_task_log_parts(
        &self,
        task_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        self.get_task_log_parts_impl(task_id, q, page, size).await
    }

    // Node operations
    async fn create_node(&self, node: &Node) -> DatastoreResult<()> {
        self.create_node_impl(node).await
    }

    async fn update_node(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>,
    ) -> DatastoreResult<()> {
        self.update_node_impl(id, modify).await
    }

    async fn get_node_by_id(&self, id: &str) -> DatastoreResult<Node> {
        self.get_node_by_id_impl(id).await
    }

    async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
        self.get_active_nodes_impl().await
    }

    // Job operations
    async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
        self.create_job_impl(job).await
    }

    async fn update_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>,
    ) -> DatastoreResult<()> {
        self.update_job_impl(id, modify).await
    }

    async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
        self.get_job_by_id_impl(id).await
    }

    async fn get_job_log_parts(
        &self,
        job_id: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        self.get_job_log_parts_impl(job_id, q, page, size).await
    }

    async fn get_jobs(
        &self,
        current_user: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<JobSummary>> {
        self.get_jobs_impl(current_user, q, page, size).await
    }

    async fn delete_job(&self, id: &str) -> DatastoreResult<()> {
        self.delete_job_impl(id).await
    }

    // Scheduled job operations
    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> DatastoreResult<()> {
        self.create_scheduled_job_impl(sj).await
    }

    async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<ScheduledJob>> {
        self.get_active_scheduled_jobs_impl().await
    }

    async fn get_scheduled_jobs(
        &self,
        current_user: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<ScheduledJobSummary>> {
        self.get_scheduled_jobs_impl(current_user, page, size).await
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> DatastoreResult<ScheduledJob> {
        self.get_scheduled_job_by_id_impl(id).await
    }

    async fn update_scheduled_job(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(ScheduledJob) -> DatastoreResult<ScheduledJob> + Send>,
    ) -> DatastoreResult<()> {
        self.update_scheduled_job_impl(id, modify).await
    }

    async fn delete_scheduled_job(&self, id: &str) -> DatastoreResult<()> {
        self.delete_scheduled_job_impl(id).await
    }

    // User and role operations
    async fn create_user(&self, user: &User) -> DatastoreResult<()> {
        self.create_user_impl(user).await
    }

    async fn get_user(&self, username: &str) -> DatastoreResult<User> {
        self.get_user_impl(username).await
    }

    async fn create_role(&self, role: &Role) -> DatastoreResult<()> {
        self.create_role_impl(role).await
    }

    async fn get_role(&self, id: &str) -> DatastoreResult<Role> {
        self.get_role_impl(id).await
    }

    async fn get_roles(&self) -> DatastoreResult<Vec<Role>> {
        self.get_roles_impl().await
    }

    async fn get_user_roles(&self, user_id: &str) -> DatastoreResult<Vec<Role>> {
        self.get_user_roles_impl(user_id).await
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        self.assign_role_impl(user_id, role_id).await
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        self.unassign_role_impl(user_id, role_id).await
    }

    // Metrics
    async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
        self.get_metrics_impl().await
    }

    // Transaction wrapper
    async fn with_tx(
        &self,
        f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Datastore,
                )
                    -> futures_util::future::BoxFuture<'a, DatastoreResult<()>>
                + Send,
        >,
    ) -> DatastoreResult<()> {
        self.with_tx_impl(f).await
    }

    // Health check
    async fn health_check(&self) -> DatastoreResult<()> {
        self.health_check_impl().await
    }
}
