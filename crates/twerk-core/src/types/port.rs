//! Network port types.
//!
//! Provides [`Port`] - a validated network port number (1-65535).

use core::fmt;
use core::ops::Deref;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated network port number (1-65535).
///
/// TCP and UDP ports are in the range 1-65535. Port 0 is reserved and invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
#[must_use = "Port should be used; it validates at construction"]
pub struct Port(u16);

/// Errors that can arise when constructing a [`Port`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    #[error("Port {value} out of valid range {min}..={max}")]
    OutOfRange { value: u16, min: u16, max: u16 },
}

impl Port {
    /// Create a new `Port`, returning an error if outside valid TCP/UDP range.
    ///
    /// # Errors
    /// Returns [`PortError::OutOfRange`] if value is 0 or > 65535.
    pub fn new(value: u16) -> Result<Self, PortError> {
        if value < 1 {
            Err(PortError::OutOfRange {
                value,
                min: 1,
                max: 65535,
            })
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the raw port value.
    #[must_use]
    pub fn value(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for Port {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u16> for Port {
    fn as_ref(&self) -> &u16 {
        &self.0
    }
}

impl FromStr for Port {
    type Err = PortError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u16 = s.parse().map_err(|_| PortError::OutOfRange {
            value: 0,
            min: 1,
            max: 65535,
        })?;
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Port::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn port_new_accepts_valid(p in 1u16..=65535) {
            prop_assert!(Port::new(p).is_ok());
        }

        #[test]
        fn port_value_roundtrip(p in 1u16..=65535) {
            let port = Port::new(p).unwrap();
            prop_assert_eq!(port.value(), p);
        }

        #[test]
        fn port_from_str_roundtrip(p in 1u16..=65535) {
            let s = p.to_string();
            let port: Port = s.parse().unwrap();
            prop_assert_eq!(port.value(), p);
        }

        #[test]
        fn port_serde_roundtrip(p in 1u16..=65535) {
            let port = Port::new(p).unwrap();
            let json = serde_json::to_string(&port).unwrap();
            let back: Port = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(port, back);
        }
    }
}
