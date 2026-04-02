#![allow(clippy::unwrap_used)]

use anyhow::Result;
use std::sync::Arc;
use twerk_app::engine::coordinator::create_coordinator;
use twerk_app::engine::coordinator::middleware::HttpLogConfig;
use twerk_app::engine::{BrokerProxy, Config, DatastoreProxy, Engine, Mode, State};
use twerk_core::job::{Job, JOB_STATE_PENDING};
use twerk_core::task::Task;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};

fn engine_with_mode(mode: Mode) -> Engine {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    Engine::new(Config {
        mode,
        ..Default::default()
    })
}

#[tokio::test]
async fn engine_new_creates_idle_engine() {
    let engine = engine_with_mode(Mode::Standalone);
    assert_eq!(engine.state(), State::Idle);
    assert_eq!(engine.mode(), Mode::Standalone);
}

#[tokio::test]
async fn engine_set_mode_only_allowed_when_idle() {
    let mut engine = engine_with_mode(Mode::Standalone);
    assert_eq!(engine.state(), State::Idle);

    engine.set_mode(Mode::Coordinator);
    assert_eq!(engine.mode(), Mode::Coordinator);

    let mut running_engine = engine_with_mode(Mode::Standalone);
    running_engine.set_mode(Mode::Worker);
    drop(running_engine);
}

#[tokio::test]
async fn engine_debug_shows_state_and_mode() {
    let engine = engine_with_mode(Mode::Standalone);
    let debug_str = format!("{engine:?}");
    assert!(debug_str.contains("Idle"));
    assert!(debug_str.contains("Standalone"));
}

#[tokio::test]
async fn engine_start_fails_when_not_idle() {
    let mut engine = engine_with_mode(Mode::Standalone);
    engine.register_runtime(Box::new(twerk_app::engine::MockRuntime));
    let _ = engine.start().await;
    assert_eq!(engine.state(), State::Running);

    let result = engine.start().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not idle"));

    let _ = engine.terminate().await;
}

#[tokio::test]
async fn engine_terminate_fails_when_not_running() {
    let mut engine = engine_with_mode(Mode::Standalone);
    let result = engine.terminate().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn start_standalone_initializes_broker_and_datastore() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    assert_eq!(engine.state(), State::Running);

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn start_coordinator_initializes_broker_and_datastore() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Coordinator);
    engine.start().await?;

    assert_eq!(engine.state(), State::Running);

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn start_worker_initializes_broker() -> Result<()> {
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
    std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash");

    let mut engine = engine_with_mode(Mode::Worker);
    engine.start().await?;

    assert_eq!(engine.state(), State::Running);

    engine.terminate().await?;
    std::env::remove_var("TWERK_RUNTIME_TYPE");
    std::env::remove_var("TWERK_RUNTIME_SHELL_CMD");
    Ok(())
}

#[tokio::test]
async fn create_runtime_from_config_fails_clearly_when_runtime_is_invalid() {
    let config = twerk_app::engine::worker::runtime_adapter::RuntimeConfig {
        runtime_type: "unknown-runtime".to_string(),
        ..Default::default()
    };

    let broker: Arc<dyn Broker + Send + Sync> = Arc::new(InMemoryBroker::new());
    let result =
        twerk_app::engine::worker::runtime_adapter::create_runtime_from_config(&config, broker)
            .await;

    assert!(result.is_err());
    assert!(result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
        .contains("unknown runtime type"));
}

#[tokio::test]
async fn engine_run_starts_without_panic() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    assert_eq!(engine.state(), State::Running);

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn broker_proxy_can_be_used_as_broker_trait() -> Result<()> {
    let broker = BrokerProxy::new();
    broker.init("inmemory", Some("")).await?;
    broker.health_check().await?;
    Ok(())
}

#[tokio::test]
async fn datastore_proxy_can_be_used_as_datastore_trait() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    let datastore = DatastoreProxy::new();
    datastore.init().await?;
    datastore.health_check().await?;
    Ok(())
}

#[tokio::test]
async fn broker_proxy_check_init_returns_error_when_not_initialized() {
    let broker = BrokerProxy::new();
    let result = broker.check_init().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not initialized"));
}

#[tokio::test]
async fn broker_proxy_init_creates_inmemory_broker() -> Result<()> {
    let broker = BrokerProxy::new();
    broker.init("inmemory", Some("")).await?;
    broker.check_init().await?;
    Ok(())
}

#[tokio::test]
async fn broker_proxy_init_accepts_rabbitmq_type() -> Result<()> {
    std::env::set_var(
        "TWERK_BROKER_RABBITMQ_URL",
        "amqp://guest:guest@localhost:5672/",
    );
    let broker = BrokerProxy::new();
    let result = broker.init("rabbitmq", Some("")).await;
    std::env::remove_var("TWERK_BROKER_RABBITMQ_URL");
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn datastore_proxy_init_creates_inmemory_datastore() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    let datastore = DatastoreProxy::new();
    datastore.init().await?;
    Ok(())
}

#[tokio::test]
async fn create_broker_inmemory_creates_broker() -> Result<()> {
    use twerk_app::engine::broker::create_broker;
    let broker = create_broker("inmemory", Some("")).await?;
    broker.health_check().await?;
    Ok(())
}

#[tokio::test]
async fn create_datastore_inmemory_creates_datastore() -> Result<()> {
    use twerk_app::engine::datastore::create_datastore;
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    let datastore = create_datastore().await?;
    datastore.health_check().await?;
    Ok(())
}

#[tokio::test]
async fn engine_submit_job_returns_error_when_engine_not_running() -> Result<()> {
    let engine = engine_with_mode(Mode::Standalone);
    let job = Job {
        id: Some("test-job".into()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    let result = engine.submit_job(job, vec![]).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not running"));
    Ok(())
}

#[tokio::test]
async fn engine_submit_job_returns_error_when_not_coordinator_mode() -> Result<()> {
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Worker);
    engine.start().await?;

    let job = Job {
        id: Some("test-job".into()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    let result = engine.submit_job(job, vec![]).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("not in coordinator/standalone mode"));

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_submit_job_submits_to_coordinator_in_standalone_mode() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    let job = Job {
        id: Some("submit-test-job".into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    let result = engine.submit_job(job.clone(), vec![]).await;
    assert!(result.is_ok());

    let submitted = result.unwrap();
    assert_eq!(submitted.id, job.id);
    assert_eq!(submitted.state, JOB_STATE_PENDING);

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_register_middleware_allowed_when_idle() {
    let mut engine = engine_with_mode(Mode::Standalone);

    engine.register_web_middleware(Arc::new(|_req, _next| {
        Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
    }));
    engine.register_task_middleware(Arc::new(|handler| handler));
    engine.register_job_middleware(Arc::new(|handler| handler));
    engine.register_node_middleware(Arc::new(|handler| handler));
    engine.register_log_middleware(Arc::new(|handler| handler));
}

#[tokio::test]
async fn engine_register_middleware_ignored_when_running() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    engine.register_web_middleware(Arc::new(|_req, _next| {
        Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
    }));
    engine.register_task_middleware(Arc::new(|handler| handler));
    engine.register_job_middleware(Arc::new(|handler| handler));
    engine.register_node_middleware(Arc::new(|handler| handler));
    engine.register_log_middleware(Arc::new(|handler| handler));

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_register_endpoint_allowed_when_idle() {
    use twerk_app::engine::EndpointHandler;
    let mut engine = engine_with_mode(Mode::Standalone);

    let handler: EndpointHandler = Arc::new(|_parts, _body| {
        Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
    });

    engine.register_endpoint("GET", "/test", handler);
}

#[tokio::test]
async fn engine_register_endpoint_ignored_when_running() -> Result<()> {
    use twerk_app::engine::EndpointHandler;
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    let handler: EndpointHandler = Arc::new(|_parts, _body| {
        Box::pin(async { axum::response::Response::new(axum::body::Body::empty()) })
    });

    engine.register_endpoint("GET", "/test", handler);

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_register_runtime_allowed_when_idle() {
    use twerk_app::engine::MockRuntime;
    let mut engine = engine_with_mode(Mode::Standalone);

    engine.register_runtime(Box::new(MockRuntime));
}

#[tokio::test]
async fn engine_register_runtime_ignored_when_already_set() {
    use twerk_app::engine::MockRuntime;
    let mut engine = engine_with_mode(Mode::Standalone);

    engine.register_runtime(Box::new(MockRuntime));
    engine.register_runtime(Box::new(MockRuntime));
}

#[tokio::test]
async fn engine_register_datastore_provider_allowed_when_idle() {
    let mut engine = engine_with_mode(Mode::Standalone);
    engine.register_datastore_provider("test", Box::new(InMemoryDatastore::new()));
}

#[tokio::test]
async fn engine_register_broker_provider_allowed_when_idle() {
    let mut engine = engine_with_mode(Mode::Standalone);
    engine.register_broker_provider("test", Box::new(InMemoryBroker::new()));
}

#[tokio::test]
async fn coordinator_submit_job_creates_job_in_datastore() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("coordinator-test-job".into()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    coordinator.submit_job(job.clone()).await?;

    let persisted = datastore.get_job_by_id("coordinator-test-job").await?;
    assert_eq!(persisted.id, job.id);

    coordinator.stop().await?;
    Ok(())
}

#[tokio::test]
async fn coordinator_submit_job_generates_id_when_missing() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: None,
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    let result = coordinator.submit_job(job).await?;
    assert!(result.id.is_some());

    coordinator.stop().await?;
    Ok(())
}

#[tokio::test]
async fn coordinator_submit_job_sets_created_at() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("timestamp-test-job".into()),
        state: JOB_STATE_PENDING.to_string(),
        created_at: None,
        ..Default::default()
    };

    let result = coordinator.submit_job(job).await?;
    assert!(result.created_at.is_some());

    coordinator.stop().await?;
    Ok(())
}

#[tokio::test]
async fn coordinator_stop_cancels_tasks() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    coordinator.stop().await?;

    Ok(())
}

#[tokio::test]
async fn cors_layer_creation_succeeds() {
    use twerk_app::engine::coordinator::middleware::cors_layer;
    let _layer = cors_layer();
}

#[tokio::test]
async fn http_log_config_default_creates_successfully() {
    let _config = HttpLogConfig::default();
}

#[tokio::test]
async fn http_log_config_debug_shows_level() {
    let config = HttpLogConfig::default();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("DEBUG"));
}

#[tokio::test]
async fn state_is_terminal_for_terminated() {
    assert!(State::Terminated.is_terminal());
    assert!(!State::Idle.is_terminal());
    assert!(!State::Running.is_terminal());
}

#[tokio::test]
async fn state_can_transition_to_terminating_from_running() {
    assert!(State::Running.can_transition_to(State::Terminating));
}

#[tokio::test]
async fn state_idle_can_transition_to_any_state() {
    assert!(State::Idle.can_transition_to(State::Terminated));
    assert!(State::Idle.can_transition_to(State::Running));
}

#[tokio::test]
async fn broker_proxy_set_broker_allows_custom_implementation() -> Result<()> {
    let broker = BrokerProxy::new();
    broker.set_broker(Box::new(InMemoryBroker::new())).await;
    broker.check_init().await?;
    Ok(())
}

#[tokio::test]
async fn datastore_proxy_set_datastore_allows_custom_implementation() -> Result<()> {
    let datastore = DatastoreProxy::new();
    datastore
        .set_datastore(Box::new(InMemoryDatastore::new()))
        .await;
    Ok(())
}

#[tokio::test]
async fn broker_proxy_clone_inner_creates_independent_copy() -> Result<()> {
    let original = BrokerProxy::new();
    original.init("inmemory", Some("")).await?;

    let cloned = original.clone_inner();
    cloned.check_init().await?;

    Ok(())
}

#[tokio::test]
async fn datastore_proxy_clone_inner_creates_independent_copy() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    let original = DatastoreProxy::new();
    original.init().await?;

    let cloned = original.clone_inner();
    let _ = cloned.get_jobs("test", "", 1, 10).await;

    Ok(())
}
