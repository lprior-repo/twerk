#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::too_many_lines)]

use axum::routing::get;
use std::sync::Arc;
use tower::ServiceExt;
use twerk_app::engine::coordinator::auth::{
    basic_auth_middleware, key_auth_middleware, BasicAuthConfig, KeyAuthConfig,
};

async fn ok_handler() -> &'static str {
    "OK"
}

#[tokio::test]
async fn test_valid_basic_auth_credentials_pass() {
    use async_trait::async_trait;
    use axum::http::{header, StatusCode};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use tower::ServiceExt;
    use twerk_core::user::User;

    struct MockDatastore {
        user: User,
    }

    impl MockDatastore {
        fn new(user: User) -> Self {
            Self { user }
        }
    }

    #[async_trait]
    impl twerk_infrastructure::datastore::Datastore for MockDatastore {
        async fn create_task(
            &self,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_task(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::task::Task,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::task::Task>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn get_active_tasks(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_all_tasks_for_job(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_next_task(
            &self,
            _parent_task_id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn create_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_log_parts(
            &self,
            _task_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn create_node(
            &self,
            _node: &twerk_core::node::Node,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_node(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::node::Node,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::node::Node>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_node_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::node::Node> {
            unimplemented!()
        }
        async fn get_active_nodes(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::node::Node>> {
            unimplemented!()
        }
        async fn create_job(
            &self,
            _job: &twerk_core::job::Job,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::Job,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::Job>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::Job> {
            unimplemented!()
        }
        async fn get_job_log_parts(
            &self,
            _job_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn get_jobs(
            &self,
            _current_user: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::JobSummary>,
        > {
            unimplemented!()
        }
        async fn delete_job(&self, _id: &str) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_scheduled_job(
            &self,
            _sj: &twerk_core::job::ScheduledJob,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_active_scheduled_jobs(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::job::ScheduledJob>> {
            unimplemented!()
        }
        async fn get_scheduled_jobs(
            &self,
            _current_user: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::ScheduledJobSummary>,
        > {
            unimplemented!()
        }
        async fn get_scheduled_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob> {
            unimplemented!()
        }
        async fn update_scheduled_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::ScheduledJob,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn delete_scheduled_job(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_user(
            &self,
            _user: &twerk_core::user::User,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_user(
            &self,
            username: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::user::User> {
            if username == "testuser" {
                Ok(self.user.clone())
            } else {
                Err(twerk_infrastructure::datastore::Error::UserNotFound)
            }
        }
        async fn create_role(
            &self,
            _role: &twerk_core::role::Role,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_role(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::role::Role> {
            unimplemented!()
        }
        async fn get_roles(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn get_user_roles(
            &self,
            _user_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn assign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn unassign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_metrics(
            &self,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
            unimplemented!()
        }
        async fn with_tx(
            &self,
            _f: Box<
                dyn for<'a> FnOnce(
                        &'a dyn twerk_infrastructure::datastore::Datastore,
                    ) -> futures_util::future::BoxFuture<
                        'a,
                        twerk_infrastructure::datastore::Result<()>,
                    > + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
    }

    let password = "testpassword";
    let hashed = bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("hashing should succeed");
    let user = User {
        username: Some("testuser".to_string()),
        password_hash: Some(hashed),
        ..Default::default()
    };

    let datastore = Arc::new(MockDatastore::new(user));
    let config = BasicAuthConfig::new(datastore.clone());

    let app = axum::Router::new().route("/test", get(ok_handler)).layer(
        axum::middleware::from_fn_with_state(config, |st, req, next| {
            Box::pin(async move { basic_auth_middleware(st, req, next).await })
        }),
    );

    let credentials = STANDARD.encode(format!("testuser:{password}"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/test")
                .header(header::AUTHORIZATION, format!("Basic {credentials}"))
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_basic_auth_credentials_return_401() {
    use async_trait::async_trait;
    use axum::http::{header, StatusCode};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use tower::ServiceExt;
    use twerk_core::user::User;

    struct MockDatastore {
        user: User,
    }

    impl MockDatastore {
        fn new(user: User) -> Self {
            Self { user }
        }
    }

    #[async_trait]
    impl twerk_infrastructure::datastore::Datastore for MockDatastore {
        async fn create_task(
            &self,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_task(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::task::Task,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::task::Task>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn get_active_tasks(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_all_tasks_for_job(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_next_task(
            &self,
            _parent_task_id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn create_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_log_parts(
            &self,
            _task_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn create_node(
            &self,
            _node: &twerk_core::node::Node,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_node(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::node::Node,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::node::Node>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_node_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::node::Node> {
            unimplemented!()
        }
        async fn get_active_nodes(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::node::Node>> {
            unimplemented!()
        }
        async fn create_job(
            &self,
            _job: &twerk_core::job::Job,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::Job,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::Job>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::Job> {
            unimplemented!()
        }
        async fn get_job_log_parts(
            &self,
            _job_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn get_jobs(
            &self,
            _current_user: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::JobSummary>,
        > {
            unimplemented!()
        }
        async fn delete_job(&self, _id: &str) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_scheduled_job(
            &self,
            _sj: &twerk_core::job::ScheduledJob,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_active_scheduled_jobs(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::job::ScheduledJob>> {
            unimplemented!()
        }
        async fn get_scheduled_jobs(
            &self,
            _current_user: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::ScheduledJobSummary>,
        > {
            unimplemented!()
        }
        async fn get_scheduled_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob> {
            unimplemented!()
        }
        async fn update_scheduled_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::ScheduledJob,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn delete_scheduled_job(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_user(
            &self,
            _user: &twerk_core::user::User,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_user(
            &self,
            username: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::user::User> {
            if username == "testuser" {
                Ok(self.user.clone())
            } else {
                Err(twerk_infrastructure::datastore::Error::UserNotFound)
            }
        }
        async fn create_role(
            &self,
            _role: &twerk_core::role::Role,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_role(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::role::Role> {
            unimplemented!()
        }
        async fn get_roles(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn get_user_roles(
            &self,
            _user_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn assign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn unassign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_metrics(
            &self,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
            unimplemented!()
        }
        async fn with_tx(
            &self,
            _f: Box<
                dyn for<'a> FnOnce(
                        &'a dyn twerk_infrastructure::datastore::Datastore,
                    ) -> futures_util::future::BoxFuture<
                        'a,
                        twerk_infrastructure::datastore::Result<()>,
                    > + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
    }

    let password = "testpassword";
    let hashed = bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("hashing should succeed");
    let user = User {
        username: Some("testuser".to_string()),
        password_hash: Some(hashed),
        ..Default::default()
    };

    let datastore = Arc::new(MockDatastore::new(user));
    let config = BasicAuthConfig::new(datastore.clone());

    let app = axum::Router::new().route("/test", get(ok_handler)).layer(
        axum::middleware::from_fn_with_state(config, |st, req, next| {
            Box::pin(async move { basic_auth_middleware(st, req, next).await })
        }),
    );

    let credentials = STANDARD.encode("testuser:wrongpassword");

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/test")
                .header(header::AUTHORIZATION, format!("Basic {credentials}"))
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_missing_basic_auth_credentials_return_401() {
    use axum::http::StatusCode;
    use tower::ServiceExt;
    use twerk_infrastructure::datastore::Error as DatastoreError;

    struct MockDatastore;

    #[async_trait::async_trait]
    impl twerk_infrastructure::datastore::Datastore for MockDatastore {
        async fn create_task(
            &self,
            _task: &twerk_core::task::Task,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_task(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::task::Task,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::task::Task>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn get_active_tasks(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_all_tasks_for_job(
            &self,
            _job_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::task::Task>> {
            unimplemented!()
        }
        async fn get_next_task(
            &self,
            _parent_task_id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::task::Task> {
            unimplemented!()
        }
        async fn create_task_log_part(
            &self,
            _part: &twerk_core::task::TaskLogPart,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_task_log_parts(
            &self,
            _task_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn create_node(
            &self,
            _node: &twerk_core::node::Node,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_node(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::node::Node,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::node::Node>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_node_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::node::Node> {
            unimplemented!()
        }
        async fn get_active_nodes(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::node::Node>> {
            unimplemented!()
        }
        async fn create_job(
            &self,
            _job: &twerk_core::job::Job,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn update_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::Job,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::Job>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::Job> {
            unimplemented!()
        }
        async fn get_job_log_parts(
            &self,
            _job_id: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::task::TaskLogPart>,
        > {
            unimplemented!()
        }
        async fn get_jobs(
            &self,
            _current_user: &str,
            _q: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::JobSummary>,
        > {
            unimplemented!()
        }
        async fn delete_job(&self, _id: &str) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_scheduled_job(
            &self,
            _sj: &twerk_core::job::ScheduledJob,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_active_scheduled_jobs(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::job::ScheduledJob>> {
            unimplemented!()
        }
        async fn get_scheduled_jobs(
            &self,
            _current_user: &str,
            _page: i64,
            _size: i64,
        ) -> twerk_infrastructure::datastore::Result<
            twerk_infrastructure::datastore::Page<twerk_core::job::ScheduledJobSummary>,
        > {
            unimplemented!()
        }
        async fn get_scheduled_job_by_id(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob> {
            unimplemented!()
        }
        async fn update_scheduled_job(
            &self,
            _id: &str,
            _modify: Box<
                dyn FnOnce(
                        twerk_core::job::ScheduledJob,
                    )
                        -> twerk_infrastructure::datastore::Result<twerk_core::job::ScheduledJob>
                    + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn delete_scheduled_job(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn create_user(
            &self,
            _user: &twerk_core::user::User,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_user(
            &self,
            _username: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::user::User> {
            Err(DatastoreError::UserNotFound)
        }
        async fn create_role(
            &self,
            _role: &twerk_core::role::Role,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_role(
            &self,
            _id: &str,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::role::Role> {
            unimplemented!()
        }
        async fn get_roles(
            &self,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn get_user_roles(
            &self,
            _user_id: &str,
        ) -> twerk_infrastructure::datastore::Result<Vec<twerk_core::role::Role>> {
            unimplemented!()
        }
        async fn assign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn unassign_role(
            &self,
            _user_id: &str,
            _role_id: &str,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn get_metrics(
            &self,
        ) -> twerk_infrastructure::datastore::Result<twerk_core::stats::Metrics> {
            unimplemented!()
        }
        async fn with_tx(
            &self,
            _f: Box<
                dyn for<'a> FnOnce(
                        &'a dyn twerk_infrastructure::datastore::Datastore,
                    ) -> futures_util::future::BoxFuture<
                        'a,
                        twerk_infrastructure::datastore::Result<()>,
                    > + Send,
            >,
        ) -> twerk_infrastructure::datastore::Result<()> {
            unimplemented!()
        }
        async fn health_check(&self) -> twerk_infrastructure::datastore::Result<()> {
            Ok(())
        }
    }

    let datastore: Arc<dyn twerk_infrastructure::datastore::Datastore> = Arc::new(MockDatastore);
    let config = BasicAuthConfig::new(datastore);

    let app = axum::Router::new().route("/test", get(ok_handler)).layer(
        axum::middleware::from_fn_with_state(config, |st, req, next| {
            Box::pin(async move { basic_auth_middleware(st, req, next).await })
        }),
    );

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/test")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_api_key_passes() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string());

    let app = axum::Router::new()
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/endpoint")
                .header("X-API-Key", "valid-api-key")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_api_key_returns_401() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string());

    let app = axum::Router::new()
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/endpoint")
                .header("X-API-Key", "invalid-api-key")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_missing_api_key_returns_401() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string());

    let app = axum::Router::new()
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/endpoint")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_request_matching_skip_path_bypasses_auth() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string())
        .with_skip_paths(vec!["GET /health".to_string()]);

    let app = axum::Router::new()
        .route("/health", get(ok_handler))
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/health")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_key_in_query_param_passes() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string());

    let app = axum::Router::new()
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/endpoint?api_key=valid-api-key")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_wildcard_skip_path_matching() {
    use axum::http::StatusCode;

    let config = KeyAuthConfig::new("valid-api-key".to_string())
        .with_skip_paths(vec!["GET /health*".to_string()]);

    let app = axum::Router::new()
        .route("/healthz", get(ok_handler))
        .route("/api/endpoint", get(ok_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("middleware should not panic");

    assert_eq!(response.status(), StatusCode::OK);
}
