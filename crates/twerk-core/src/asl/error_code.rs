//! ErrorCode enum for ASL error matching.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    All,
    Timeout,
    TaskFailed,
    Permissions,
    ResultPathMatchFailure,
    ParameterPathFailure,
    BranchFailed,
    NoChoiceMatched,
    IntrinsicFailure,
    HeartbeatTimeout,
    Custom(String),
}

impl ErrorCode {
    /// Parse a string into an `ErrorCode` (case-insensitive for known variants).
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "all" => Self::All,
            "timeout" => Self::Timeout,
            "taskfailed" => Self::TaskFailed,
            "permissions" => Self::Permissions,
            "resultpathmatchfailure" => Self::ResultPathMatchFailure,
            "parameterpathfailure" => Self::ParameterPathFailure,
            "branchfailed" => Self::BranchFailed,
            "nochoicematched" => Self::NoChoiceMatched,
            "intrinsicfailure" => Self::IntrinsicFailure,
            "heartbeattimeout" => Self::HeartbeatTimeout,
            _ => Self::Custom(s.to_owned()),
        }
    }

    /// Returns `true` if `self` matches `other`.
    /// `All` matches everything; otherwise exact equality.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        *self == Self::All || self == other
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Timeout => f.write_str("timeout"),
            Self::TaskFailed => f.write_str("taskfailed"),
            Self::Permissions => f.write_str("permissions"),
            Self::ResultPathMatchFailure => f.write_str("resultpathmatchfailure"),
            Self::ParameterPathFailure => f.write_str("parameterpathfailure"),
            Self::BranchFailed => f.write_str("branchfailed"),
            Self::NoChoiceMatched => f.write_str("nochoicematched"),
            Self::IntrinsicFailure => f.write_str("intrinsicfailure"),
            Self::HeartbeatTimeout => f.write_str("heartbeattimeout"),
            Self::Custom(s) => f.write_str(s),
        }
    }
}

impl FromStr for ErrorCode {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::parse(&s))
    }
}
