//! Broker proxy tests
//!
//! Tests for [`BrokerProxy`], [`InMemoryBroker`], [`RabbitMQBroker`], and broker factory functions.

use crate::broker::{
    BrokerProxy, InMemoryBroker, BrokerType, RabbitMQOptions,
    create_broker, env_string, env_string_default, env_duration_ms_default, env_bool,
};
use tork::broker::{Broker, TaskHandler, TaskProgressHandler, HeartbeatHandler, JobHandler, EventHandler};
use tork::task::Task;
use tork::job::Job;
use tork::node::Node;

#[tokio::test]
async fn test_broker_proxy_new() {
    let proxy = BrokerProxy::new();
    // Should not be initialized
    let result = proxy.health_check().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_broker_proxy_init_inmemory() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    // Should now be healthy
    let result = proxy.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_check_init_when_not_initialized() {
    let proxy = BrokerProxy::new();
    let result = proxy.check_init().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not initialized"));
}

#[tokio::test]
async fn test_broker_proxy_check_init_after_init() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    let result = proxy.check_init().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_set_broker() {
    let proxy = BrokerProxy::new();
    proxy.set_broker(Box::new(InMemoryBroker::new())).await;
    
    let result = proxy.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_clone_inner() {
    let proxy = BrokerProxy::new();
    let cloned = proxy.clone_inner();
    
    // Both should be independent but uninitialized
    let result1 = proxy.check_init().await;
    let result2 = cloned.check_init().await;
    assert!(result1.is_err());
    assert!(result2.is_err());
}

#[tokio::test]
async fn test_broker_proxy_publish_task_not_initialized() {
    let proxy = BrokerProxy::new();
    let task = Task::default();
    let result = proxy.publish_task("test-queue".to_string(), &task).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_broker_proxy_publish_task() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let task = Task::default();
    let result = proxy.publish_task("test-queue".to_string(), &task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_publish_task_progress() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let task = Task::default();
    let result = proxy.publish_task_progress(&task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_publish_job() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let job = Job::default();
    let result = proxy.publish_job(&job).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_subscribe_for_tasks() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let handler: TaskHandler = Arc::new(|_qname, _task| Box::pin(async { Ok(()) }));
    let result = proxy.subscribe_for_tasks("test-queue".to_string(), handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_subscribe_for_task_progress() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let handler: TaskProgressHandler = Arc::new(|_task| Box::pin(async { Ok(()) }));
    let result = proxy.subscribe_for_task_progress(handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_publish_heartbeat() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let node = Node::default();
    let result = proxy.publish_heartbeat(node).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_subscribe_for_heartbeats() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let handler: HeartbeatHandler = Arc::new(|_node| Box::pin(async { Ok(()) }));
    let result = proxy.subscribe_for_heartbeats(handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_subscribe_for_jobs() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let handler: JobHandler = Arc::new(|_job| Box::pin(async { Ok(()) }));
    let result = proxy.subscribe_for_jobs(handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_publish_event() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let event = serde_json::json!({"type": "test"});
    let result = proxy.publish_event("test.topic".to_string(), event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_subscribe_for_events() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let handler: EventHandler = Arc::new(|_event| Box::pin(async { Ok(()) }));
    let result = proxy.subscribe_for_events("test.*".to_string(), handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_queues() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let result = proxy.queues().await;
    assert!(result.is_ok());
    let queues = result.unwrap();
    assert!(queues.is_empty());
}

#[tokio::test]
async fn test_broker_proxy_queue_info() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let result = proxy.queue_info("test-queue".to_string()).await;
    assert!(result.is_ok());
    let info = result.unwrap();
    assert_eq!(info.name, "test-queue");
}

#[tokio::test]
async fn test_broker_proxy_delete_queue() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let result = proxy.delete_queue("test-queue".to_string()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_broker_proxy_shutdown() {
    let proxy = BrokerProxy::new();
    proxy.init("inmemory").await.expect("should init");
    
    let result = proxy.shutdown().await;
    assert!(result.is_ok());
}

// ── InMemoryBroker tests ─────────────────────────────────────────

#[tokio::test]
async fn test_in_memory_broker_default() {
    let broker = InMemoryBroker::default();
    let result = broker.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_publish_task() {
    let broker = InMemoryBroker::new();
    let task = Task::default();
    let result = broker.publish_task("test-queue".to_string(), &task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_subscribe_for_tasks() {
    let broker = InMemoryBroker::new();
    let handler: TaskHandler = Arc::new(|_qname, _task| Box::pin(async { Ok(()) }));
    let result = broker.subscribe_for_tasks("test-queue".to_string(), handler).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_publish_task_progress() {
    let broker = InMemoryBroker::new();
    let task = Task::default();
    let result = broker.publish_task_progress(&task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_publish_heartbeat() {
    let broker = InMemoryBroker::new();
    let node = Node::default();
    let result = broker.publish_heartbeat(node).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_publish_job() {
    let broker = InMemoryBroker::new();
    let job = Job::default();
    let result = broker.publish_job(&job).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_publish_event() {
    let broker = InMemoryBroker::new();
    let event = serde_json::json!({"key": "value"});
    let result = broker.publish_event("test.topic".to_string(), event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_queues() {
    let broker = InMemoryBroker::new();
    let result = broker.queues().await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_in_memory_broker_queue_info() {
    let broker = InMemoryBroker::new();
    let result = broker.queue_info("test-queue".to_string()).await;
    assert!(result.is_ok());
    let info = result.unwrap();
    assert_eq!(info.name, "test-queue");
    assert_eq!(info.size, 0);
    assert_eq!(info.subscribers, 0);
    assert_eq!(info.unacked, 0);
}

#[tokio::test]
async fn test_in_memory_broker_delete_queue() {
    let broker = InMemoryBroker::new();
    let result = broker.delete_queue("test-queue".to_string()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_in_memory_broker_shutdown() {
    let broker = InMemoryBroker::new();
    let result = broker.shutdown().await;
    assert!(result.is_ok());
}

// ── BrokerType tests ────────────────────────────────────────────

#[test]
fn test_broker_type_from_str_inmemory() {
    assert_eq!(BrokerType::from_str("inmemory"), BrokerType::InMemory);
    assert_eq!(BrokerType::from_str("INMEMORY"), BrokerType::InMemory);
    assert_eq!(BrokerType::from_str("InMemory"), BrokerType::InMemory);
}

#[test]
fn test_broker_type_from_str_rabbitmq() {
    assert_eq!(BrokerType::from_str("rabbitmq"), BrokerType::RabbitMQ);
    assert_eq!(BrokerType::from_str("RABBITMQ"), BrokerType::RabbitMQ);
    assert_eq!(BrokerType::from_str("RabbitMQ"), BrokerType::RabbitMQ);
}

#[test]
fn test_broker_type_from_str_unknown_defaults_to_inmemory() {
    assert_eq!(BrokerType::from_str("unknown"), BrokerType::InMemory);
    assert_eq!(BrokerType::from_str(""), BrokerType::InMemory);
}

// ── RabbitMQOptions tests ───────────────────────────────────────

#[test]
fn test_rabbitmq_options_default() {
    let opts = RabbitMQOptions::default();
    assert!(opts.management_url.is_none());
    assert!(!opts.durable_queues);
    assert_eq!(opts.queue_type, "classic");
}

// ── Config helper tests ──────────────────────────────────────────

#[test]
fn test_env_string_unset() {
    std::env::remove_var("TORK_TEST_UNSET");
    assert_eq!(env_string("test.unset"), "");
}

#[test]
fn test_env_string_set() {
    std::env::set_var("TORK_TEST_SET", "value123");
    assert_eq!(env_string("test.set"), "value123");
    std::env::remove_var("TORK_TEST_SET");
}

#[test]
fn test_env_string_converts_dots_to_underscores() {
    std::env::set_var("TORK_TEST_DOT_VALUE", "dot_value");
    assert_eq!(env_string("test.dot.value"), "dot_value");
    std::env::remove_var("TORK_TEST_DOT_VALUE");
}

#[test]
fn test_env_string_default_empty() {
    assert_eq!(env_string_default("test.default", "fallback"), "fallback");
}

#[test]
fn test_env_string_default_set() {
    std::env::set_var("TORK_TEST_DEFAULT", "custom");
    assert_eq!(env_string_default("test.default", "fallback"), "custom");
    std::env::remove_var("TORK_TEST_DEFAULT");
}

#[test]
fn test_env_duration_ms_default_empty() {
    let dur = env_duration_ms_default("test.dur.empty", 5000);
    assert_eq!(dur.as_millis(), 5000);
}

#[test]
fn test_env_duration_ms_default_set() {
    std::env::set_var("TORK_TEST_DUR", "1000");
    let dur = env_duration_ms_default("test.dur", 5000);
    assert_eq!(dur.as_millis(), 1000);
    std::env::remove_var("TORK_TEST_DUR");
}

#[test]
fn test_env_duration_ms_default_invalid() {
    std::env::set_var("TORK_TEST_DUR_INVALID", "not_a_number");
    let dur = env_duration_ms_default("test.dur.invalid", 5000);
    assert_eq!(dur.as_millis(), 5000);
    std::env::remove_var("TORK_TEST_DUR_INVALID");
}

#[test]
fn test_env_bool_default_false() {
    std::env::remove_var("TORK_TEST_BOOL_FALSE");
    assert!(!env_bool("test.bool.false", false));
}

#[test]
fn test_env_bool_default_true() {
    std::env::remove_var("TORK_TEST_BOOL_TRUE");
    assert!(env_bool("test.bool.true", true));
}

#[test]
fn test_env_bool_true_string() {
    std::env::set_var("TORK_TEST_BOOL_TRUE_STRING", "true");
    assert!(env_bool("test.bool.true.string", false));
    std::env::remove_var("TORK_TEST_BOOL_TRUE_STRING");
}

#[test]
fn test_env_bool_one_string() {
    std::env::set_var("TORK_TEST_BOOL_ONE", "1");
    assert!(env_bool("test.bool.one", false));
    std::env::remove_var("TORK_TEST_BOOL_ONE");
}

#[test]
fn test_env_bool_false_string() {
    std::env::set_var("TORK_TEST_BOOL_FALSE_STRING", "false");
    assert!(!env_bool("test.bool.false.string", true));
    std::env::remove_var("TORK_TEST_BOOL_FALSE_STRING");
}

// ── create_broker tests ──────────────────────────────────────────

#[tokio::test]
async fn test_create_broker_inmemory() {
    let broker = create_broker("inmemory").await;
    assert!(broker.is_ok());
    let broker = broker.unwrap();
    
    // Should be functional
    let result = broker.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_broker_type_default() {
    // Default type when unknown should be inmemory
    let broker = create_broker("unknown").await;
    assert!(broker.is_ok());
}

use std::sync::Arc;