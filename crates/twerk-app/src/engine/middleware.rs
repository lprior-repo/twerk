//! Twerk Engine - Middleware composition and registration

use super::types::{
    JobHandlerFunc, JobMiddlewareFunc, LogHandlerFunc, LogMiddlewareFunc, NodeHandlerFunc,
    NodeMiddlewareFunc, TaskHandlerFunc, TaskMiddlewareFunc, WebMiddlewareFunc,
};
use std::sync::Arc;

/// Middleware composition helper
pub struct MiddlewareComposer {
    web: Vec<WebMiddlewareFunc>,
    task: Vec<TaskMiddlewareFunc>,
    job: Vec<JobMiddlewareFunc>,
    node: Vec<NodeMiddlewareFunc>,
    log: Vec<LogMiddlewareFunc>,
}

impl MiddlewareComposer {
    pub fn new() -> Self {
        Self {
            web: Vec::new(),
            task: Vec::new(),
            job: Vec::new(),
            node: Vec::new(),
            log: Vec::new(),
        }
    }

    /// Add web middleware
    pub fn with_web(mut self, mw: WebMiddlewareFunc) -> Self {
        self.web.push(mw);
        self
    }

    /// Add task middleware
    pub fn with_task(mut self, mw: TaskMiddlewareFunc) -> Self {
        self.task.push(mw);
        self
    }

    /// Add job middleware
    pub fn with_job(mut self, mw: JobMiddlewareFunc) -> Self {
        self.job.push(mw);
        self
    }

    /// Add node middleware
    pub fn with_node(mut self, mw: NodeMiddlewareFunc) -> Self {
        self.node.push(mw);
        self
    }

    /// Add log middleware
    pub fn with_log(mut self, mw: LogMiddlewareFunc) -> Self {
        self.log.push(mw);
        self
    }

    /// Compose task handler with registered middleware
    pub fn compose_task_handler(&self, handler: TaskHandlerFunc) -> TaskHandlerFunc {
        self.task.iter().rev().fold(handler, |h, mw| mw(h))
    }

    /// Compose job handler with registered middleware
    pub fn compose_job_handler(&self, handler: JobHandlerFunc) -> JobHandlerFunc {
        self.job.iter().rev().fold(handler, |h, mw| mw(h))
    }

    /// Compose node handler with registered middleware
    pub fn compose_node_handler(&self, handler: NodeHandlerFunc) -> NodeHandlerFunc {
        self.node.iter().rev().fold(handler, |h, mw| mw(h))
    }

    /// Compose log handler with registered middleware
    pub fn compose_log_handler(&self, handler: LogHandlerFunc) -> LogHandlerFunc {
        self.log.iter().rev().fold(handler, |h, mw| mw(h))
    }
}

impl Default for MiddlewareComposer {
    fn default() -> Self {
        Self::new()
    }
}
