//! Redaction middleware for task data.
//!
//! This middleware redacts sensitive information from tasks when they
//! are read, preventing secrets from being exposed in API responses.
//!
//! Full parity with Go `middleware/task/redact.go`.

use crate::middleware::task::task_handler::{Context, HandlerFunc, MiddlewareFunc};
use crate::middleware::task::task_types::EventType;
use std::sync::Arc;
use tork::task::Task;

/// Redacter trait for sensitive data.
///
/// Implementations must redact task secrets in-place.
/// Go parity: `redact.Redacter.RedactTask(t *tork.Task)`
pub trait Redacter: Send + Sync {
    /// Redacts a task's sensitive data in-place.
    fn redact_task(&self, task: &mut Task);
}

/// Creates a redaction middleware.
///
/// Go parity: `func Redact(redacter *redact.Redacter) MiddlewareFunc`
pub fn redact_middleware<R: Redacter + 'static>(redacter: Arc<R>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| -> HandlerFunc {
        let redacter = redacter.clone();
        Arc::new(move |ctx: Context, et: EventType, task: &mut Task| {
            // Redact only on Read events (Go parity)
            if et == EventType::Read {
                redacter.redact_task(task);
            }

            // Call the next handler
            next(ctx, et, task)
        })
    })
}

/// Default redacter implementation that uses key-pattern matching.
///
/// Wraps [`crate::redact::Redacter`] for full redaction coverage:
/// env vars, mount opts, pre/post/parallel subtasks, registry password,
/// subjob secrets and webhook headers.
///
/// This implementation does not use job secrets (no datastore access).
/// For secret-aware redaction, implement [`Redacter`] with a
/// [`crate::redact::JobSecretLookup`].
#[derive(Debug, Clone)]
pub struct DefaultRedacter {
    inner: crate::redact::Redacter,
}

impl DefaultRedacter {
    /// Creates a new DefaultRedacter with default secret patterns.
    ///
    /// Uses the same default matchers as Go: SECRET, PASSWORD, ACCESS_KEY.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: crate::redact::Redacter::default(),
        }
    }
}

impl Default for DefaultRedacter {
    fn default() -> Self {
        Self::new()
    }
}

impl Redacter for DefaultRedacter {
    fn redact_task(&self, task: &mut Task) {
        // Delegate to the full redaction engine with empty secrets.
        // The crate::redact::Redacter returns a new Task (pure function),
        // so we assign it back via *task = ... to mutate in-place.
        let secrets = std::collections::HashMap::new();
        let redacted = self.inner.redact_task(task, &secrets);
        *task = redacted;
    }
}

/// Secret-aware redacter that looks up job secrets for value-based redaction.
///
/// Go parity: `redact.NewRedacter(ds)` — uses the datastore to fetch job secrets
/// so that secret values embedded in env vars, mount opts, etc. are also redacted.
#[allow(dead_code)]
pub struct SecretsRedacter<L> {
    inner: crate::redact::Redacter,
    lookup: L,
}

impl<L> SecretsRedacter<L>
where
    L: crate::redact::JobSecretLookup,
{
    /// Creates a new SecretsRedacter with the given secret lookup.
    #[must_use]
    #[allow(dead_code)]
    pub fn new(lookup: L) -> Self {
        Self {
            inner: crate::redact::Redacter::default(),
            lookup,
        }
    }
}

impl<L> Redacter for SecretsRedacter<L>
where
    L: crate::redact::JobSecretLookup + Send + Sync,
{
    fn redact_task(&self, task: &mut Task) {
        // Use the full redaction engine with job-secret lookup.
        // If the job is not found or has no job_id, returns task unchanged.
        let redacted = self.inner.redact_task_with_lookup(task, &self.lookup);
        *task = redacted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tork::task::TASK_STATE_RUNNING;

    #[test]
    fn test_default_redacter_new() {
        let redacter = DefaultRedacter::new();
        // Verify the inner redacter has matchers
        assert!(redacter.inner.should_redact("SECRET"));
        assert!(redacter.inner.should_redact("PASSWORD"));
        assert!(redacter.inner.should_redact("ACCESS_KEY"));
    }

    #[test]
    fn test_default_redacter_redacts_env_keys() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            env: Some(HashMap::from([
                ("DB_PASSWORD".to_string(), "super_secret".to_string()),
                ("AWS_ACCESS_KEY_ID".to_string(), "tok123".to_string()),
                ("PUBLIC_VAR".to_string(), "hello".to_string()),
            ])),
            ..Default::default()
        };

        redacter.redact_task(&mut task);
        let env = task.env.as_ref().expect("env");
        assert_eq!(env["DB_PASSWORD"], "[REDACTED]");
        assert_eq!(env["AWS_ACCESS_KEY_ID"], "[REDACTED]");
        assert_eq!(env["PUBLIC_VAR"], "hello");
    }

    #[test]
    fn test_default_redacter_redacts_mount_opts() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            mounts: Some(vec![tork::mount::Mount {
                opts: Some(HashMap::from([
                    ("password".to_string(), "abc".to_string()),
                    ("type".to_string(), "nfs".to_string()),
                ])),
                ..Default::default()
            }]),
            ..Default::default()
        };

        redacter.redact_task(&mut task);
        let opts = task.mounts.as_ref().expect("mounts")[0]
            .opts
            .as_ref()
            .expect("opts");
        assert_eq!(opts["password"], "[REDACTED]");
        assert_eq!(opts["type"], "nfs");
    }

    #[test]
    fn test_default_redacter_redacts_registry_password() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            registry: Some(tork::task::Registry {
                username: Some("admin".to_string()),
                password: Some("pw".to_string()),
            }),
            ..Default::default()
        };

        redacter.redact_task(&mut task);
        let reg = task.registry.as_ref().expect("registry");
        assert_eq!(reg.username.as_deref(), Some("admin"));
        assert_eq!(reg.password.as_deref(), Some("[REDACTED]"));
    }

    #[test]
    fn test_default_redacter_redacts_pre_tasks() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            env: Some(HashMap::from([(
                "PARENT_SECRET".to_string(),
                "val".to_string(),
            )])),
            pre: Some(vec![Task {
                env: Some(HashMap::from([(
                    "PRE_PASSWORD".to_string(),
                    "val".to_string(),
                )])),
                ..Default::default()
            }]),
            ..Default::default()
        };

        redacter.redact_task(&mut task);

        // Parent redacted
        assert_eq!(
            task.env.as_ref().expect("env")["PARENT_SECRET"],
            "[REDACTED]"
        );

        // Pre task redacted (this was the clone+discard bug)
        let pre_env = task.pre.as_ref().expect("pre")[0]
            .env
            .as_ref()
            .expect("pre env");
        assert_eq!(pre_env["PRE_PASSWORD"], "[REDACTED]");
    }

    #[test]
    fn test_default_redacter_redacts_post_tasks() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            post: Some(vec![Task {
                env: Some(HashMap::from([(
                    "POST_ACCESS_KEY".to_string(),
                    "key".to_string(),
                )])),
                ..Default::default()
            }]),
            ..Default::default()
        };

        redacter.redact_task(&mut task);

        // Post task redacted (this was the clone+discard bug)
        let post_env = task.post.as_ref().expect("post")[0]
            .env
            .as_ref()
            .expect("post env");
        assert_eq!(post_env["POST_ACCESS_KEY"], "[REDACTED]");
    }

    #[test]
    fn test_default_redacter_redacts_parallel_tasks() {
        let redacter = DefaultRedacter::new();
        let mut task = Task {
            parallel: Some(tork::task::ParallelTask {
                tasks: Some(vec![Task {
                    env: Some(HashMap::from([(
                        "PAR_SECRET".to_string(),
                        "val".to_string(),
                    )])),
                    ..Default::default()
                }]),
                completions: 2,
            }),
            ..Default::default()
        };

        redacter.redact_task(&mut task);
        let par_env = task
            .parallel
            .as_ref()
            .expect("parallel")
            .tasks
            .as_ref()
            .expect("parallel tasks")[0]
            .env
            .as_ref()
            .expect("parallel env");
        assert_eq!(par_env["PAR_SECRET"], "[REDACTED]");
    }

    #[test]
    fn test_secrets_redacter_redacts_secret_values() {
        struct MockLookup;

        impl crate::redact::JobSecretLookup for MockLookup {
            fn get_job_secrets(&self, _job_id: &str) -> Option<HashMap<String, String>> {
                Some(HashMap::from([(
                    "db_token".to_string(),
                    "s3cret_token".to_string(),
                )]))
            }
        }

        let redacter = SecretsRedacter::new(MockLookup);
        let mut task = Task {
            job_id: Some("job-1".to_string()),
            env: Some(HashMap::from([(
                "DATABASE_URL".to_string(),
                "postgres://user:s3cret_token@host/db".to_string(),
            )])),
            ..Default::default()
        };

        redacter.redact_task(&mut task);
        let env = task.env.as_ref().expect("env");
        // Key "DATABASE_URL" doesn't match PASSWORD/SECRET/ACCESS_KEY matchers,
        // but the secret value "s3cret_token" in the URL gets replaced.
        assert_eq!(env["DATABASE_URL"], "postgres://user:[REDACTED]@host/db");
    }

    #[test]
    fn test_middleware_redact_on_read() {
        let redacter = Arc::new(DefaultRedacter::new());
        let mw = redact_middleware(redacter);
        let handler = mw(crate::middleware::task::noop_handler());

        let mut task = Task {
            env: Some(HashMap::from([(
                "MY_SECRET".to_string(),
                "hidden".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(std::sync::RwLock::new(()));
        handler(ctx, EventType::Read, &mut task).expect("handler ok");
        assert_eq!(task.env.as_ref().expect("env")["MY_SECRET"], "[REDACTED]");
    }

    #[test]
    fn test_middleware_no_redact_on_state_change() {
        let redacter = Arc::new(DefaultRedacter::new());
        let mw = redact_middleware(redacter);
        let handler = mw(crate::middleware::task::noop_handler());

        let mut task = Task {
            state: TASK_STATE_RUNNING,
            env: Some(HashMap::from([(
                "MY_SECRET".to_string(),
                "hidden".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(std::sync::RwLock::new(()));
        handler(ctx, EventType::StateChange, &mut task).expect("handler ok");
        // Should NOT be redacted on StateChange
        assert_eq!(task.env.as_ref().expect("env")["MY_SECRET"], "hidden");
    }

    #[test]
    fn test_default_impl() {
        let redacter = DefaultRedacter::default();
        assert!(redacter.inner.should_redact("SECRET"));
    }
}
