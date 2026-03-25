//! Coordinator configuration logic.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;

use tork::broker::Broker;
use tork::datastore::Datastore;

use tork_runtime::conf;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during coordinator operations.
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("broker error: {0}")]
    Broker(String),

    #[error("datastore error: {0}")]
    Datastore(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("handler error: {0}")]
    Handler(String),

    #[error("config error: {0}")]
    Config(String),
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Broker-level task handler type.
pub type BrokerTaskHandler = tork::broker::TaskHandler;

/// Broker-level job handler type.
pub type BrokerJobHandler = tork::broker::JobHandler;

/// Broker-level node handler type.
pub type BrokerNodeHandler = tork::broker::HeartbeatHandler;

/// Broker-level log handler type.
pub type BrokerLogHandler = tork::broker::TaskLogPartHandler;

/// Middleware chains for handler types.
#[derive(Clone, Default)]
pub struct Middleware {
    /// Middleware for job handlers
    pub job: Vec<Arc<dyn Fn(BrokerJobHandler) -> BrokerJobHandler + Send + Sync>>,
    /// Middleware for task handlers
    pub task: Vec<Arc<dyn Fn(BrokerTaskHandler) -> BrokerTaskHandler + Send + Sync>>,
    /// Middleware for node handlers
    pub node: Vec<Arc<dyn Fn(BrokerNodeHandler) -> BrokerNodeHandler + Send + Sync>>,
    /// Middleware for log handlers
    pub log: Vec<Arc<dyn Fn(BrokerLogHandler) -> BrokerLogHandler + Send + Sync>>,
}

impl std::fmt::Debug for Middleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Middleware")
            .field("job", &format!("[{} middleware fns]", self.job.len()))
            .field("task", &format!("[{} middleware fns]", self.task.len()))
            .field("node", &format!("[{} middleware fns]", self.node.len()))
            .field("log", &format!("[{} middleware fns]", self.log.len()))
            .finish()
    }
}

impl Middleware {
    /// Apply the job middleware chain to a handler.
    #[must_use]
    pub fn apply_job(&self, handler: BrokerJobHandler) -> BrokerJobHandler {
        self.job.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the task middleware chain to a handler.
    #[must_use]
    pub fn apply_task(&self, handler: BrokerTaskHandler) -> BrokerTaskHandler {
        self.task.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the node middleware chain to a handler.
    #[must_use]
    pub fn apply_node(&self, handler: BrokerNodeHandler) -> BrokerNodeHandler {
        self.node.iter().fold(handler, |h, mw| mw(h))
    }

    /// Apply the log middleware chain to a handler.
    #[must_use]
    pub fn apply_log(&self, handler: BrokerLogHandler) -> BrokerLogHandler {
        self.log.iter().fold(handler, |h, mw| mw(h))
    }
}

// ---------------------------------------------------------------------------
// API Endpoints
// ---------------------------------------------------------------------------

/// Configuration for enabling/disabling API endpoint groups.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiEndpoints {
    /// Enable health check endpoint (`GET /health`).
    #[serde(default = "default_true")]
    pub health: bool,
    /// Enable job management endpoints (`/jobs/*`).
    #[serde(default = "default_true")]
    pub jobs: bool,
    /// Enable task endpoints (`/tasks/*`).
    #[serde(default = "default_true")]
    pub tasks: bool,
    /// Enable node listing endpoint (`GET /nodes`).
    #[serde(default = "default_true")]
    pub nodes: bool,
    /// Enable queue management endpoints (`/queues/*`).
    #[serde(default = "default_true")]
    pub queues: bool,
    /// Enable metrics endpoint (`GET /metrics`).
    #[serde(default = "default_true")]
    pub metrics: bool,
    /// Enable user management endpoints (`/users`).
    #[serde(default = "default_true")]
    pub users: bool,
    /// Enable scheduled job endpoints (`/scheduled-jobs/*`).
    #[serde(default = "default_true")]
    pub scheduled_jobs: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for ApiEndpoints {
    fn default() -> Self {
        Self {
            health: true,
            jobs: true,
            tasks: true,
            nodes: true,
            queues: true,
            metrics: true,
            users: true,
            scheduled_jobs: true,
        }
    }
}

impl ApiEndpoints {
    /// Convert to the `HashMap<String, bool>` format used by the API router.
    #[must_use]
    pub fn to_enabled_map(&self) -> HashMap<String, bool> {
        let mut map = HashMap::new();
        map.insert("health".to_string(), self.health);
        map.insert("jobs".to_string(), self.jobs);
        map.insert("tasks".to_string(), self.tasks);
        map.insert("nodes".to_string(), self.nodes);
        map.insert("queues".to_string(), self.queues);
        map.insert("metrics".to_string(), self.metrics);
        map.insert("users".to_string(), self.users);
        map.insert("scheduled_jobs".to_string(), self.scheduled_jobs);
        map
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Coordinator configuration.
pub struct Config {
    /// Coordinator name
    pub name: String,
    /// Message broker
    pub broker: Arc<dyn Broker>,
    /// Persistent datastore
    pub datastore: Arc<dyn Datastore>,
    /// Distributed locker
    pub locker: Arc<dyn locker::Locker>,
    /// API listen address (e.g. "0.0.0.0:8000")
    pub address: String,
    /// Queue concurrency settings (queue name → number of consumers)
    pub queues: HashMap<String, i64>,
    /// Enabled API endpoints (legacy map format)
    pub enabled: HashMap<String, bool>,
    /// API endpoint toggling configuration
    pub endpoints: ApiEndpoints,
    /// Middleware chains for handlers
    pub middleware: Middleware,
}

#[derive(Deserialize)]
struct ConfigToml {
    #[serde(default)]
    name: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    queues: HashMap<String, i64>,
    #[serde(default)]
    enabled: HashMap<String, bool>,
    #[serde(default)]
    endpoints: ApiEndpoints,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("name", &self.name)
            .field("address", &self.address)
            .field("queues", &self.queues)
            .field("enabled", &self.enabled)
            .field("endpoints", &self.endpoints)
            .field("broker", &"<dyn Broker>")
            .field("datastore", &"<dyn Datastore>")
            .field("locker", &"<dyn Locker>")
            .finish()
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            broker: self.broker.clone(),
            datastore: self.datastore.clone(),
            locker: self.locker.clone(),
            address: self.address.clone(),
            queues: self.queues.clone(),
            enabled: self.enabled.clone(),
            endpoints: self.endpoints.clone(),
            middleware: self.middleware.clone(),
        }
    }
}

impl Config {
    /// Load coordinator configuration from the global config.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinatorError::Config`] if unmarshaling fails.
    pub fn load(
        broker: Arc<dyn Broker>,
        ds: Arc<dyn Datastore>,
        locker: Arc<dyn locker::Locker>,
    ) -> Result<Self, CoordinatorError> {
        let toml: ConfigToml =
            conf::unmarshal("coordinator").map_err(|e| CoordinatorError::Config(e.to_string()))?;
        Ok(Self {
            name: toml.name,
            broker,
            datastore: ds,
            locker,
            address: toml.address,
            queues: toml.queues,
            enabled: toml.enabled,
            endpoints: toml.endpoints,
            middleware: Middleware::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::Mutex;
    use tork_runtime::conf::{self, ConfigError};
    use tork::job::{Job, ScheduledJob};
    use tork::node::Node;
    use tork::task::Task;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup() {
        // Clear TORK_* env vars
        for (k, _) in env::vars() {
            if k.starts_with("TORK_") {
                env::remove_var(&k);
            }
        }
    }

    fn write_config(path: &std::path::Path, contents: &str) {
        fs::write(path, contents).expect("failed to write config file");
    }

    #[test]
    fn test_load_config_not_exist() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let result = conf::load_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_config_not_exist_user_defined() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("no.such.thing");
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        let result = conf::load_config();
        assert!(result.is_err());
        if let Err(ConfigError::UserConfigNotFound(p)) = result {
            assert_eq!(p, path.to_string_lossy());
        } else {
            panic!("expected UserConfigNotFound error, got {:?}", result);
        }
    }

    #[test]
    fn test_load_config_bad_contents() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(&path, "xyz");
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        let result = conf::load_config();
        assert!(result.is_err());
    }

    #[test]
    fn test_string() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
key1 = "value1"
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert_eq!("value1", conf::string("main.key1"));
    }

    #[test]
    fn test_strings() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config_strings.toml");
        write_config(
            &path,
            r#"
[main]
keys = ["value1"]
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert_eq!(vec!["value1"], conf::strings("main.keys"));
    }

    #[test]
    fn test_strings_env() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        env::set_var("TORK_MAIN_STRINGS_KEYS", "a,b,c");
        conf::load_config().unwrap();
        assert_eq!(vec!["a", "b", "c"], conf::strings("main.strings.keys"));
    }

    #[test]
    fn test_string_default() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
key1 = "value1"
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert_eq!("v2", conf::string_default("main.key2", "v2"));
    }

    #[test]
    fn test_int_map() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
map.key1 = 1
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        let result = conf::int_map("main.map");
        assert_eq!(1, result.get("key1").copied().unwrap_or(0));
    }

    #[test]
    fn test_load_config_env() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        env::set_var("TORK_HELLO", "world");
        conf::load_config().unwrap();
        assert_eq!("world", conf::string("hello"));
    }

    #[test]
    fn test_load_config_with_overriding_env() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config_with_override.toml");
        write_config(
            &path,
            r#"
[main]
key1 = "value1"
key3 = "value3"
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        env::set_var("TORK_MAIN_KEY1", "value2");
        conf::load_config().unwrap();
        assert_eq!("value2", conf::string("main.key1"));
        assert_eq!("value3", conf::string("main.key3"));
    }

    #[test]
    fn test_bool_true() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
enabled = true
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert!(conf::bool("main.enabled"));
    }

    #[test]
    fn test_bool_false() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
enabled = false
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert!(!conf::bool("main.enabled"));
    }

    #[test]
    fn test_bool_default() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
enabled = false
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert!(!conf::bool_default("main.enabled", true));
        assert!(!conf::bool_default("main.enabled", false));
        assert!(conf::bool_default("main.other", true));
    }

    #[test]
    fn test_duration_default() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[main]
some.duration = "5m"
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();
        assert_eq!(
            time::Duration::minutes(5),
            conf::duration_default("main.some.duration", time::Duration::seconds(60))
        );
        assert_eq!(
            time::Duration::seconds(60),
            conf::duration_default("main.other.duration", time::Duration::seconds(60))
        );
    }

    #[test]
    fn test_bool_map() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        env::set_var("TORK_BOOLMAP_KEY1", "false");
        env::set_var("TORK_BOOLMAP_KEY2", "true");
        conf::load_config().unwrap();
        let m = conf::bool_map("boolmap");
        assert_eq!(false, m.get("key1").copied().unwrap_or(true));
        assert_eq!(true, m.get("key2").copied().unwrap_or(false));
    }

    struct DummyBroker;
    impl tork::broker::Broker for DummyBroker {
        fn publish_task(&self, _q: String, _t: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(&self, _q: String, _h: tork::broker::TaskHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _t: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(&self, _h: tork::broker::TaskProgressHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _n: Node) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(&self, _h: tork::broker::HeartbeatHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _j: &Job) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(&self, _h: tork::broker::JobHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(&self, _topic: String, _ev: serde_json::Value) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(&self, _p: String, _h: tork::broker::EventHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(&self, _p: &tork::task::TaskLogPart) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(&self, _h: tork::broker::TaskLogPartHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn queue_info(&self, _q: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> {
            Box::pin(async {
                Ok(tork::broker::QueueInfo {
                    name: "test".into(),
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            })
        }
        fn delete_queue(&self, _q: String) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    struct DummyDatastore;
    impl tork::datastore::Datastore for DummyDatastore {
        fn create_task(&self, _t: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_task(&self, _id: String, _t: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_tasks(&self, _job_id: String) -> tork::datastore::BoxedFuture<Vec<Task>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_next_task(&self, _parent_task_id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(&self, _p: tork::task::TaskLogPart) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self,
            _task_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 10,
                })
            })
        }
        fn create_node(&self, _n: Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_node(&self, _id: String, _n: Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<Node>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_job(&self, _j: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, _id: String, _j: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Job>> {
            Box::pin(async { Ok(None) })
        }
        fn get_job_log_parts(
            &self,
            _job_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 10,
                })
            })
        }
        fn get_jobs(
            &self,
            _current_user: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 10,
                })
            })
        }
        fn create_scheduled_job(&self, _j: ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self,
            _current_user: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>>
        {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 10,
                })
            })
        }
        fn get_scheduled_job_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(
            &self,
            _id: String,
            _j: ScheduledJob,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _u: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(&self, _username: String) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
            Box::pin(async { Ok(None) })
        }
        fn create_role(&self, _r: tork::role::Role) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_role(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::role::Role>> {
            Box::pin(async { Ok(None) })
        }
        fn get_roles(&self) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn assign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_metrics(&self) -> tork::datastore::BoxedFuture<tork::stats::Metrics> {
            Box::pin(async {
                Ok(tork::stats::Metrics {
                    jobs: tork::stats::JobMetrics { running: 0 },
                    tasks: tork::stats::TaskMetrics { running: 0 },
                    nodes: tork::stats::NodeMetrics {
                        running: 0,
                        cpu_percent: 0.0,
                    },
                })
            })
        }
        fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    struct DummyLocker;
    struct DummyLock;
    impl locker::Lock for DummyLock {
        fn release_lock(
            self: std::pin::Pin<Box<Self>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), locker::LockError>> + Send>>
        {
            Box::pin(async { Ok(()) })
        }
    }
    impl locker::Locker for DummyLocker {
        fn acquire_lock(&self, _key: &str) -> locker::AcquireLockFuture {
            Box::pin(async { Ok(Box::pin(DummyLock) as std::pin::Pin<Box<dyn locker::Lock>>) })
        }
    }

    #[test]
    fn test_unmarshal() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[coordinator]
name = "my-coordinator"
address = "0.0.0.0:8001"
[coordinator.queues]
default = 5
[coordinator.endpoints]
health = false
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        conf::load_config().unwrap();

        let cfg = Config::load(
            Arc::new(DummyBroker),
            Arc::new(DummyDatastore),
            Arc::new(DummyLocker),
        )
        .unwrap();
        assert_eq!(cfg.name, "my-coordinator");
        assert_eq!(cfg.address, "0.0.0.0:8001");
        assert_eq!(cfg.queues.get("default"), Some(&5));
        assert!(!cfg.endpoints.health);
        assert!(cfg.endpoints.jobs); // default true
    }

    #[test]
    fn test_load_coordinator_config_overriding_env() {
        let _guard = TEST_MUTEX.lock().unwrap();
        setup();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        write_config(
            &path,
            r#"
[coordinator]
name = "my-coordinator"
"#,
        );
        env::set_var("TORK_CONFIG", path.to_string_lossy().as_ref());
        env::set_var("TORK_COORDINATOR_NAME", "overridden");
        conf::load_config().unwrap();

        let cfg = Config::load(
            Arc::new(DummyBroker),
            Arc::new(DummyDatastore),
            Arc::new(DummyLocker),
        )
        .unwrap();
        assert_eq!(cfg.name, "overridden");
    }
}
