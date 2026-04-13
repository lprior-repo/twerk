//! WaitDuration and WaitState types for ASL pause semantics.
//!
//! A WaitState pauses execution for a duration before transitioning.
//! WaitDuration is mutually exclusive: exactly one of four fields.

use std::fmt;

use serde::de::{self, Deserializer};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use thiserror::Error;

use super::transition::{Transition, TransitionError};
use super::types::{JsonPath, JsonPathError, StateName, StateNameError};

// ---------------------------------------------------------------------------
// WaitDurationError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WaitDurationError {
    #[error("wait duration must specify exactly one of: seconds, timestamp, seconds_path, timestamp_path")]
    NoFieldSpecified,
    #[error("wait duration has multiple fields set: {fields:?}")]
    MultipleFieldsSpecified { fields: Vec<String> },
    #[error("wait timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("invalid json path: {0}")]
    InvalidJsonPath(#[from] JsonPathError),
}

// ---------------------------------------------------------------------------
// WaitDuration
// ---------------------------------------------------------------------------

/// How long a Wait state pauses execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WaitDuration {
    Seconds(u64),
    Timestamp(String),
    SecondsPath(JsonPath),
    TimestampPath(JsonPath),
}

impl WaitDuration {
    #[must_use]
    pub fn is_seconds(&self) -> bool {
        matches!(self, Self::Seconds(_))
    }

    #[must_use]
    pub fn is_timestamp(&self) -> bool {
        matches!(self, Self::Timestamp(_))
    }

    #[must_use]
    pub fn is_seconds_path(&self) -> bool {
        matches!(self, Self::SecondsPath(_))
    }

    #[must_use]
    pub fn is_timestamp_path(&self) -> bool {
        matches!(self, Self::TimestampPath(_))
    }
}

impl fmt::Display for WaitDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seconds(n) => write!(f, "seconds: {n}"),
            Self::Timestamp(t) => write!(f, "timestamp: {t}"),
            Self::SecondsPath(p) => write!(f, "seconds_path: {p}"),
            Self::TimestampPath(p) => write!(f, "timestamp_path: {p}"),
        }
    }
}

// ---------------------------------------------------------------------------
// WaitDuration Serde
// ---------------------------------------------------------------------------

impl Serialize for WaitDuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Seconds(n) => map.serialize_entry("seconds", n)?,
            Self::Timestamp(t) => map.serialize_entry("timestamp", t)?,
            Self::SecondsPath(p) => map.serialize_entry("seconds_path", p.as_str())?,
            Self::TimestampPath(p) => map.serialize_entry("timestamp_path", p.as_str())?,
        }
        map.end()
    }
}

#[derive(Deserialize)]
struct WaitDurationHelper {
    seconds: Option<u64>,
    timestamp: Option<String>,
    seconds_path: Option<String>,
    timestamp_path: Option<String>,
}

impl WaitDurationHelper {
    fn into_wait_duration(self) -> Result<WaitDuration, WaitDurationError> {
        let mut fields = Vec::new();
        if self.seconds.is_some() {
            fields.push("seconds".to_owned());
        }
        if self.timestamp.is_some() {
            fields.push("timestamp".to_owned());
        }
        if self.seconds_path.is_some() {
            fields.push("seconds_path".to_owned());
        }
        if self.timestamp_path.is_some() {
            fields.push("timestamp_path".to_owned());
        }

        if fields.len() > 1 {
            return Err(WaitDurationError::MultipleFieldsSpecified { fields });
        }

        if let Some(n) = self.seconds {
            return Ok(WaitDuration::Seconds(n));
        }
        if let Some(t) = self.timestamp {
            if t.is_empty() {
                return Err(WaitDurationError::EmptyTimestamp);
            }
            return Ok(WaitDuration::Timestamp(t));
        }
        if let Some(p) = self.seconds_path {
            let jp = JsonPath::new(p)?;
            return Ok(WaitDuration::SecondsPath(jp));
        }
        if let Some(p) = self.timestamp_path {
            let jp = JsonPath::new(p)?;
            return Ok(WaitDuration::TimestampPath(jp));
        }

        Err(WaitDurationError::NoFieldSpecified)
    }
}

impl<'de> Deserialize<'de> for WaitDuration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let helper = WaitDurationHelper::deserialize(deserializer)?;
        helper.into_wait_duration().map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// WaitState
// ---------------------------------------------------------------------------

/// A state that pauses execution for a duration before transitioning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitState {
    duration: WaitDuration,
    transition: Transition,
}

impl WaitState {
    #[must_use]
    pub fn new(duration: WaitDuration, transition: Transition) -> Self {
        Self {
            duration,
            transition,
        }
    }

    #[must_use]
    pub fn duration(&self) -> &WaitDuration {
        &self.duration
    }

    #[must_use]
    pub fn transition(&self) -> &Transition {
        &self.transition
    }
}

// ---------------------------------------------------------------------------
// WaitState Serde — flattened duration + transition fields
// ---------------------------------------------------------------------------

impl Serialize for WaitState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let transition_is_next = self.transition.is_next();
        let map_len = 2; // one duration field + one transition field
        let mut map = serializer.serialize_map(Some(map_len))?;

        match &self.duration {
            WaitDuration::Seconds(n) => map.serialize_entry("seconds", n)?,
            WaitDuration::Timestamp(t) => map.serialize_entry("timestamp", t)?,
            WaitDuration::SecondsPath(p) => map.serialize_entry("seconds_path", p.as_str())?,
            WaitDuration::TimestampPath(p) => map.serialize_entry("timestamp_path", p.as_str())?,
        }

        if transition_is_next {
            if let Some(name) = self.transition.target_state() {
                map.serialize_entry("next", name.as_str())?;
            }
        } else {
            map.serialize_entry("end", &true)?;
        }

        map.end()
    }
}

impl<'de> Deserialize<'de> for WaitState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct WaitStateHelper {
            // Duration fields
            seconds: Option<u64>,
            timestamp: Option<String>,
            seconds_path: Option<String>,
            timestamp_path: Option<String>,
            // Transition fields
            next: Option<String>,
            end: Option<bool>,
        }

        let helper = WaitStateHelper::deserialize(deserializer)?;

        // Build duration
        let dur_helper = WaitDurationHelper {
            seconds: helper.seconds,
            timestamp: helper.timestamp,
            seconds_path: helper.seconds_path,
            timestamp_path: helper.timestamp_path,
        };
        let duration = dur_helper.into_wait_duration().map_err(de::Error::custom)?;

        // Build transition
        let transition = match (helper.next, helper.end) {
            (Some(_), Some(_)) => {
                return Err(de::Error::custom(TransitionError::BothNextAndEnd));
            }
            (None, None) => {
                return Err(de::Error::custom(TransitionError::NeitherNextNorEnd));
            }
            (Some(name), None) => {
                let sn = StateName::new(name).map_err(|e: StateNameError| {
                    de::Error::custom(TransitionError::InvalidStateName(e))
                })?;
                Transition::Next(sn)
            }
            (None, Some(true)) => Transition::End,
            (None, Some(false)) => {
                return Err(de::Error::custom(TransitionError::EndMustBeTrue));
            }
        };

        Ok(WaitState {
            duration,
            transition,
        })
    }
}
