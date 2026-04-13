//! Transition enum for ASL state machine flow control.
//!
//! A Transition represents what happens after a state completes:
//! either proceed to a named next state, or end the execution.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use thiserror::Error;

use super::types::{StateName, StateNameError};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransitionError {
    #[error("transition has both 'next' and 'end' fields set")]
    BothNextAndEnd,
    #[error("transition has neither 'next' nor 'end' field")]
    NeitherNextNorEnd,
    #[error("transition 'end' field must be true, got false")]
    EndMustBeTrue,
    #[error("invalid state name in transition: {0}")]
    InvalidStateName(#[from] StateNameError),
}

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

/// How a state declares what happens after it completes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Transition {
    Next(StateName),
    End,
}

impl Transition {
    #[must_use]
    pub fn next(name: StateName) -> Self {
        Self::Next(name)
    }

    #[must_use]
    pub fn end() -> Self {
        Self::End
    }

    #[must_use]
    pub fn is_next(&self) -> bool {
        matches!(self, Self::Next(_))
    }

    #[must_use]
    pub fn is_end(&self) -> bool {
        matches!(self, Self::End)
    }

    #[must_use]
    pub fn target_state(&self) -> Option<&StateName> {
        match self {
            Self::Next(name) => Some(name),
            Self::End => None,
        }
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Next(name) => write!(f, "-> {name}"),
            Self::End => f.write_str("END"),
        }
    }
}

// ---------------------------------------------------------------------------
// Serde
// ---------------------------------------------------------------------------

impl Serialize for Transition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Next(name) => map.serialize_entry("next", name.as_str())?,
            Self::End => map.serialize_entry("end", &true)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Transition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Next,
            End,
        }

        struct TransitionVisitor;

        impl<'de> Visitor<'de> for TransitionVisitor {
            type Value = Transition;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map with either 'next' or 'end' field")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut next_val: Option<String> = None;
                let mut end_val: Option<bool> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Next => {
                            if next_val.is_some() {
                                return Err(de::Error::duplicate_field("next"));
                            }
                            next_val = Some(map.next_value()?);
                        }
                        Field::End => {
                            if end_val.is_some() {
                                return Err(de::Error::duplicate_field("end"));
                            }
                            end_val = Some(map.next_value()?);
                        }
                    }
                }

                match (next_val, end_val) {
                    (Some(_), Some(_)) => Err(de::Error::custom(TransitionError::BothNextAndEnd)),
                    (None, None) => Err(de::Error::custom(TransitionError::NeitherNextNorEnd)),
                    (Some(name), None) => {
                        let sn = StateName::new(name)
                            .map_err(|e| de::Error::custom(TransitionError::InvalidStateName(e)))?;
                        Ok(Transition::Next(sn))
                    }
                    (None, Some(true)) => Ok(Transition::End),
                    (None, Some(false)) => Err(de::Error::custom(TransitionError::EndMustBeTrue)),
                }
            }
        }

        deserializer.deserialize_map(TransitionVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_variant_stores_name() {
        let name = StateName::new("A").unwrap();
        let t = Transition::next(name.clone());
        assert_eq!(t.target_state(), Some(&name));
    }

    #[test]
    fn end_variant_has_no_target() {
        assert_eq!(Transition::end().target_state(), None);
    }
}
