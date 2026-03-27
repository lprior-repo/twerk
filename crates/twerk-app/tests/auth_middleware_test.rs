use std::sync::Arc;
use twerk_app::engine::coordinator::auth::{basic_auth_layer, key_auth_layer, BasicAuthConfig, KeyAuthConfig};

#[tokio::test]
async fn basic_auth_layer_creates_proper_layer() {
    use async_trait::async_trait;
    use twerk_infrastructure::datastore::{Error as DatastoreError, Result as DatastoreResult};

    struct MockDatastore;

    #[async_trait]
    impl twerk_infrastructure::datastore::Datastore for MockDatastore {
        async fn create_task(&self, _task: &twerk_core::task::Task) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn update_task(
            &self,
            _id: &str,
            _modify: Box<dyn FnOnce(twerk_core::task::Task) -> DatastoreResult<twerk_core::task::Task> + Send>,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_task_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn get_active_tasks(&self, _job_id: &str) -> DatastoreResult<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_next_task(&self, _parent_task_id: &str) -> DatastoreResult<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn create_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_task_log_parts(
            &self,
            _task_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> DatastoreResult<twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>>
        {
            unimplemented!()
        }
        async fn create_node(&self, _node: &twerk_core::node::Node) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn update_node(
            &self,
            _id: &str,
            _modify: Box<dyn FnOnce(twerk_core::node::Node) -> DatastoreResult<twerk_core::node::Node> + Send>,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_node_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::node::Node> {
            unimplemented!()
        }
        async fn get_active_nodes(&self) -> DatastoreResult<Vec<twerk_core::node::Node>> {
            unimplemented!()
        }
        async fn create_job(&self, _job: &twerk_core::job::Job) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn update_job(
            &self,
            _id: &str,
            _modify: Box<dyn FnOnce(twerk_core::job::Job) -> DatastoreResult<twerk_core::job::Job> + Send>,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_job_by_id(&self, _id: &str) -> DatastoreResult<twerk_core::job::Job> {
            unimplemented!()
        }
        async fn get_job_log_parts(
            &self,
            _job_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> DatastoreResult<twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>>
        {
            unimplemented!()
        }
        async fn get_jobs(
            &self,
            _current_user: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> DatastoreResult<twerk_infrastructure::datastore::Page<twerk_core::job::JobSummary>>
        {
            unimplemented!()
        }
        async fn create_scheduled_job(
            &self,
            _sj: &twerk_core::job::ScheduledJob,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<twerk_core::job::ScheduledJob>> {
            unimplemented!()
        }
        async fn get_scheduled_jobs(
            &self,
            _current_user: &str,
            _page: i64,
            _size: i64,
        ) -> DatastoreResult<twerk_infrastructure::datastore::Page<twerk_core::job::ScheduledJobSummary>>
        {
            unimplemented!()
        }
        async fn get_scheduled_job_by_id(
            &self,
            _id: &str,
        ) -> DatastoreResult<twerk_core::job::ScheduledJob> {
            unimplemented!()
        }
        async fn update_scheduled_job(
            &self,
            _id: &str,
            _modify: Box<dyn FnOnce(twerk_core::job::ScheduledJob) -> DatastoreResult<twerk_core::job::ScheduledJob> + Send>,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn delete_scheduled_job(&self, _id: &str) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn create_user(&self, _user: &twerk_core::user::User) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_user(&self, _username: &str) -> DatastoreResult<twerk_core::user::User> {
            Err(DatastoreError::UserNotFound)
        }
        async fn create_role(&self, _role: &twerk_core::role::Role) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_role(&self, _id: &str) -> DatastoreResult<twerk_core::role::Role> {
            unimplemented!()
        }
        async fn get_roles(&self) -> DatastoreResult<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn get_user_roles(&self, _user_id: &str) -> DatastoreResult<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn assign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn unassign_role(&self, _user_id: &str, _role_id: &str) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
            unimplemented!()
        }
        async fn with_tx(
            &self,
            _f: Box<dyn for<'a> FnOnce(&'a dyn twerk_infrastructure::datastore::Datastore) -> futures_util::future::BoxFuture<'a, DatastoreResult<()>> + Send>,
        ) -> DatastoreResult<()> {
            unimplemented!()
        }
        async fn health_check(&self) -> DatastoreResult<()> {
            Ok(())
        }
    }

    let datastore = MockDatastore;
    let config = BasicAuthConfig::new(Arc::new(datastore));
    let _layer = basic_auth_layer(config);
}

#[tokio::test]
async fn key_auth_layer_creates_proper_layer() {
    let config = KeyAuthConfig::new("test-api-key".to_string());
    let _layer = key_auth_layer(config);
}

#[tokio::test]
async fn key_auth_config_with_empty_key_allowed() {
    let config = KeyAuthConfig::new("".to_string());
    let _layer = key_auth_layer(config);
}

#[tokio::test]
async fn key_auth_config_with_custom_skip_paths() {
    let config = KeyAuthConfig::new("key".to_string())
        .with_skip_paths(vec!["GET /health".to_string(), "POST /ready".to_string()]);
    let _layer = key_auth_layer(config);
}
