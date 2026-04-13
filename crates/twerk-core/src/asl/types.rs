//! NewType wrappers for ASL (Amazon States Language) primitives.
//!
//! Each type enforces validation at construction time, making illegal states
//! unrepresentable. Custom `Deserialize` impls reject invalid values.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Generates Display, FromStr, AsRef<str>, Deref<Target=str>, and
/// validating Serialize/Deserialize for a string newtype.
macro_rules! str_newtype_impls {
    ($ty:ident, $err:ty) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl FromStr for $ty {
            type Err = $err;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }
        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
        impl Deref for $ty {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// StateName
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateNameError {
    #[error("state name cannot be empty")]
    Empty,
    #[error("state name length {0} exceeds maximum of 256 characters")]
    TooLong(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateName(String);

impl StateName {
    pub fn new(name: impl Into<String>) -> Result<Self, StateNameError> {
        let s = name.into();
        if s.is_empty() || s.trim().is_empty() {
            return Err(StateNameError::Empty);
        }
        if s.len() > 256 {
            return Err(StateNameError::TooLong(s.len()));
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(StateName, StateNameError);

// ---------------------------------------------------------------------------
// Expression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExpressionError {
    #[error("expression cannot be empty")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Expression(String);

impl Expression {
    pub fn new(expr: impl Into<String>) -> Result<Self, ExpressionError> {
        let s = expr.into();
        if s.is_empty() {
            return Err(ExpressionError::Empty);
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(Expression, ExpressionError);

// ---------------------------------------------------------------------------
// JsonPath
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JsonPathError {
    #[error("JSON path cannot be empty")]
    Empty,
    #[error("JSON path must start with '$', got '{0}'")]
    MissingDollarPrefix(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsonPath(String);

impl JsonPath {
    pub fn new(path: impl Into<String>) -> Result<Self, JsonPathError> {
        let s = path.into();
        if s.is_empty() {
            return Err(JsonPathError::Empty);
        }
        if !s.starts_with('$') {
            return Err(JsonPathError::MissingDollarPrefix(s));
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(JsonPath, JsonPathError);

// ---------------------------------------------------------------------------
// VariableName
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VariableNameError {
    #[error("variable name cannot be empty")]
    Empty,
    #[error("variable name length {0} exceeds maximum of 128 characters")]
    TooLong(usize),
    #[error("variable name must start with ASCII letter or underscore, got '{0}'")]
    InvalidStart(char),
    #[error("variable name contains invalid character '{0}'")]
    InvalidCharacter(char),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableName(String);

impl VariableName {
    pub fn new(name: impl Into<String>) -> Result<Self, VariableNameError> {
        let s = name.into();
        if s.is_empty() {
            return Err(VariableNameError::Empty);
        }
        if s.len() > 128 {
            return Err(VariableNameError::TooLong(s.len()));
        }
        let mut chars = s.chars();
        let first = match chars.next() {
            Some(c) => c,
            None => return Err(VariableNameError::Empty),
        };
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(VariableNameError::InvalidStart(first));
        }
        for c in chars {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Err(VariableNameError::InvalidCharacter(c));
            }
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(VariableName, VariableNameError);

// ---------------------------------------------------------------------------
// ImageRef
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImageRefError {
    #[error("image reference cannot be empty")]
    Empty,
    #[error("image reference contains whitespace")]
    ContainsWhitespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRef(String);

impl ImageRef {
    pub fn new(image: impl Into<String>) -> Result<Self, ImageRefError> {
        let s = image.into();
        if s.is_empty() {
            return Err(ImageRefError::Empty);
        }
        if s.chars().any(|c| c.is_whitespace()) {
            return Err(ImageRefError::ContainsWhitespace);
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(ImageRef, ImageRefError);

// ---------------------------------------------------------------------------
// ShellScript
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShellScriptError {
    #[error("shell script cannot be empty")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShellScript(String);

impl ShellScript {
    pub fn new(script: impl Into<String>) -> Result<Self, ShellScriptError> {
        let s = script.into();
        if s.is_empty() {
            return Err(ShellScriptError::Empty);
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

str_newtype_impls!(ShellScript, ShellScriptError);

// ---------------------------------------------------------------------------
// BackoffRate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Error)]
pub enum BackoffRateError {
    #[error("backoff rate must be positive, got {0}")]
    NotPositive(f64),
    #[error("backoff rate must be finite, got {0}")]
    NotFinite(f64),
    #[error("failed to parse backoff rate: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackoffRate(f64);

impl BackoffRate {
    pub fn new(rate: f64) -> Result<Self, BackoffRateError> {
        if !rate.is_finite() {
            return Err(BackoffRateError::NotFinite(rate));
        }
        if rate <= 0.0 {
            return Err(BackoffRateError::NotPositive(rate));
        }
        Ok(Self(rate))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl fmt::Display for BackoffRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BackoffRate {
    type Err = BackoffRateError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: f64 = s
            .parse()
            .map_err(|e: std::num::ParseFloatError| BackoffRateError::ParseError(e.to_string()))?;
        Self::new(v)
    }
}

impl Serialize for BackoffRate {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for BackoffRate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(d)?;
        Self::new(v).map_err(serde::de::Error::custom)
    }
}
