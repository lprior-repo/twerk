//! Twerk Engine - Registration methods (middleware, endpoints, providers, job submission)

use super::state::{Mode, State};
use super::types::{
    EndpointHandler, JobListener, JobMiddlewareFunc, LogMiddlewareFunc, NodeMiddlewareFunc,
    SubmitTaskError, TaskHandle, TaskMiddlewareFunc, WebMiddlewareFunc,
};
use super::TOPIC_JOB;
use anyhow::Result;
use std::sync::Arc;
use tracing::error;
use twerk_core::id::TaskId;
use twerk_core::task::Task;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;
use twerk_infrastructure::runtime::{Mounter, Runtime};
use twerk_common::constants::DEFAULT_TASK_NAME;

// ── Typed engine registration errors ───────────────────────────────

#[derive(Debug, thiserror::Error)]
pub(crate) enum EngineRegistrationError {
    #[error("engine is not running")]
    NotRunning,
    #[error("engine not in coordinator/standalone mode")]
    InvalidMode,
    #[error("coordinator not available")]
    CoordinatorUnavailable,
}

impl super::Engine {
    /// Register web middleware
    pub fn register_web_middleware(&mut self, mw: WebMiddlewareFunc) {
        if self.state != State::Idle {
            return;
        }
        self.middleware.web.push(mw);
    }

    /// Register task middleware
    pub fn register_task_middleware(&mut self, mw: TaskMiddlewareFunc) {
        if self.state != State::Idle {
            return;
        }
        self.middleware.task.push(mw);
    }

    /// Register job middleware
    pub fn register_job_middleware(&mut self, mw: JobMiddlewareFunc) {
        if self.state != State::Idle {
            return;
        }
        self.middleware.job.push(mw);
    }

    /// Register node middleware
    pub fn register_node_middleware(&mut self, mw: NodeMiddlewareFunc) {
        if self.state != State::Idle {
            return;
        }
        self.middleware.node.push(mw);
    }

    /// Register log middleware
    pub fn register_log_middleware(&mut self, mw: LogMiddlewareFunc) {
        if self.state != State::Idle {
            return;
        }
        self.middleware.log.push(mw);
    }

    /// Register an API endpoint
    pub fn register_endpoint(&mut self, method: &str, path: &str, handler: EndpointHandler) {
        if self.state != State::Idle {
            return;
        }
        let key = format!("{} {}", method, path);
        self.endpoints.insert(key, handler);
    }

    /// Register a runtime provider
    pub fn register_runtime(&mut self, rt: Box<dyn Runtime + Send + Sync>) {
        if self.state != State::Idle {
            return;
        }
        if self.runtime.is_some() {
            return;
        }
        self.runtime = Some(rt);
    }

    /// Register a datastore provider
    pub fn register_datastore_provider(
        &mut self,
        name: &str,
        provider: Box<dyn Datastore + Send + Sync>,
    ) {
        if self.state != State::Idle {
            return;
        }
        let name = name.to_string();
        if self.ds_providers.contains_key(&name) {
            return;
        }
        self.ds_providers.insert(name, provider);
    }

    /// Register a broker provider
    pub fn register_broker_provider(
        &mut self,
        name: &str,
        provider: Box<dyn Broker + Send + Sync>,
    ) {
        if self.state != State::Idle {
            return;
        }
        let name = name.to_string();
        if self.broker_providers.contains_key(&name) {
            return;
        }
        self.broker_providers.insert(name, provider);
    }

    /// Register a mounter for a specific runtime.
    ///
    /// Matches Go's `RegisterMounter(rt, name, mounter)`:
    /// - Creates a new `MultiMounter` for the runtime if one doesn't exist yet.
    /// - Registers the named mounter into that runtime's `MultiMounter`.
    pub fn register_mounter(&mut self, rt: &str, name: &str, mounter: Box<dyn Mounter>) {
        if self.state != State::Idle {
            return;
        }
        let rt_key = rt.to_string();
        let entry = self.mounters.entry(rt_key).or_default();
        // Silently ignore duplicate mounter registrations, matching Go's
        // behavior of creating a new MultiMounter per runtime key. The
        // underlying `MultiMounter::register_mounter` returns a
        // `MountError::DuplicateMounter` which we log if it's not expected.
        if let Err(e) = entry.register_mounter(name, mounter) {
            error!("failed to register mounter {name} for runtime {rt}: {e}");
        }
    }

    /// Submit a job to the engine
    pub async fn submit_job(
        &self,
        job: twerk_core::job::Job,
        listeners: Vec<JobListener>,
    ) -> Result<twerk_core::job::Job> {
        if self.state != State::Running {
            return Err(EngineRegistrationError::NotRunning.into());
        }
        if self.mode != Mode::Standalone && self.mode != Mode::Coordinator {
            return Err(EngineRegistrationError::InvalidMode.into());
        }

        // Get the job ID for listener matching
        let job_id = job.id.clone();

        // Subscribe to job events if there are listeners
        if !listeners.is_empty() {
            let broker = self.broker.clone();
            let listeners = Arc::new(listeners);
            let job_id_for_listener = job_id.clone();

            broker
                .subscribe_for_events(
                    TOPIC_JOB.to_string(),
                    Arc::new(move |event: serde_json::Value| {
                        let listeners = listeners.clone();
                        let job_id = job_id_for_listener.clone();
                        Box::pin(async move {
                            // Try to parse the event as a job
                            if let Ok(ev_job) =
                                serde_json::from_value::<twerk_core::job::Job>(event)
                            {
                                if ev_job.id.as_ref() == job_id.as_ref() {
                                    for listener in listeners.iter() {
                                        listener(ev_job.clone());
                                    }
                                }
                            }
                            Ok(())
                        })
                    }),
                )
                .await?;
        }

        // Submit to coordinator
        let coordinator = self.coordinator.read().await;
        if let Some(ref coord) = *coordinator {
            let result = coord.submit_job(job).await?;
            Ok(result.deep_clone())
        } else {
            Err(EngineRegistrationError::CoordinatorUnavailable.into())
        }
    }

    /// Register a job listener
    pub fn add_job_listener(&self, listener: JobListener) {
        let mut listeners = self.job_listeners.blocking_write();
        listeners.push(listener);
    }

    /// Submit a task to the engine and returns a handle with the task ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not running
    /// - A task with the same ID has already been submitted
    pub async fn submit_task(&mut self, mut task: Task) -> Result<TaskHandle, SubmitTaskError> {
        if self.state != State::Running {
            return Err(SubmitTaskError::NotRunning);
        }

        let task_id = match task.id {
            Some(id) => {
                if self.submitted_tasks.contains(&id) {
                    return Err(SubmitTaskError::DuplicateTaskId(id));
                }
                id
            }
            None => TaskId::new(DEFAULT_TASK_NAME)
                .map_err(|e| SubmitTaskError::InvalidTaskName(e.to_string()))?,
        };

        task.id = Some(task_id.clone());
        self.submitted_tasks.insert(task_id.clone());

        let queue_name = task.queue.clone().unwrap_or_else(|| DEFAULT_TASK_NAME.to_string());
        self.broker
            .publish_task(queue_name, &task)
            .await
            .map_err(|_| SubmitTaskError::NotRunning)?;

        Ok(TaskHandle { task_id })
    }
}
