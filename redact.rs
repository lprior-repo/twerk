//! Redaction utilities.
//!
//! Provides functionality for redacting sensitive information from strings and data structures.
//! All core functions are pure calculations — they take borrowed data and return new owned values.
//!
//! ## Architecture
//!
//! - **Data**: [`Matcher`] trait, [`JobSecretLookup`] trait, [`REDACTED_STR`] constant
//! - **Calculations**: [`Redacter`] methods — all pure, zero I/O, zero mutation
//! - **Actions**: Callers (middleware) own the I/O of looking up jobs and persisting results

use std::collections::HashMap;
use std::sync::Arc;

use tork::job::{Job, JobContext};
use tork::mount::Mount;
use tork::task::{ParallelTask, Registry, SubJobTask, Task, TaskLogPart, Webhook};

/// The redaction replacement string.
pub const REDACTED_STR: &str = "[REDACTED]";

/// Trait for matching keys that should be redacted.
pub trait Matcher: Send + Sync {
    /// Check if a key matches this matcher.
    fn matches(&self, key: &str) -> bool;
}

/// Matcher that checks if a key contains a substring (case-insensitive).
#[derive(Clone)]
pub struct ContainsMatcher {
    substring: String,
}

impl ContainsMatcher {
    /// Create a new contains matcher wrapped in `Arc`.
    #[must_use]
    pub fn new(substr: &str) -> Arc<Self> {
        Arc::new(Self {
            substring: substr.to_uppercase(),
        })
    }
}

impl Matcher for ContainsMatcher {
    fn matches(&self, key: &str) -> bool {
        key.to_uppercase().contains(&self.substring)
    }
}

/// Create a matcher that checks if a key contains the given substring (case-insensitive).
#[must_use]
pub fn contains_matcher(substr: &str) -> Arc<dyn Matcher> {
    ContainsMatcher::new(substr)
}

/// Matcher that uses wildcard pattern matching.
#[derive(Clone)]
pub struct WildcardMatcher {
    pattern: String,
}

impl WildcardMatcher {
    /// Create a new wildcard matcher wrapped in `Arc`.
    #[must_use]
    pub fn new(pattern: &str) -> Arc<Self> {
        Arc::new(Self {
            pattern: pattern.to_string(),
        })
    }
}

impl Matcher for WildcardMatcher {
    fn matches(&self, key: &str) -> bool {
        crate::wildcard::match_pattern(&self.pattern, key)
    }
}

/// Create a matcher that uses wildcard pattern matching.
#[must_use]
pub fn wildcard_matcher(pattern: &str) -> Arc<dyn Matcher> {
    WildcardMatcher::new(pattern)
}

/// Returns the default set of matchers for key-based redaction.
///
/// Matches keys containing (case-insensitive): `SECRET`, `PASSWORD`, `ACCESS_KEY`
#[must_use]
pub fn default_matchers() -> Vec<Arc<dyn Matcher>> {
    vec![
        contains_matcher("SECRET"),
        contains_matcher("PASSWORD"),
        contains_matcher("ACCESS_KEY"),
    ]
}

/// Trait for looking up job secrets by job ID.
///
/// This is a capability-based interface — callers provide their own
/// implementation backed by whatever data source they use (database, cache, etc.).
pub trait JobSecretLookup: Send + Sync {
    /// Look up the secrets for a job.
    /// Returns `None` if the job is not found.
    fn get_job_secrets(&self, job_id: &str) -> Option<HashMap<String, String>>;
}

/// A redacter that can redact sensitive information from tasks and jobs.
///
/// Uses a set of [`Matcher`]s to identify sensitive keys and replaces
/// secret values with [`REDACTED_STR`].
#[derive(Clone)]
pub struct Redacter {
    matchers: Vec<Arc<dyn Matcher>>,
}

impl std::fmt::Debug for Redacter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redacter")
            .field("matcher_count", &self.matchers.len())
            .finish()
    }
}

impl Default for Redacter {
    fn default() -> Self {
        Self::new(default_matchers())
    }
}

impl Redacter {
    /// Create a new redacter with the given matchers.
    #[must_use]
    pub fn new(matchers: Vec<Arc<dyn Matcher>>) -> Self {
        Self { matchers }
    }

    /// Check if a key should be redacted based on matchers.
    #[must_use]
    pub fn should_redact(&self, key: &str) -> bool {
        self.matchers.iter().any(|m| m.matches(key))
    }

    /// Redact a single value: if the key matches any matcher, return `[REDACTED]`.
    /// Otherwise, replace all non-empty secret substrings found in the value.
    #[must_use]
    pub fn redact_value(
        &self,
        value: &str,
        key: &str,
        secrets: &HashMap<String, String>,
    ) -> String {
        if self.should_redact(key) {
            return REDACTED_STR.to_string();
        }
        secrets
            .values()
            .filter(|s| !s.is_empty())
            .fold(value.to_string(), |acc, secret| {
                acc.replace(secret.as_str(), REDACTED_STR)
            })
    }

    /// Redact a map of variables by key matching and secret value replacement.
    ///
    /// For each entry:
    /// - If the key matches any matcher → value becomes `[REDACTED]`
    /// - Otherwise, any secret values found within the value are replaced
    #[must_use]
    pub fn redact_vars(
        &self,
        vars: &HashMap<String, String>,
        secrets: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        vars.iter()
            .map(|(k, v)| (k.clone(), self.redact_value(v, k, secrets)))
            .collect()
    }

    /// Redact a task recursively using the provided secrets.
    ///
    /// Redacts: env vars, mount opts, pre/post/parallel subtasks,
    /// registry password, subjob secrets and webhook headers.
    ///
    /// Returns a new `Task` — the original is not modified.
    #[must_use]
    pub fn redact_task(&self, task: &Task, secrets: &HashMap<String, String>) -> Task {
        Task {
            env: task.env.as_ref().map(|e| self.redact_vars(e, secrets)),
            mounts: task.mounts.as_ref().map(|mounts| {
                mounts
                    .iter()
                    .map(|mount| Mount {
                        opts: mount.opts.as_ref().map(|o| self.redact_vars(o, secrets)),
                        ..mount.clone()
                    })
                    .collect()
            }),
            pre: task
                .pre
                .as_ref()
                .map(|pre| pre.iter().map(|p| self.redact_task(p, secrets)).collect()),
            post: task
                .post
                .as_ref()
                .map(|post| post.iter().map(|p| self.redact_task(p, secrets)).collect()),
            parallel: task.parallel.as_ref().map(|parallel| ParallelTask {
                tasks: parallel
                    .tasks
                    .as_ref()
                    .map(|tasks| tasks.iter().map(|t| self.redact_task(t, secrets)).collect()),
                ..parallel.clone()
            }),
            registry: task.registry.as_ref().map(|_| Registry {
                username: task.registry.as_ref().and_then(|r| r.username.clone()),
                password: Some(REDACTED_STR.to_string()),
            }),
            subjob: task.subjob.as_ref().map(|subjob| SubJobTask {
                secrets: subjob.secrets.as_ref().map(|s| {
                    s.iter()
                        .map(|(k, _)| (k.clone(), REDACTED_STR.to_string()))
                        .collect()
                }),
                webhooks: subjob.webhooks.as_ref().map(|whs| {
                    whs.iter()
                        .map(|w| Webhook {
                            headers: w.headers.as_ref().map(|h| self.redact_vars(h, secrets)),
                            ..w.clone()
                        })
                        .collect()
                }),
                ..subjob.clone()
            }),
            ..task.clone()
        }
    }

    /// Redact a task by looking up its job's secrets from a data source.
    ///
    /// If the task has no `job_id`, or the job is not found by the lookup,
    /// returns the original task unchanged.
    #[must_use]
    pub fn redact_task_with_lookup<L: JobSecretLookup>(&self, task: &Task, lookup: &L) -> Task {
        let job_id = match &task.job_id {
            Some(id) => id.as_str(),
            None => return task.clone(),
        };

        match lookup.get_job_secrets(job_id) {
            Some(secrets) => self.redact_task(task, &secrets),
            None => task.clone(),
        }
    }

    /// Redact a job's sensitive data using its own secrets.
    ///
    /// Redacts: inputs, webhook headers, context (inputs/secrets/tasks),
    /// all tasks and execution tasks, then blanks out the secrets themselves.
    ///
    /// Returns a new `Job` — the original is not modified.
    #[must_use]
    pub fn redact_job(&self, job: &Job) -> Job {
        let empty_secrets = HashMap::<String, String>::new();
        let secrets: &HashMap<String, String> = match &job.secrets {
            Some(s) => s,
            None => &empty_secrets,
        };

        Job {
            inputs: job.inputs.as_ref().map(|i| self.redact_vars(i, secrets)),
            webhooks: job.webhooks.as_ref().map(|whs| {
                whs.iter()
                    .map(|w| Webhook {
                        headers: w.headers.as_ref().map(|h| self.redact_vars(h, secrets)),
                        ..w.clone()
                    })
                    .collect()
            }),
            context: JobContext {
                inputs: job
                    .context
                    .inputs
                    .as_ref()
                    .map(|i| self.redact_vars(i, secrets)),
                secrets: job
                    .context
                    .secrets
                    .as_ref()
                    .map(|s| self.redact_vars(s, secrets)),
                tasks: job
                    .context
                    .tasks
                    .as_ref()
                    .map(|t| self.redact_vars(t, secrets)),
                ..job.context.clone()
            },
            tasks: job
                .tasks
                .iter()
                .map(|t| self.redact_task(t, secrets))
                .collect(),
            execution: job
                .execution
                .iter()
                .map(|t| self.redact_task(t, secrets))
                .collect(),
            secrets: job.secrets.as_ref().map(|s| {
                s.iter()
                    .map(|(k, _)| (k.clone(), REDACTED_STR.to_string()))
                    .collect()
            }),
            ..job.clone()
        }
    }

    /// Redact a task log part by replacing secret values in its contents.
    /// Returns a new `TaskLogPart` with redacted contents.
    #[must_use]
    pub fn redact_task_log_part(
        &self,
        part: &TaskLogPart,
        secrets: &HashMap<String, String>,
    ) -> TaskLogPart {
        let contents = part.contents.as_ref().map(|c| {
            secrets
                .values()
                .filter(|s| !s.is_empty())
                .fold(c.clone(), |acc, secret| {
                    acc.replace(secret.as_str(), REDACTED_STR)
                })
        });
        TaskLogPart {
            id: part.id.clone(),
            number: part.number,
            task_id: part.task_id.clone(),
            contents,
            created_at: part.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Matcher tests ──────────────────────────────────────────────────

    #[test]
    fn test_default_matchers() {
        let matchers = default_matchers();
        assert_eq!(matchers.len(), 3);
        assert!(matchers[0].matches("MY_SECRET_KEY"));
        assert!(matchers[1].matches("DB_PASSWORD"));
        assert!(matchers[2].matches("AWS_ACCESS_KEY_ID"));
    }

    #[test]
    fn test_contains_matcher() {
        let matcher = contains_matcher("SECRET");
        assert!(matcher.matches("MY_SECRET_KEY"));
        assert!(matcher.matches("secret"));
        assert!(!matcher.matches("PUBLIC"));
    }

    #[test]
    fn test_wildcard_matcher() {
        let matcher = wildcard_matcher("env.*");
        assert!(matcher.matches("env.HOST"));
        assert!(matcher.matches("env.PORT"));
        assert!(!matcher.matches("env"));
        assert!(!matcher.matches("HOST"));
    }

    #[test]
    fn test_should_redact() {
        let redacter = Redacter::default();
        assert!(redacter.should_redact("API_SECRET"));
        assert!(redacter.should_redact("db_password"));
        assert!(redacter.should_redact("AWS_ACCESS_KEY"));
        assert!(!redacter.should_redact("HOST"));
        assert!(!redacter.should_redact("PORT"));
    }

    // ── redact_value tests ─────────────────────────────────────────────

    #[test]
    fn test_redact_value_key_match_overrides_secret_replacement() {
        let redacter = Redacter::default();
        let secrets = HashMap::from([("pw".to_string(), "s3cret".to_string())]);
        // Key matches PASSWORD matcher → always [REDACTED], even though secret isn't in value
        assert_eq!(
            redacter.redact_value("localhost:5432", "DB_PASSWORD", &secrets),
            REDACTED_STR
        );
    }

    #[test]
    fn test_redact_value_secret_in_value() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::from([("token".to_string(), "s3cret".to_string())]);
        assert_eq!(
            redacter.redact_value("host=s3cret", "HOST", &secrets),
            "host=[REDACTED]"
        );
    }

    #[test]
    fn test_redact_value_multiple_secrets() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::from([
            ("a".to_string(), "alpha".to_string()),
            ("b".to_string(), "beta".to_string()),
        ]);
        assert_eq!(
            redacter.redact_value("alpha and beta", "CONTENT", &secrets),
            "[REDACTED] and [REDACTED]"
        );
    }

    #[test]
    fn test_redact_value_no_match() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::from([("token".to_string(), "hidden".to_string())]);
        assert_eq!(
            redacter.redact_value("localhost:5432", "HOST", &secrets),
            "localhost:5432"
        );
    }

    #[test]
    fn test_redact_value_empty_secret_skipped() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::from([("empty".to_string(), String::new())]);
        assert_eq!(
            redacter.redact_value("unchanged", "KEY", &secrets),
            "unchanged"
        );
    }

    // ── redact_vars tests ──────────────────────────────────────────────

    #[test]
    fn test_redact_vars_key_and_secret() {
        let redacter = Redacter::default();
        let vars = HashMap::from([
            ("api_key".to_string(), "secret123".to_string()),
            ("username".to_string(), "admin".to_string()),
            ("password".to_string(), "supersecret".to_string()),
        ]);
        let secrets = HashMap::from([("my_secret".to_string(), "secret123".to_string())]);

        let redacted = redacter.redact_vars(&vars, &secrets);

        assert_eq!(redacted["api_key"], REDACTED_STR); // key contains SECRET
        assert_eq!(redacted["username"], "admin"); // no key match, no secret in value
        assert_eq!(redacted["password"], REDACTED_STR); // key contains PASSWORD
    }

    #[test]
    fn test_redact_vars_secret_in_value_no_key_match() {
        let redacter = Redacter::new(vec![]);
        let vars = HashMap::from([("url".to_string(), "postgres://user:pw@host".to_string())]);
        let secrets = HashMap::from([("db_pw".to_string(), "pw".to_string())]);

        let redacted = redacter.redact_vars(&vars, &secrets);
        assert_eq!(redacted["url"], "postgres://user:[REDACTED]@host");
    }

    #[test]
    fn test_redact_vars_empty() {
        let redacter = Redacter::default();
        let vars = HashMap::new();
        let secrets = HashMap::from([("k".to_string(), "v".to_string())]);

        let redacted = redacter.redact_vars(&vars, &secrets);
        assert!(redacted.is_empty());
    }

    // ── redact_task tests ──────────────────────────────────────────────

    #[test]
    fn test_redact_task_env() {
        let redacter = Redacter::default();
        let secrets = HashMap::from([("tk".to_string(), "token_value".to_string())]);
        let task = Task {
            env: Some(HashMap::from([
                ("DB_PASSWORD".to_string(), "pw123".to_string()),
                ("URL".to_string(), "http://token_value@api.com".to_string()),
            ])),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);

        let env = result.env.as_ref().expect("env should be present");
        assert_eq!(env["DB_PASSWORD"], REDACTED_STR);
        assert_eq!(env["URL"], "http://[REDACTED]@api.com");
    }

    #[test]
    fn test_redact_task_env_none() {
        let redacter = Redacter::default();
        let secrets = HashMap::new();
        let task = Task {
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        assert!(result.env.is_none());
    }

    #[test]
    fn test_redact_task_mounts() {
        let redacter = Redacter::default();
        let secrets = HashMap::new();
        let task = Task {
            mounts: Some(vec![Mount {
                opts: Some(HashMap::from([
                    ("password".to_string(), "abc".to_string()),
                    ("type".to_string(), "nfs".to_string()),
                ])),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        let mounts = result.mounts.as_ref().expect("mounts should be present");
        let opts = mounts[0].opts.as_ref().expect("opts should be present");
        assert_eq!(opts["password"], REDACTED_STR);
        assert_eq!(opts["type"], "nfs");
    }

    #[test]
    fn test_redact_task_pre_post_recursive() {
        let redacter = Redacter::default();
        let secrets = HashMap::new();
        let task = Task {
            pre: Some(vec![Task {
                env: Some(HashMap::from([(
                    "PRE_SECRET".to_string(),
                    "val".to_string(),
                )])),
                ..Default::default()
            }]),
            post: Some(vec![Task {
                env: Some(HashMap::from([(
                    "POST_PASSWORD".to_string(),
                    "val".to_string(),
                )])),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);

        let pre_env = result.pre.as_ref().expect("pre")[0]
            .env
            .as_ref()
            .expect("pre env");
        assert_eq!(pre_env["PRE_SECRET"], REDACTED_STR);

        let post_env = result.post.as_ref().expect("post")[0]
            .env
            .as_ref()
            .expect("post env");
        assert_eq!(post_env["POST_PASSWORD"], REDACTED_STR);
    }

    #[test]
    fn test_redact_task_parallel() {
        let redacter = Redacter::default();
        let secrets = HashMap::new();
        let task = Task {
            parallel: Some(ParallelTask {
                tasks: Some(vec![Task {
                    env: Some(HashMap::from([(
                        "PARALLEL_ACCESS_KEY".to_string(),
                        "key".to_string(),
                    )])),
                    ..Default::default()
                }]),
                completions: 3,
            }),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        let par_env = result
            .parallel
            .as_ref()
            .expect("parallel")
            .tasks
            .as_ref()
            .expect("parallel tasks")[0]
            .env
            .as_ref()
            .expect("parallel task env");
        assert_eq!(par_env["PARALLEL_ACCESS_KEY"], REDACTED_STR);
    }

    #[test]
    fn test_redact_task_registry() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::new();
        let task = Task {
            registry: Some(Registry {
                username: Some("admin".to_string()),
                password: Some("super_secret_pw".to_string()),
            }),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        let reg = result.registry.as_ref().expect("registry");
        assert_eq!(reg.username.as_deref(), Some("admin"));
        assert_eq!(reg.password.as_deref(), Some("[REDACTED]"));
    }

    #[test]
    fn test_redact_task_registry_none() {
        let redacter = Redacter::default();
        let secrets = HashMap::new();
        let task = Task {
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        assert!(result.registry.is_none());
    }

    #[test]
    fn test_redact_task_subjob() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::from([("api_key".to_string(), "ak123".to_string())]);
        let task = Task {
            subjob: Some(SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: Some(HashMap::from([
                    ("db_pass".to_string(), "xyz".to_string()),
                    ("api_key".to_string(), "ak123".to_string()),
                ])),
                auto_delete: None,
                output: None,
                detached: false,
                webhooks: Some(vec![Webhook {
                    url: Some("http://example.com".to_string()),
                    headers: Some(HashMap::from([
                        ("Authorization".to_string(), "Bearer ak123".to_string()),
                        ("Content-Type".to_string(), "application/json".to_string()),
                    ])),
                    event: None,
                    r#if: None,
                }]),
            }),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);
        let sj = result.subjob.as_ref().expect("subjob");
        assert_eq!(
            sj.secrets.as_ref().expect("secrets")["db_pass"],
            REDACTED_STR
        );
        assert_eq!(
            sj.secrets.as_ref().expect("secrets")["api_key"],
            REDACTED_STR
        );

        let wh_headers = sj.webhooks.as_ref().expect("webhooks")[0]
            .headers
            .as_ref()
            .expect("webhook headers");
        assert_eq!(wh_headers["Authorization"], "Bearer [REDACTED]");
        assert_eq!(wh_headers["Content-Type"], "application/json");
    }

    #[test]
    fn test_redact_task_preserves_unredacted_fields() {
        let redacter = Redacter::new(vec![]);
        let secrets = HashMap::new();
        let task = Task {
            id: Some("task-1".to_string()),
            name: Some("build".to_string()),
            image: Some("alpine:3.18".to_string()),
            cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
            ..Default::default()
        };

        let result = redacter.redact_task(&task, &secrets);

        assert_eq!(result.id, Some("task-1".to_string()));
        assert_eq!(result.name, Some("build".to_string()));
        assert_eq!(result.image, Some("alpine:3.18".to_string()));
        assert_eq!(
            result.cmd,
            Some(vec!["echo".to_string(), "hello".to_string()])
        );
    }

    // ── redact_task_with_lookup tests ──────────────────────────────────

    struct MockLookup {
        secrets: HashMap<String, String>,
    }

    impl JobSecretLookup for MockLookup {
        fn get_job_secrets(&self, _job_id: &str) -> Option<HashMap<String, String>> {
            Some(self.secrets.clone())
        }
    }

    struct EmptyLookup;

    impl JobSecretLookup for EmptyLookup {
        fn get_job_secrets(&self, _job_id: &str) -> Option<HashMap<String, String>> {
            None
        }
    }

    #[test]
    fn test_redact_task_with_lookup_found() {
        let redacter = Redacter::default();
        let lookup = MockLookup {
            secrets: HashMap::from([("pw".to_string(), "secret_pw".to_string())]),
        };
        let task = Task {
            job_id: Some("job-1".to_string()),
            env: Some(HashMap::from([(
                "HOST".to_string(),
                "secret_pw@host".to_string(),
            )])),
            ..Default::default()
        };

        let result = redacter.redact_task_with_lookup(&task, &lookup);
        let env = result.env.as_ref().expect("env");
        assert_eq!(env["HOST"], "[REDACTED]@host");
    }

    #[test]
    fn test_redact_task_with_lookup_no_job_id() {
        let redacter = Redacter::default();
        let lookup = MockLookup {
            secrets: HashMap::from([("pw".to_string(), "secret".to_string())]),
        };
        let task = Task {
            job_id: None,
            env: Some(HashMap::from([(
                "HOST".to_string(),
                "secret@host".to_string(),
            )])),
            ..Default::default()
        };

        let result = redacter.redact_task_with_lookup(&task, &lookup);
        // No job_id → returns original unchanged
        let env = result.env.as_ref().expect("env");
        assert_eq!(env["HOST"], "secret@host");
    }

    #[test]
    fn test_redact_task_with_lookup_not_found() {
        let redacter = Redacter::default();
        let lookup = EmptyLookup;
        let task = Task {
            job_id: Some("missing-job".to_string()),
            env: Some(HashMap::from([("HOST".to_string(), "val".to_string())])),
            ..Default::default()
        };

        let result = redacter.redact_task_with_lookup(&task, &lookup);
        // Job not found → returns original unchanged
        let env = result.env.as_ref().expect("env");
        assert_eq!(env["HOST"], "val");
    }

    // ── redact_job tests ───────────────────────────────────────────────

    #[test]
    fn test_redact_job_full() {
        let redacter = Redacter::default();
        let job = Job {
            id: Some("job-1".to_string()),
            inputs: Some(HashMap::from([
                (
                    "url".to_string(),
                    "http://user:token_pw@api.com".to_string(),
                ),
                ("name".to_string(), "test".to_string()),
            ])),
            tasks: vec![Task {
                env: Some(HashMap::from([(
                    "DB_PASSWORD".to_string(),
                    "dbpw".to_string(),
                )])),
                ..Default::default()
            }],
            execution: vec![Task {
                env: Some(HashMap::from([(
                    "EXEC_ACCESS_KEY".to_string(),
                    "execak".to_string(),
                )])),
                ..Default::default()
            }],
            context: JobContext {
                inputs: Some(HashMap::from([(
                    "context_secret".to_string(),
                    "cs123".to_string(),
                )])),
                secrets: Some(HashMap::from([
                    ("s1".to_string(), "token_pw".to_string()),
                    ("s2".to_string(), "other".to_string()),
                ])),
                tasks: Some(HashMap::from([(
                    "task_secret".to_string(),
                    "ts123".to_string(),
                )])),
                ..Default::default()
            },
            webhooks: Some(vec![Webhook {
                url: Some("http://hook.com".to_string()),
                headers: Some(HashMap::from([(
                    "X-Secret".to_string(),
                    "token_pw".to_string(),
                )])),
                event: None,
                r#if: None,
            }]),
            secrets: Some(HashMap::from([
                ("s1".to_string(), "token_pw".to_string()),
                ("s2".to_string(), "other".to_string()),
            ])),
            ..Default::default()
        };

        let result = redacter.redact_job(&job);

        // Inputs: secret replaced in value
        let inputs = result.inputs.as_ref().expect("inputs");
        assert_eq!(inputs["url"], "http://user:[REDACTED]@api.com");
        assert_eq!(inputs["name"], "test");

        // Secrets: all values blanked
        let secrets = result.secrets.as_ref().expect("secrets");
        assert_eq!(secrets["s1"], REDACTED_STR);
        assert_eq!(secrets["s2"], REDACTED_STR);

        // Context secrets: values blanked by key match + secret replacement
        let ctx_secrets = result.context.secrets.as_ref().expect("context secrets");
        assert_eq!(ctx_secrets["s1"], REDACTED_STR); // key matches SECRET
        assert_eq!(ctx_secrets["s2"], REDACTED_STR); // secret value "other" is unique, but key matches SECRET

        // Context inputs: key "context_secret" matches SECRET matcher → redacted
        let ctx_inputs = result.context.inputs.as_ref().expect("context inputs");
        assert_eq!(ctx_inputs["context_secret"], REDACTED_STR);

        // Tasks redacted
        let task_env = result.tasks[0].env.as_ref().expect("task env");
        assert_eq!(task_env["DB_PASSWORD"], REDACTED_STR);

        // Execution redacted
        let exec_env = result.execution[0].env.as_ref().expect("execution env");
        assert_eq!(exec_env["EXEC_ACCESS_KEY"], REDACTED_STR);

        // Webhook headers redacted (key contains SECRET)
        let wh_headers = result.webhooks.as_ref().expect("webhooks")[0]
            .headers
            .as_ref()
            .expect("webhook headers");
        assert_eq!(wh_headers["X-Secret"], REDACTED_STR);
    }

    #[test]
    fn test_redact_job_no_secrets() {
        let redacter = Redacter::default();
        let job = Job {
            inputs: Some(HashMap::from([(
                "url".to_string(),
                "http://api.com".to_string(),
            )])),
            ..Default::default()
        };

        let result = redacter.redact_job(&job);
        let inputs = result.inputs.as_ref().expect("inputs");
        assert_eq!(inputs["url"], "http://api.com");
        assert!(result.secrets.is_none());
    }

    #[test]
    fn test_redact_job_preserves_id() {
        let redacter = Redacter::default();
        let job = Job {
            id: Some("preserved-id".to_string()),
            ..Default::default()
        };

        let result = redacter.redact_job(&job);
        assert_eq!(result.id, Some("preserved-id".to_string()));
    }

    // ── redact_task_log_part tests ─────────────────────────────────────

    #[test]
    fn test_redact_task_log_part() {
        let redacter = Redacter::new(vec![]);
        let part = TaskLogPart {
            id: Some("part-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: Some("line with secret123 and more".to_string()),
            created_at: None,
        };
        let secrets = HashMap::from([("my_secret".to_string(), "secret123".to_string())]);

        let result = redacter.redact_task_log_part(&part, &secrets);

        assert_eq!(
            result.contents,
            Some("line with [REDACTED] and more".to_string())
        );
        assert_eq!(result.id, Some("part-1".to_string()));
        assert_eq!(result.number, 1);
    }

    #[test]
    fn test_redact_task_log_part_no_contents() {
        let redacter = Redacter::default();
        let part = TaskLogPart {
            id: Some("part-1".to_string()),
            number: 1,
            task_id: Some("task-1".to_string()),
            contents: None,
            created_at: None,
        };
        let secrets = HashMap::from([("k".to_string(), "v".to_string())]);

        let result = redacter.redact_task_log_part(&part, &secrets);
        assert!(result.contents.is_none());
    }

    #[test]
    fn test_redact_task_log_part_multiple_secrets() {
        let redacter = Redacter::new(vec![]);
        let part = TaskLogPart {
            id: None,
            number: 0,
            task_id: None,
            contents: Some("user=admin&pass=s3cret&token=abcd".to_string()),
            created_at: None,
        };
        let secrets = HashMap::from([
            ("p".to_string(), "s3cret".to_string()),
            ("t".to_string(), "abcd".to_string()),
        ]);

        let result = redacter.redact_task_log_part(&part, &secrets);
        assert_eq!(
            result.contents,
            Some("user=admin&pass=[REDACTED]&token=[REDACTED]".to_string())
        );
    }

    // ── Default Redacter ───────────────────────────────────────────────

    #[test]
    fn test_default_redacter_has_three_matchers() {
        let redacter = Redacter::default();
        assert_eq!(redacter.matchers.len(), 3);
    }

    #[test]
    fn test_custom_matchers_override_defaults() {
        let redacter = Redacter::new(vec![contains_matcher("CUSTOM")]);
        assert_eq!(redacter.matchers.len(), 1);
        assert!(redacter.should_redact("MY_CUSTOM_KEY"));
        assert!(!redacter.should_redact("PASSWORD"));
    }
}
