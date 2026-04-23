//! `MapState`: iterate over an array with a sub-state-machine per element.
//!
//! Enforces INV-MS1 through INV-MS5 at construction time.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::catcher::Catcher;
use super::machine::StateMachine;
use super::retrier::Retrier;
use super::transition::Transition;
use super::types::Expression;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Error)]
pub enum MapStateError {
    #[error("tolerated failure percentage must be 0.0..=100.0, got {0}")]
    InvalidToleratedFailurePercentage(f64),
    #[error("tolerated failure percentage must be finite")]
    NonFiniteToleratedFailurePercentage,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_tolerance(pct: Option<f64>) -> Result<(), MapStateError> {
    if let Some(p) = pct {
        if !p.is_finite() {
            return Err(MapStateError::NonFiniteToleratedFailurePercentage);
        }
        if !(0.0..=100.0).contains(&p) {
            return Err(MapStateError::InvalidToleratedFailurePercentage(p));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// MapState
// ---------------------------------------------------------------------------

/// Iterates over an array, running a sub-state-machine per element.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "RawMapState")]
pub struct MapState {
    items_path: Expression,
    item_processor: Box<StateMachine>,
    max_concurrency: Option<u32>,
    transition: Transition,
    retry: Vec<Retrier>,
    catch: Vec<Catcher>,
    tolerated_failure_percentage: Option<f64>,
}

impl MapState {
    /// Validated constructor.
    ///
    /// Returns `Err` if `tolerated_failure_percentage` violates INV-MS5
    /// (must be finite and in 0.0..=100.0 when present).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        items_path: Expression,
        item_processor: Box<StateMachine>,
        max_concurrency: Option<u32>,
        transition: Transition,
        retry: Vec<Retrier>,
        catch: Vec<Catcher>,
        tolerated_failure_percentage: Option<f64>,
    ) -> Result<Self, MapStateError> {
        validate_tolerance(tolerated_failure_percentage)?;
        Ok(Self {
            items_path,
            item_processor,
            max_concurrency,
            transition,
            retry,
            catch,
            tolerated_failure_percentage,
        })
    }

    #[must_use]
    pub fn items_path(&self) -> &Expression {
        &self.items_path
    }

    #[must_use]
    pub fn item_processor(&self) -> &StateMachine {
        &self.item_processor
    }

    #[must_use]
    pub fn max_concurrency(&self) -> Option<u32> {
        self.max_concurrency
    }

    #[must_use]
    pub fn transition(&self) -> &Transition {
        &self.transition
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
    pub fn tolerated_failure_percentage(&self) -> Option<f64> {
        self.tolerated_failure_percentage
    }
}

// ---------------------------------------------------------------------------
// Serde raw helper for deserialization with validation
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMapState {
    items_path: Expression,
    item_processor: Box<StateMachine>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    max_concurrency: Option<u32>,
    #[serde(flatten)]
    transition: Transition,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    retry: Vec<Retrier>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    catch: Vec<Catcher>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerated_failure_percentage: Option<f64>,
}

impl TryFrom<RawMapState> for MapState {
    type Error = MapStateError;

    fn try_from(raw: RawMapState) -> Result<Self, Self::Error> {
        Self::new(
            raw.items_path,
            raw.item_processor,
            raw.max_concurrency,
            raw.transition,
            raw.retry,
            raw.catch,
            raw.tolerated_failure_percentage,
        )
    }
}

impl From<MapState> for RawMapState {
    fn from(ms: MapState) -> Self {
        Self {
            items_path: ms.items_path,
            item_processor: ms.item_processor,
            max_concurrency: ms.max_concurrency,
            transition: ms.transition,
            retry: ms.retry,
            catch: ms.catch,
            tolerated_failure_percentage: ms.tolerated_failure_percentage,
        }
    }
}

impl<'de> Deserialize<'de> for MapState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawMapState::deserialize(deserializer)?;
        MapState::try_from(raw).map_err(serde::de::Error::custom)
    }
}
