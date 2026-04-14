//! Comprehensive tests for the types module.
//!
//! Tests validation logic, error paths, and boundary conditions.

use std::f64::NAN;

// ===========================================================================
// Port Tests
// ===========================================================================

mod port_tests {
    use super::*;
    use crate::types::{Port, PortError};

    #[test]
    fn port_new_accepts_valid_u16_when_value_is_in_range_1_to_65535() {
        let port = Port::new(8080).unwrap();
        assert_eq!(port.value(), 8080);
    }

    #[test]
    fn port_new_accepts_min_boundary_one() {
        let port = Port::new(1).unwrap();
        assert_eq!(port.value(), 1);
    }

    #[test]
    fn port_new_accepts_max_boundary_65535() {
        let port = Port::new(65535).unwrap();
        assert_eq!(port.value(), 65535);
    }

    #[test]
    fn port_new_accepts_middle_value_32768() {
        let port = Port::new(32768).unwrap();
        assert_eq!(port.value(), 32768);
    }

    #[test]
    fn port_new_rejects_zero_with_out_of_range_error() {
        let err = Port::new(0).unwrap_err();
        assert!(matches!(
            err,
            PortError::OutOfRange {
                value: 0,
                min: 1,
                max: 65535
            }
        ));
    }

    #[test]
    fn port_new_rejects_65536_with_out_of_range_error() {
        let err = Port::new(65536).unwrap_err();
        assert!(matches!(
            err,
            PortError::OutOfRange {
                value: 65536,
                min: 1,
                max: 65535
            }
        ));
    }

    #[test]
    fn port_new_rejects_far_out_of_range() {
        let err = Port::new(100000).unwrap_err();
        assert!(matches!(
            err,
            PortError::OutOfRange {
                value: 100000,
                min: 1,
                max: 65535
            }
        ));
    }

    #[test]
    fn port_value_accessor_returns_inner_u16() {
        let port = Port::new(443).unwrap();
        assert_eq!(port.value(), 443);
    }

    #[test]
    fn port_deref_yields_u16() {
        let port = Port::new(80).unwrap();
        let dereferenced: u16 = *port;
        assert_eq!(dereferenced, 80);
    }

    #[test]
    fn port_asref_yields_u16_ref() {
        let port = Port::new(80).unwrap();
        let reference: &u16 = port.as_ref();
        assert_eq!(reference, &80);
    }

    #[test]
    fn port_debug_format_contains_type_name() {
        let port = Port::new(22).unwrap();
        let debug = format!("{:?}", port);
        assert!(
            debug.contains("Port("),
            "Debug format should contain 'Port()': got {}",
            debug
        );
    }

    #[test]
    fn port_display_format_shows_raw_value() {
        let port = Port::new(22).unwrap();
        assert_eq!(format!("{}", port), "22");
    }

    #[test]
    fn port_equality_holds_for_same_values() {
        let p1 = Port::new(80).unwrap();
        let p2 = Port::new(80).unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn port_inequality_holds_for_different_values() {
        let p1 = Port::new(80).unwrap();
        let p2 = Port::new(8080).unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn port_error_display_shows_value_and_bounds() {
        let err = Port::new(0).unwrap_err();
        let display = format!("{}", err);
        assert!(
            display.contains("0"),
            "Error display should contain value 0: got {}",
            display
        );
        assert!(
            display.contains("1"),
            "Error display should contain min 1: got {}",
            display
        );
        assert!(
            display.contains("65535"),
            "Error display should contain max 65535: got {}",
            display
        );
    }

    #[test]
    fn port_error_equality() {
        let err1 = Port::new(0).unwrap_err();
        let err2 = Port::new(0).unwrap_err();
        assert_eq!(err1, err2);
    }

    #[test]
    fn port_from_str_parses_valid() {
        let result: Result<Port, _> = "8080".parse();
        let port = result.expect("Port should parse from valid string");
        assert_eq!(port.value(), 8080);
    }

    #[test]
    fn port_from_str_rejects_invalid() {
        let result: Result<Port, _> = "invalid".parse();
        let err = result.unwrap_err();
        assert!(matches!(err, PortError::OutOfRange { .. }));
    }

    #[test]
    fn port_from_str_rejects_zero() {
        let result: Result<Port, _> = "0".parse();
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            PortError::OutOfRange {
                value: 0,
                min: 1,
                max: 65535
            }
        ));
    }

    #[test]
    fn port_from_str_rejects_out_of_range() {
        let result: Result<Port, _> = "65536".parse();
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            PortError::OutOfRange {
                value: 65536,
                min: 1,
                max: 65535
            }
        ));
    }
}

// ===========================================================================
// RetryLimit Tests
// ===========================================================================

mod retry_limit_tests {
    use super::*;
    use crate::types::{RetryLimit, RetryLimitError};

    #[test]
    fn retry_limit_new_accepts_zero() {
        let rl = RetryLimit::new(0).unwrap();
        assert_eq!(rl.value(), 0);
    }

    #[test]
    fn retry_limit_new_accepts_mid_value() {
        let rl = RetryLimit::new(5).unwrap();
        assert_eq!(rl.value(), 5);
    }

    #[test]
    fn retry_limit_new_accepts_u32_max() {
        let rl = RetryLimit::new(u32::MAX).unwrap();
        assert_eq!(rl.value(), u32::MAX);
    }

    #[test]
    fn retry_limit_from_option_accepts_some_value() {
        let rl = RetryLimit::from_option(Some(3)).unwrap();
        assert_eq!(rl.value(), 3);
    }

    #[test]
    fn retry_limit_from_option_rejects_none() {
        let err = RetryLimit::from_option(None).unwrap_err();
        assert!(matches!(err, RetryLimitError::NoneNotAllowed));
    }

    #[test]
    fn retry_limit_value_accessor_returns_inner_u32() {
        let rl = RetryLimit::new(5).unwrap();
        assert_eq!(rl.value(), 5);
    }

    #[test]
    fn retry_limit_deref_yields_u32() {
        let rl = RetryLimit::new(10).unwrap();
        let dereferenced: u32 = *rl;
        assert_eq!(dereferenced, 10);
    }

    #[test]
    fn retry_limit_asref_yields_u32_ref() {
        let rl = RetryLimit::new(10).unwrap();
        let reference: &u32 = rl.as_ref();
        assert_eq!(reference, &10);
    }

    #[test]
    fn retry_limit_display_shows_raw_value() {
        let rl = RetryLimit::new(7).unwrap();
        assert_eq!(format!("{}", rl), "7");
    }

    #[test]
    fn retry_limit_equality_holds_for_same_values() {
        let r1 = RetryLimit::new(5).unwrap();
        let r2 = RetryLimit::new(5).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn retry_limit_error_display_shows_message() {
        let err = RetryLimit::from_option(None).unwrap_err();
        assert_eq!(format!("{}", err), "Optional retry limit must be present");
    }

    #[test]
    fn retry_limit_error_equality() {
        let err1 = RetryLimit::from_option(None).unwrap_err();
        let err2 = RetryLimit::from_option(None).unwrap_err();
        assert_eq!(err1, err2);
    }
}

// ===========================================================================
// RetryAttempt Tests
// ===========================================================================

mod retry_attempt_tests {
    use super::*;
    use crate::types::RetryAttempt;

    #[test]
    fn retry_attempt_new_accepts_zero() {
        let ra = RetryAttempt::new(0).unwrap();
        assert_eq!(ra.value(), 0);
    }

    #[test]
    fn retry_attempt_new_accepts_mid_range() {
        let ra = RetryAttempt::new(2147483647).unwrap();
        assert_eq!(ra.value(), 2147483647);
    }

    #[test]
    fn retry_attempt_new_accepts_u32_max() {
        let ra = RetryAttempt::new(u32::MAX).unwrap();
        assert_eq!(ra.value(), u32::MAX);
    }

    #[test]
    fn retry_attempt_value_accessor_returns_inner_u32() {
        let ra = RetryAttempt::new(1).unwrap();
        assert_eq!(ra.value(), 1);
    }

    #[test]
    fn retry_attempt_deref_yields_u32() {
        let ra = RetryAttempt::new(1).unwrap();
        let dereferenced: u32 = *ra;
        assert_eq!(dereferenced, 1);
    }

    #[test]
    fn retry_attempt_asref_yields_u32_ref() {
        let ra = RetryAttempt::new(1).unwrap();
        let reference: &u32 = ra.as_ref();
        assert_eq!(reference, &1);
    }

    #[test]
    fn retry_attempt_display_shows_raw_value() {
        let ra = RetryAttempt::new(4).unwrap();
        assert_eq!(format!("{}", ra), "4");
    }

    #[test]
    fn retry_attempt_equality_holds_for_same_values() {
        let a1 = RetryAttempt::new(2).unwrap();
        let a2 = RetryAttempt::new(2).unwrap();
        assert_eq!(a1, a2);
    }
}

// ===========================================================================
// Progress Tests
// ===========================================================================

mod progress_tests {
    use super::*;
    use crate::types::{Progress, ProgressError};

    #[test]
    fn progress_new_accepts_zero() {
        let p = Progress::new(0.0).unwrap();
        assert_eq!(p.value(), 0.0);
    }

    #[test]
    fn progress_new_accepts_50_percent() {
        let p = Progress::new(50.0).unwrap();
        assert_eq!(p.value(), 50.0);
    }

    #[test]
    fn progress_new_accepts_100_percent() {
        let p = Progress::new(100.0).unwrap();
        assert_eq!(p.value(), 100.0);
    }

    #[test]
    fn progress_new_accepts_subnormal_positive() {
        let p = Progress::new(0.0000001).unwrap();
        assert_eq!(p.value(), 0.0000001);
    }

    #[test]
    fn progress_new_accepts_value_near_max() {
        let p = Progress::new(99.9999).unwrap();
        assert_eq!(p.value(), 99.9999);
    }

    #[test]
    fn progress_new_rejects_negative_with_out_of_range_error() {
        let err = Progress::new(-0.001).unwrap_err();
        assert!(matches!(
            err,
            ProgressError::OutOfRange {
                value,
                min: 0.0,
                max: 100.0
            } if value < 0.0
        ));
    }

    #[test]
    fn progress_new_rejects_over_100_with_out_of_range_error() {
        let err = Progress::new(100.001).unwrap_err();
        assert!(matches!(
            err,
            ProgressError::OutOfRange {
                value,
                min: 0.0,
                max: 100.0
            } if value > 100.0
        ));
    }

    #[test]
    fn progress_new_rejects_nan() {
        let err = Progress::new(NAN).unwrap_err();
        assert!(matches!(err, ProgressError::NaN));
    }

    #[test]
    fn progress_new_rejects_negative_infinity() {
        let err = Progress::new(f64::NEG_INFINITY).unwrap_err();
        assert!(matches!(err, ProgressError::OutOfRange { .. }));
    }

    #[test]
    fn progress_new_rejects_positive_infinity() {
        let err = Progress::new(f64::INFINITY).unwrap_err();
        assert!(matches!(err, ProgressError::OutOfRange { .. }));
    }

    #[test]
    fn progress_value_accessor_returns_inner_f64() {
        let p = Progress::new(75.5).unwrap();
        assert_eq!(p.value(), 75.5);
    }

    #[test]
    fn progress_deref_yields_f64() {
        let p = Progress::new(33.3).unwrap();
        let dereferenced: f64 = *p;
        assert_eq!(dereferenced, 33.3);
    }

    #[test]
    fn progress_asref_yields_f64_ref() {
        let p = Progress::new(33.3).unwrap();
        let reference: &f64 = p.as_ref();
        assert_eq!(reference, &33.3);
    }

    #[test]
    fn progress_debug_format_contains_type_name() {
        let p = Progress::new(50.0).unwrap();
        let debug = format!("{:?}", p);
        assert!(
            debug.contains("Progress("),
            "Debug format should contain 'Progress()': got {}",
            debug
        );
    }

    #[test]
    fn progress_display_shows_raw_value() {
        let p = Progress::new(62.5).unwrap();
        assert_eq!(format!("{}", p), "62.5");
    }

    #[test]
    fn progress_equality_holds_for_same_values() {
        let p1 = Progress::new(50.0).unwrap();
        let p2 = Progress::new(50.0).unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn progress_error_out_of_range_display_for_negative() {
        let err = Progress::new(-0.001).unwrap_err();
        let display = format!("{}", err);
        assert!(
            display.contains("-0.001"),
            "Error display should contain value: got {}",
            display
        );
        assert!(
            display.contains("0.0"),
            "Error display should contain min: got {}",
            display
        );
        assert!(
            display.contains("100.0"),
            "Error display should contain max: got {}",
            display
        );
    }

    #[test]
    fn progress_error_out_of_range_display_for_over_100() {
        let err = Progress::new(100.001).unwrap_err();
        let display = format!("{}", err);
        assert!(
            display.contains("100.001"),
            "Error display should contain value: got {}",
            display
        );
    }

    #[test]
    fn progress_error_nan_display_contains_nan() {
        let err = Progress::new(NAN).unwrap_err();
        let display = format!("{}", err);
        assert!(
            display.contains("NaN"),
            "Error display should contain 'NaN': got {}",
            display
        );
    }

    #[test]
    fn progress_error_out_of_range_equality() {
        let err1 = Progress::new(-0.001).unwrap_err();
        let err2 = Progress::new(-0.001).unwrap_err();
        assert_eq!(err1, err2);
    }

    #[test]
    fn progress_error_nan_equality() {
        let err1 = Progress::new(NAN).unwrap_err();
        let err2 = Progress::new(NAN).unwrap_err();
        // The error stores the NaN value, so they should be equal
        assert_eq!(err1, err2);
    }
}

// ===========================================================================
// TaskCount Tests
// ===========================================================================

mod task_count_tests {
    use super::*;
    use crate::types::{TaskCount, TaskCountError};

    #[test]
    fn task_count_new_accepts_zero() {
        let tc = TaskCount::new(0).unwrap();
        assert_eq!(tc.value(), 0);
    }

    #[test]
    fn task_count_new_accepts_mid_value() {
        let tc = TaskCount::new(100).unwrap();
        assert_eq!(tc.value(), 100);
    }

    #[test]
    fn task_count_new_accepts_u32_max() {
        let tc = TaskCount::new(u32::MAX).unwrap();
        assert_eq!(tc.value(), u32::MAX);
    }

    #[test]
    fn task_count_from_option_accepts_some_value() {
        let tc = TaskCount::from_option(Some(10)).unwrap();
        assert_eq!(tc.value(), 10);
    }

    #[test]
    fn task_count_from_option_rejects_none() {
        let err = TaskCount::from_option(None).unwrap_err();
        assert!(matches!(err, TaskCountError::NoneNotAllowed));
    }

    #[test]
    fn task_count_value_accessor_returns_inner_u32() {
        let tc = TaskCount::new(7).unwrap();
        assert_eq!(tc.value(), 7);
    }

    #[test]
    fn task_count_deref_yields_u32() {
        let tc = TaskCount::new(7).unwrap();
        let dereferenced: u32 = *tc;
        assert_eq!(dereferenced, 7);
    }

    #[test]
    fn task_count_asref_yields_u32_ref() {
        let tc = TaskCount::new(7).unwrap();
        let reference: &u32 = tc.as_ref();
        assert_eq!(reference, &7);
    }

    #[test]
    fn task_count_display_shows_raw_value() {
        let tc = TaskCount::new(42).unwrap();
        assert_eq!(format!("{}", tc), "42");
    }

    #[test]
    fn task_count_equality_holds_for_same_values() {
        let c1 = TaskCount::new(7).unwrap();
        let c2 = TaskCount::new(7).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn task_count_error_display_shows_message() {
        let err = TaskCount::from_option(None).unwrap_err();
        assert_eq!(format!("{}", err), "Optional task count must be present");
    }

    #[test]
    fn task_count_error_equality() {
        let err1 = TaskCount::from_option(None).unwrap_err();
        let err2 = TaskCount::from_option(None).unwrap_err();
        assert_eq!(err1, err2);
    }
}

// ===========================================================================
// TaskPosition Tests
// ===========================================================================

mod task_position_tests {
    use super::*;
    use crate::types::TaskPosition;

    #[test]
    fn task_position_new_accepts_zero() {
        let tp = TaskPosition::new(0).unwrap();
        assert_eq!(tp.value(), 0);
    }

    #[test]
    fn task_position_new_accepts_positive_value() {
        let tp = TaskPosition::new(99).unwrap();
        assert_eq!(tp.value(), 99);
    }

    #[test]
    fn task_position_new_accepts_negative_one() {
        let tp = TaskPosition::new(-1).unwrap();
        assert_eq!(tp.value(), -1);
    }

    #[test]
    fn task_position_new_accepts_negative_value() {
        let tp = TaskPosition::new(-5).unwrap();
        assert_eq!(tp.value(), -5);
    }

    #[test]
    fn task_position_new_accepts_i64_min() {
        let tp = TaskPosition::new(i64::MIN).unwrap();
        assert_eq!(tp.value(), i64::MIN);
    }

    #[test]
    fn task_position_new_accepts_i64_max() {
        let tp = TaskPosition::new(i64::MAX).unwrap();
        assert_eq!(tp.value(), i64::MAX);
    }

    #[test]
    fn task_position_value_accessor_returns_inner_i64() {
        let tp = TaskPosition::new(99).unwrap();
        assert_eq!(tp.value(), 99);
    }

    #[test]
    fn task_position_deref_yields_i64() {
        let tp = TaskPosition::new(99).unwrap();
        let dereferenced: i64 = *tp;
        assert_eq!(dereferenced, 99);
    }

    #[test]
    fn task_position_asref_yields_i64_ref() {
        let tp = TaskPosition::new(99).unwrap();
        let reference: &i64 = tp.as_ref();
        assert_eq!(reference, &99);
    }

    #[test]
    fn task_position_display_shows_raw_i64() {
        let tp = TaskPosition::new(-123).unwrap();
        assert_eq!(format!("{}", tp), "-123");
    }

    #[test]
    fn task_position_debug_format_contains_type_name() {
        let tp = TaskPosition::new(5).unwrap();
        let debug = format!("{:?}", tp);
        assert!(
            debug.contains("TaskPosition("),
            "Debug format should contain 'TaskPosition()': got {}",
            debug
        );
    }

    #[test]
    fn task_position_equality_holds_for_same_values() {
        let p1 = TaskPosition::new(42).unwrap();
        let p2 = TaskPosition::new(42).unwrap();
        assert_eq!(p1, p2);
    }
}
