//! Task executor module.
//!
//! Provides the core Executor for running tasks to completion with timeout support.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::sleep;
use tracing::debug;

pub use crate::runtime::BoxedFuture;
pub use crate::timeout::Timeout;

#[derive(Debug, Clone)]
pub struct TaskOutput {
    pub task_id: String,
    pub exit_code: i32,
}

pub struct Executor {
    timeout: Duration,
}

impl Executor {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub async fn run(&self, task: Arc<dyn RunnableTask>) -> Result<TaskOutput, Timeout> {
        let task_id = task.id().to_string();
        debug!(task_id = %task_id, timeout_ms = %self.timeout.as_millis(), "Executor::run starting task");

        tokio::select! {
            result = task.clone().run() => {
                debug!(task_id = %task_id, "Executor::run task completed successfully");
                result.map_err(|_| unreachable!())
            }
            () = sleep(self.timeout) => {
                debug!(task_id = %task_id, "Executor::run task timed out");
                task.stop().await;
                Err(Timeout { task_id })
            }
        }
    }
}

#[async_trait]
pub trait RunnableTask: Send + Sync {
    fn id(&self) -> &str;
    async fn run(self: Arc<Self>) -> Result<TaskOutput, std::convert::Infallible>;
    async fn stop(&self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestTask {
        id: String,
        duration: Duration,
        ran: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
    }

    impl TestTask {
        fn new(id: &str, duration: Duration) -> (Arc<Self>, Arc<AtomicBool>, Arc<AtomicBool>) {
            let ran = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));
            let ran_clone = ran.clone();
            let stopped_clone = stopped.clone();
            let task = Self {
                id: id.to_string(),
                duration,
                ran: ran_clone,
                stopped: stopped_clone,
            };
            (Arc::new(task), ran, stopped)
        }
    }

    #[async_trait]
    impl RunnableTask for TestTask {
        fn id(&self) -> &str {
            &self.id
        }

        async fn run(self: Arc<Self>) -> Result<TaskOutput, std::convert::Infallible> {
            sleep(self.duration).await;
            self.ran.store(true, Ordering::SeqCst);
            Ok(TaskOutput {
                task_id: self.id.clone(),
                exit_code: 0,
            })
        }

        async fn stop(&self) {
            self.stopped.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_executor_run_completes_in_time() {
        let (task, ran, _stopped) = TestTask::new("task-1", Duration::from_millis(50));
        let executor = Executor::new(Duration::from_millis(100));

        let result = executor.run(task).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.task_id, "task-1");
        assert_eq!(output.exit_code, 0);
        assert!(ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_executor_run_times_out() {
        let (task, _ran, stopped) = TestTask::new("task-2", Duration::from_millis(200));
        let executor = Executor::new(Duration::from_millis(100));

        let result = executor.run(task).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.task_id, "task-2");
        assert!(stopped.load(Ordering::SeqCst), "Task should be stopped on timeout");
    }

    #[tokio::test]
    async fn test_executor_no_resource_leak_after_timeout() {
        let executor = Executor::new(Duration::from_millis(50));

        struct QuickTask {
            id: String,
            clean: Arc<AtomicBool>,
        }

        impl QuickTask {
            fn new(id: &str, clean: Arc<AtomicBool>) -> Arc<Self> {
                Arc::new(Self {
                    id: id.to_string(),
                    clean,
                })
            }
        }

        #[async_trait]
        impl RunnableTask for QuickTask {
            fn id(&self) -> &str {
                &self.id
            }

            async fn run(self: Arc<Self>) -> Result<TaskOutput, std::convert::Infallible> {
                sleep(Duration::from_millis(200)).await;
                Ok(TaskOutput {
                    task_id: self.id.clone(),
                    exit_code: 0,
                })
            }

            async fn stop(&self) {
                self.clean.store(true, Ordering::SeqCst);
            }
        }

        let cleaned = Arc::new(AtomicBool::new(false));
        let task = QuickTask::new("leak-check", cleaned.clone());
        let result = executor.run(task).await;
        assert!(result.is_err());

        assert!(cleaned.load(Ordering::SeqCst), "Task should be cleaned up after timeout");

        let quick_task = QuickTask::new("reuse-check", Arc::new(AtomicBool::new(false)));
        let reuse_result = executor.run(quick_task).await;
        assert!(reuse_result.is_err());
    }
}