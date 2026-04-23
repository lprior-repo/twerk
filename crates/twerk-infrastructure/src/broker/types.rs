//! Broker type aliases for async handlers and futures.

use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;
pub(crate) type BoxedHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

pub type TaskHandler = Arc<dyn Fn(Arc<Task>) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskProgressHandler = Arc<dyn Fn(Task) -> BoxedHandlerFuture + Send + Sync>;
pub type HeartbeatHandler = Arc<dyn Fn(Node) -> BoxedHandlerFuture + Send + Sync>;
pub type JobHandler = Arc<dyn Fn(Job) -> BoxedHandlerFuture + Send + Sync>;
pub type EventHandler = Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskLogPartHandler = Arc<dyn Fn(TaskLogPart) -> BoxedHandlerFuture + Send + Sync>;
