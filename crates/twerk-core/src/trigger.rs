use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Trigger lifecycle states.
///
/// Each variant is a zero-cost discriminant (`Copy`). Serialization uses
/// `SCREAMING_SNAKE_CASE`. Parsing is case-insensitive. Default is `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TriggerState {
    #[default]
    Active,
    Paused,
    Disabled,
    Error,
}

/// Error returned when a string cannot be parsed as a [`TriggerState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTriggerStateError(pub String);

impl fmt::Display for ParseTriggerStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown TriggerState: {}", self.0)
    }
}

impl std::error::Error for ParseTriggerStateError {}

impl fmt::Display for TriggerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::Disabled => "DISABLED",
            Self::Error => "ERROR",
        };
        f.write_str(name)
    }
}

impl FromStr for TriggerState {
    type Err = ParseTriggerStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" => Ok(Self::Paused),
            "DISABLED" => Ok(Self::Disabled),
            "ERROR" => Ok(Self::Error),
            _ => Err(ParseTriggerStateError(s.to_string())),
        }
    }
}

// =========================================================================
// TESTS — RED PHASE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    // =====================================================================
    // TriggerState — serde serialization (Behaviors 1-4)
    // =====================================================================

    #[test]
    fn trigger_state_serializes_active_to_uppercase() {
        let json = serde_json::to_string(&TriggerState::Active).unwrap();
        assert_eq!(json, "\"ACTIVE\"");
    }

    #[test]
    fn trigger_state_serializes_paused_to_uppercase() {
        let json = serde_json::to_string(&TriggerState::Paused).unwrap();
        assert_eq!(json, "\"PAUSED\"");
    }

    #[test]
    fn trigger_state_serializes_disabled_to_uppercase() {
        let json = serde_json::to_string(&TriggerState::Disabled).unwrap();
        assert_eq!(json, "\"DISABLED\"");
    }

    #[test]
    fn trigger_state_serializes_error_to_uppercase() {
        let json = serde_json::to_string(&TriggerState::Error).unwrap();
        assert_eq!(json, "\"ERROR\"");
    }

    // =====================================================================
    // TriggerState — default (Behavior 5)
    // =====================================================================

    #[test]
    fn trigger_state_default_returns_active() {
        assert_eq!(TriggerState::default(), TriggerState::Active);
    }

    // =====================================================================
    // TriggerState — Display formatting (Behaviors 6-9)
    // =====================================================================

    #[test]
    fn trigger_state_display_formats_active() {
        assert_eq!(format!("{}", TriggerState::Active), "ACTIVE");
    }

    #[test]
    fn trigger_state_display_formats_paused() {
        assert_eq!(format!("{}", TriggerState::Paused), "PAUSED");
    }

    #[test]
    fn trigger_state_display_formats_disabled() {
        assert_eq!(format!("{}", TriggerState::Disabled), "DISABLED");
    }

    #[test]
    fn trigger_state_display_formats_error() {
        assert_eq!(format!("{}", TriggerState::Error), "ERROR");
    }

    // =====================================================================
    // TriggerState — FromStr valid parsing (Behaviors 10-13)
    // =====================================================================

    #[test]
    fn trigger_state_parses_lowercase_active() {
        assert_eq!(
            "active".parse::<TriggerState>().unwrap(),
            TriggerState::Active
        );
    }

    #[test]
    fn trigger_state_parses_uppercase_active() {
        assert_eq!(
            "ACTIVE".parse::<TriggerState>().unwrap(),
            TriggerState::Active
        );
    }

    #[test]
    fn trigger_state_parses_mixed_case_paused() {
        assert_eq!(
            "Paused".parse::<TriggerState>().unwrap(),
            TriggerState::Paused
        );
    }

    #[test]
    fn trigger_state_parses_lowercase_error() {
        assert_eq!(
            "error".parse::<TriggerState>().unwrap(),
            TriggerState::Error
        );
    }

    // =====================================================================
    // TriggerState — FromStr rejection (Behaviors 14-18)
    // =====================================================================

    #[test]
    fn trigger_state_parse_rejects_unknown_string() {
        let result: Result<TriggerState, _> = "DESTROYED".parse();
        assert_eq!(
            result,
            Err(ParseTriggerStateError(String::from("DESTROYED")))
        );
        assert_eq!(result.unwrap_err().0, "DESTROYED");
    }

    #[test]
    fn trigger_state_parse_rejects_empty_string() {
        let result: Result<TriggerState, _> = "".parse();
        assert_eq!(result, Err(ParseTriggerStateError(String::from(""))));
        assert_eq!(result.unwrap_err().0, "");
    }

    #[test]
    fn trigger_state_parse_rejects_whitespace_only() {
        let result: Result<TriggerState, _> = "   ".parse();
        assert_eq!(result, Err(ParseTriggerStateError(String::from("   "))));
        assert_eq!(result.unwrap_err().0, "   ");
    }

    #[test]
    fn trigger_state_parse_rejects_prefix_of_valid_name() {
        let result: Result<TriggerState, _> = "ACTIV".parse();
        assert_eq!(result, Err(ParseTriggerStateError(String::from("ACTIV"))));
        assert_eq!(result.unwrap_err().0, "ACTIV");
    }

    #[test]
    fn trigger_state_parse_rejects_trailing_whitespace() {
        let result: Result<TriggerState, _> = "ACTIVE ".parse();
        assert_eq!(result, Err(ParseTriggerStateError(String::from("ACTIVE "))));
        assert_eq!(result.unwrap_err().0, "ACTIVE ");
    }

    // =====================================================================
    // TriggerState — JSON deserialization (Behaviors 19-23)
    // =====================================================================

    #[test]
    fn trigger_state_deserializes_active_from_json() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"ACTIVE\"").unwrap(),
            TriggerState::Active
        );
    }

    #[test]
    fn trigger_state_deserializes_paused_from_json() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"PAUSED\"").unwrap(),
            TriggerState::Paused
        );
    }

    #[test]
    fn trigger_state_deserializes_disabled_from_json() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"DISABLED\"").unwrap(),
            TriggerState::Disabled
        );
    }

    #[test]
    fn trigger_state_deserializes_error_from_json() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"ERROR\"").unwrap(),
            TriggerState::Error
        );
    }

    #[test]
    fn trigger_state_deserialize_rejects_unknown_value() {
        let result: Result<TriggerState, serde_json::Error> = serde_json::from_str("\"UNKNOWN\"");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown variant"));
    }

    // =====================================================================
    // TriggerState — Display==serde roundtrip (Behavior 24)
    // =====================================================================

    #[rstest::rstest]
    #[case(TriggerState::Active)]
    #[case(TriggerState::Paused)]
    #[case(TriggerState::Disabled)]
    #[case(TriggerState::Error)]
    fn trigger_state_display_equals_serde_for_all_variants(#[case] state: TriggerState) {
        let display = format!("{state}");
        let serde_name = serde_json::to_string(&state)
            .unwrap()
            .trim_matches('"')
            .to_string();
        assert_eq!(display, serde_name);
    }

    // =====================================================================
    // ParseTriggerStateError (Behaviors 25-28)
    // =====================================================================

    #[test]
    fn parse_trigger_state_error_displays_message() {
        assert_eq!(
            format!("{}", ParseTriggerStateError(String::from("bad"))),
            "unknown TriggerState: bad"
        );
    }

    #[test]
    fn parse_trigger_state_error_implements_std_error() {
        let err = ParseTriggerStateError(String::from("test"));
        let e: &dyn std::error::Error = &err;
        assert!(e.source().is_none());
    }

    #[test]
    fn parse_trigger_state_error_partial_eq_compares_inner() {
        let err1 = ParseTriggerStateError(String::from("X"));
        let err2 = ParseTriggerStateError(String::from("X"));
        let err3 = ParseTriggerStateError(String::from("Y"));
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn parse_trigger_state_error_clone_produces_identical_copy() {
        let err = ParseTriggerStateError(String::from("test"));
        let cloned = err.clone();
        assert_eq!(cloned, err);
        assert_eq!(cloned.0, "test");
    }

    // =====================================================================
    // TriggerState — Copy, PartialEq, Eq, Hash (Behaviors 29-32)
    // =====================================================================

    #[test]
    fn trigger_state_is_copy_and_zero_sized_heap() {
        let state = TriggerState::Active;
        let copy = state;
        assert_eq!(copy, state);
        assert!(std::mem::size_of::<TriggerState>() <= std::mem::size_of::<u8>());
    }

    #[test]
    fn trigger_state_partial_eq_reflexive() {
        let state = TriggerState::Active;
        assert_eq!(state, state);
    }

    #[rstest::rstest]
    #[case(TriggerState::Active)]
    #[case(TriggerState::Paused)]
    #[case(TriggerState::Disabled)]
    #[case(TriggerState::Error)]
    fn trigger_state_eq_reflexive_for_all_variants(#[case] state: TriggerState) {
        let copy = state;
        assert_eq!(state, copy);
    }

    #[test]
    fn trigger_state_ne_distinguishes_distinct_variants() {
        assert_ne!(TriggerState::Active, TriggerState::Paused);
    }

    #[test]
    fn trigger_state_hash_works_in_hashset() {
        let mut set = HashSet::new();
        set.insert(TriggerState::Active);
        set.insert(TriggerState::Paused);
        set.insert(TriggerState::Active);
        assert_eq!(set.len(), 2);
    }

    // =====================================================================
    // Proptest invariants
    // =====================================================================

    proptest::proptest! {
        /// TriggerState serde roundtrip: serialize then deserialize yields same value.
        #[test]
        fn proptest_trigger_state_serde_roundtrip_preserves_value(
            state in proptest::sample::select(vec![
                TriggerState::Active,
                TriggerState::Paused,
                TriggerState::Disabled,
                TriggerState::Error,
            ])
        ) {
            let json = serde_json::to_string(&state).unwrap();
            let recovered: TriggerState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(state, recovered);
        }

        /// TriggerState FromStr case-insensitivity: mixed-case variant names parse correctly.
        #[test]
        fn proptest_trigger_state_from_str_ignores_case(
            (input, expected) in proptest::sample::select(vec![
                ("active", TriggerState::Active),
                ("ACTIVE", TriggerState::Active),
                ("Active", TriggerState::Active),
                ("aCtIvE", TriggerState::Active),
                ("paused", TriggerState::Paused),
                ("PAUSED", TriggerState::Paused),
                ("Paused", TriggerState::Paused),
                ("pAuSeD", TriggerState::Paused),
                ("disabled", TriggerState::Disabled),
                ("DISABLED", TriggerState::Disabled),
                ("Disabled", TriggerState::Disabled),
                ("dIsAbLeD", TriggerState::Disabled),
                ("error", TriggerState::Error),
                ("ERROR", TriggerState::Error),
                ("Error", TriggerState::Error),
                ("eRrOr", TriggerState::Error),
            ])
        ) {
            let parsed: TriggerState = input.parse().unwrap();
            prop_assert_eq!(parsed, expected);
        }
    }
}
