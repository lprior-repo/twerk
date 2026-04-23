//! Retrier and `JitterStrategy` types for ASL retry policies.
//!
//! A Retrier specifies how a state retries on specific errors,
//! with exponential backoff and optional jitter.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::error_code::ErrorCode;
use super::types::BackoffRate;

// ---------------------------------------------------------------------------
// JitterStrategy
// ---------------------------------------------------------------------------

/// Controls whether random jitter is added to computed retry delays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JitterStrategy {
    Full,
    #[default]
    None,
}

impl fmt::Display for JitterStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => f.write_str("FULL"),
            Self::None => f.write_str("NONE"),
        }
    }
}

// ---------------------------------------------------------------------------
// RetrierError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RetrierError {
    #[error("retrier error_equals must not be empty")]
    EmptyErrorEquals,
    #[error("retrier interval_seconds must be >= 1, got {0}")]
    IntervalTooSmall(u64),
    #[error("retrier max_attempts must be >= 1, got {0}")]
    MaxAttemptsTooSmall(u32),
    #[error("retrier max_delay_seconds ({max_delay}) must be > interval_seconds ({interval})")]
    MaxDelayNotGreaterThanInterval { max_delay: u64, interval: u64 },
}

// ---------------------------------------------------------------------------
// Retrier
// ---------------------------------------------------------------------------

/// A retry policy attached to a state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "RawRetrier")]
pub struct Retrier {
    error_equals: Vec<ErrorCode>,
    interval_seconds: u64,
    max_attempts: u32,
    backoff_rate: BackoffRate,
    max_delay_seconds: Option<u64>,
    jitter_strategy: JitterStrategy,
}

impl Retrier {
    pub fn new(
        error_equals: Vec<ErrorCode>,
        interval_seconds: u64,
        max_attempts: u32,
        backoff_rate: BackoffRate,
        max_delay_seconds: Option<u64>,
        jitter_strategy: JitterStrategy,
    ) -> Result<Self, RetrierError> {
        if error_equals.is_empty() {
            return Err(RetrierError::EmptyErrorEquals);
        }
        if interval_seconds < 1 {
            return Err(RetrierError::IntervalTooSmall(interval_seconds));
        }
        if max_attempts < 1 {
            return Err(RetrierError::MaxAttemptsTooSmall(max_attempts));
        }
        if let Some(d) = max_delay_seconds {
            if d <= interval_seconds {
                return Err(RetrierError::MaxDelayNotGreaterThanInterval {
                    max_delay: d,
                    interval: interval_seconds,
                });
            }
        }
        Ok(Self {
            error_equals,
            interval_seconds,
            max_attempts,
            backoff_rate,
            max_delay_seconds,
            jitter_strategy,
        })
    }

    #[must_use]
    pub fn error_equals(&self) -> &[ErrorCode] {
        &self.error_equals
    }

    #[must_use]
    pub fn interval_seconds(&self) -> u64 {
        self.interval_seconds
    }

    #[must_use]
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    #[must_use]
    pub fn backoff_rate(&self) -> BackoffRate {
        self.backoff_rate
    }

    #[must_use]
    pub fn max_delay_seconds(&self) -> Option<u64> {
        self.max_delay_seconds
    }

    #[must_use]
    pub fn jitter_strategy(&self) -> JitterStrategy {
        self.jitter_strategy
    }
}

// ---------------------------------------------------------------------------
// Serde raw helper for deserialization with validation
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRetrier {
    error_equals: Vec<ErrorCode>,
    interval_seconds: u64,
    max_attempts: u32,
    backoff_rate: BackoffRate,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_delay_seconds: Option<u64>,
    #[serde(default)]
    jitter_strategy: JitterStrategy,
}

impl TryFrom<RawRetrier> for Retrier {
    type Error = RetrierError;

    fn try_from(raw: RawRetrier) -> Result<Self, Self::Error> {
        Self::new(
            raw.error_equals,
            raw.interval_seconds,
            raw.max_attempts,
            raw.backoff_rate,
            raw.max_delay_seconds,
            raw.jitter_strategy,
        )
    }
}

impl From<Retrier> for RawRetrier {
    fn from(r: Retrier) -> Self {
        Self {
            error_equals: r.error_equals,
            interval_seconds: r.interval_seconds,
            max_attempts: r.max_attempts,
            backoff_rate: r.backoff_rate,
            max_delay_seconds: r.max_delay_seconds,
            jitter_strategy: r.jitter_strategy,
        }
    }
}

impl<'de> Deserialize<'de> for Retrier {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawRetrier::deserialize(deserializer)?;
        Retrier::try_from(raw).map_err(serde::de::Error::custom)
    }
}
