//! Redaction middleware for task data.
//!
//! This middleware redacts sensitive information from tasks when they
//! are read, preventing secrets from being exposed in API responses.

use crate::middleware::task::task_error::TaskMiddlewareError;
use crate::middleware::task::task_handler::{Context, HandlerFunc, MiddlewareFunc};
use crate::middleware::task::task_types::EventType;
use std::sync::Arc;
use tork::task::Task;

/// Redacter trait for sensitive data.
pub trait Redacter: Send + Sync {
    /// Redacts a task's sensitive data.
    fn redact_task(&self, task: &mut Task);
}

/// Creates a redaction middleware.
pub fn redact_middleware<R: Redacter + 'static>(redacter: Arc<R>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| -> HandlerFunc {
        let redacter = redacter.clone();
        Arc::new(move |ctx: Context, et: EventType, task: &mut Task| {
            let redacter = redacter.clone();

            // Check if this is a Read event
            if et == EventType::Read {
                redacter.redact_task(task);
            }

            // Call the next handler
            next(ctx, et, task)
        })
    })
}

/// Default redacter implementation.
///
/// Redacts environment variables and other fields that may contain secrets.
#[derive(Debug, Clone, Default)]
pub struct DefaultRedacter {
    /// Keys that indicate secret data.
    secret_keys: Vec<String>,
}

impl DefaultRedacter {
    /// Creates a new DefaultRedacter with default secret patterns.
    #[must_use]
    pub fn new() -> Self {
        Self {
            secret_keys: vec![
                "SECRET".to_string(),
                "PASSWORD".to_string(),
                "ACCESS_KEY".to_string(),
                "API_KEY".to_string(),
                "TOKEN".to_string(),
            ],
        }
    }

    /// Redacts secrets from a task.
    pub fn redact_task_inner(
        &self,
        task: &mut Task,
        _secrets: &std::collections::HashMap<String, String>,
    ) {
        // Redact env vars
        if let Some(env) = &mut task.env {
            for (key, value) in env.iter_mut() {
                // Check if key matches secret patterns
                let is_secret = self
                    .secret_keys
                    .iter()
                    .any(|pattern| key.to_uppercase().contains(pattern));

                if is_secret {
                    *value = "[REDACTED]".to_string();
                }
            }
        }

        // Recursively redact pre tasks
        if let Some(pre) = &task.pre {
            for pre_task in pre {
                let mut pre_task_mut = pre_task.clone();
                self.redact_task_inner(&mut pre_task_mut, _secrets);
            }
        }

        // Recursively redact post tasks
        if let Some(post) = &task.post {
            for post_task in post {
                let mut post_task_mut = post_task.clone();
                self.redact_task_inner(&mut post_task_mut, _secrets);
            }
        }
    }
}

impl Redacter for DefaultRedacter {
    fn redact_task(&self, task: &mut Task) {
        let secrets = std::collections::HashMap::new();
        self.redact_task_inner(task, &secrets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_redacter_new() {
        let redacter = DefaultRedacter::new();
        assert!(!redacter.secret_keys.is_empty());
    }
}
