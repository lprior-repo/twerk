//! TaskState: the primary execution state that runs a container image.
//!
//! Enforces all invariants at construction time (INV-TS1 through INV-TS11).
//! Immutable after construction — no setters.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::catcher::Catcher;
use super::retrier::Retrier;
use super::transition::Transition;
use super::types::{Expression, ImageRef, ShellScript, VariableName};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Error)]
pub enum TaskStateError {
    #[error("task state timeout must be >= 1 second, got {0}")]
    TimeoutTooSmall(u64),
    #[error("task state heartbeat must be >= 1 second, got {0}")]
    HeartbeatTooSmall(u64),
    #[error("task state heartbeat ({heartbeat}s) must be less than timeout ({timeout}s)")]
    HeartbeatExceedsTimeout { heartbeat: u64, timeout: u64 },
    #[error("task state env key cannot be empty")]
    EmptyEnvKey,
}

// ---------------------------------------------------------------------------
// TaskState
// ---------------------------------------------------------------------------

/// The primary workhorse state: runs a container image with a shell command,
/// captures output, and supports retry/catch error handling.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "RawTaskState")]
pub struct TaskState {
    image: ImageRef,
    run: ShellScript,
    env: HashMap<String, Expression>,
    var: Option<VariableName>,
    timeout: Option<u64>,
    heartbeat: Option<u64>,
    retry: Vec<Retrier>,
    catch: Vec<Catcher>,
    transition: Transition,
}

impl TaskState {
    /// Validated constructor.
    ///
    /// Returns `Err` if any invariant (INV-TS1 through INV-TS11) is violated.
    /// Field types that are already-validated newtypes (ImageRef, ShellScript,
    /// etc.) guarantee their own invariants via the type system.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        image: ImageRef,
        run: ShellScript,
        env: HashMap<String, Expression>,
        var: Option<VariableName>,
        timeout: Option<u64>,
        heartbeat: Option<u64>,
        retry: Vec<Retrier>,
        catch: Vec<Catcher>,
        transition: Transition,
    ) -> Result<Self, TaskStateError> {
        // INV-TS4: env keys must be non-empty
        if env.keys().any(String::is_empty) {
            return Err(TaskStateError::EmptyEnvKey);
        }
        // INV-TS6: timeout >= 1
        if let Some(t) = timeout {
            if t < 1 {
                return Err(TaskStateError::TimeoutTooSmall(t));
            }
        }
        // INV-TS7: heartbeat >= 1
        if let Some(h) = heartbeat {
            if h < 1 {
                return Err(TaskStateError::HeartbeatTooSmall(h));
            }
        }
        // INV-TS8: heartbeat < timeout (when both present)
        if let (Some(t), Some(h)) = (timeout, heartbeat) {
            if h >= t {
                return Err(TaskStateError::HeartbeatExceedsTimeout {
                    heartbeat: h,
                    timeout: t,
                });
            }
        }
        Ok(Self {
            image,
            run,
            env,
            var,
            timeout,
            heartbeat,
            retry,
            catch,
            transition,
        })
    }

    #[must_use]
    pub fn image(&self) -> &ImageRef {
        &self.image
    }

    #[must_use]
    pub fn run(&self) -> &ShellScript {
        &self.run
    }

    #[must_use]
    pub fn env(&self) -> &HashMap<String, Expression> {
        &self.env
    }

    #[must_use]
    pub fn var(&self) -> Option<&VariableName> {
        self.var.as_ref()
    }

    #[must_use]
    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    #[must_use]
    pub fn heartbeat(&self) -> Option<u64> {
        self.heartbeat
    }

    #[must_use]
    pub fn retry(&self) -> &[Retrier] {
        &self.retry
    }

    #[must_use]
    pub fn catch(&self) -> &[Catcher] {
        &self.catch
    }

    #[must_use]
    pub fn transition(&self) -> &Transition {
        &self.transition
    }
}

// ---------------------------------------------------------------------------
// Serde raw helper for deserialization with validation
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTaskState {
    image: ImageRef,
    run: ShellScript,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    env: HashMap<String, Expression>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    var: Option<VariableName>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    heartbeat: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    retry: Vec<Retrier>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    catch: Vec<Catcher>,
    #[serde(flatten)]
    transition: Transition,
}

impl TryFrom<RawTaskState> for TaskState {
    type Error = TaskStateError;

    fn try_from(raw: RawTaskState) -> Result<Self, Self::Error> {
        Self::new(
            raw.image,
            raw.run,
            raw.env,
            raw.var,
            raw.timeout,
            raw.heartbeat,
            raw.retry,
            raw.catch,
            raw.transition,
        )
    }
}

impl From<TaskState> for RawTaskState {
    fn from(ts: TaskState) -> Self {
        Self {
            image: ts.image,
            run: ts.run,
            env: ts.env,
            var: ts.var,
            timeout: ts.timeout,
            heartbeat: ts.heartbeat,
            retry: ts.retry,
            catch: ts.catch,
            transition: ts.transition,
        }
    }
}

impl<'de> Deserialize<'de> for TaskState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawTaskState::deserialize(deserializer)?;
        TaskState::try_from(raw).map_err(serde::de::Error::custom)
    }
}
