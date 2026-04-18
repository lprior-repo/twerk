use twerk_core::asl::error_code::ErrorCode;
use twerk_core::asl::retrier::{JitterStrategy, Retrier};
use twerk_core::asl::types::BackoffRate;

#[kani::proof]
fn retrier_valid_construction() {
    let error_equals = vec![ErrorCode::Timeout, ErrorCode::TaskFailed];
    let interval_seconds: u64 = kani::any();
    let max_attempts: u32 = kani::any();
    let backoff_rate: f64 = kani::any();
    let max_delay_seconds: Option<u64> = kani::any();
    let jitter_strategy = JitterStrategy::None;

    if interval_seconds >= 1 && max_attempts >= 1 && backoff_rate > 0.0 && backoff_rate.is_finite()
    {
        if let Some(max_delay) = max_delay_seconds {
            if max_delay > interval_seconds {
                if let Ok(br) = BackoffRate::new(backoff_rate) {
                    let result = Retrier::new(
                        error_equals.clone(),
                        interval_seconds,
                        max_attempts,
                        br,
                        max_delay_seconds,
                        jitter_strategy,
                    );
                    assert!(result.is_ok(), "Valid parameters should create Retrier");
                }
            }
        } else {
            if let Ok(br) = BackoffRate::new(backoff_rate) {
                let result = Retrier::new(
                    error_equals.clone(),
                    interval_seconds,
                    max_attempts,
                    br,
                    max_delay_seconds,
                    jitter_strategy,
                );
                assert!(
                    result.is_ok(),
                    "Valid parameters without max_delay should create Retrier"
                );
            }
        }
    }
}

#[kani::proof]
fn retrier_rejects_empty_error_equals() {
    let interval_seconds = 1;
    let max_attempts = 1u32;
    let backoff_rate = BackoffRate::new(1.0).unwrap();
    let max_delay_seconds: Option<u64> = None;
    let jitter_strategy = JitterStrategy::None;

    let result = Retrier::new(
        vec![],
        interval_seconds,
        max_attempts,
        backoff_rate,
        max_delay_seconds,
        jitter_strategy,
    );

    assert!(result.is_err(), "Empty error_equals should be rejected");
}

#[kani::proof]
fn retrier_rejects_zero_interval() {
    let error_equals = vec![ErrorCode::Timeout];
    let max_attempts = 1u32;
    let backoff_rate = BackoffRate::new(1.0).unwrap();
    let max_delay_seconds: Option<u64> = None;
    let jitter_strategy = JitterStrategy::None;

    let result = Retrier::new(
        error_equals,
        0,
        max_attempts,
        backoff_rate,
        max_delay_seconds,
        jitter_strategy,
    );

    assert!(result.is_err(), "Zero interval should be rejected");
}

#[kani::proof]
fn retrier_rejects_zero_max_attempts() {
    let error_equals = vec![ErrorCode::Timeout];
    let interval_seconds = 1u64;
    let backoff_rate = BackoffRate::new(1.0).unwrap();
    let max_delay_seconds: Option<u64> = None;
    let jitter_strategy = JitterStrategy::None;

    let result = Retrier::new(
        error_equals,
        interval_seconds,
        0,
        backoff_rate,
        max_delay_seconds,
        jitter_strategy,
    );

    assert!(result.is_err(), "Zero max_attempts should be rejected");
}

#[kani::proof]
fn retrier_rejects_max_delay_less_than_interval() {
    let error_equals = vec![ErrorCode::Timeout];
    let interval_seconds = 10u64;
    let max_attempts = 3u32;
    let backoff_rate = BackoffRate::new(1.0).unwrap();
    let max_delay_seconds = Some(5u64);
    let jitter_strategy = JitterStrategy::None;

    let result = Retrier::new(
        error_equals,
        interval_seconds,
        max_attempts,
        backoff_rate,
        max_delay_seconds,
        jitter_strategy,
    );

    assert!(result.is_err(), "max_delay <= interval should be rejected");
}

#[kani::proof]
fn retrier_serialize_deserialize_roundtrip() {
    let error_equals = vec![ErrorCode::Timeout, ErrorCode::TaskFailed];
    let backoff_rate = BackoffRate::new(2.0).unwrap();

    let retrier = Retrier::new(
        error_equals,
        1,
        3,
        backoff_rate,
        Some(60),
        JitterStrategy::Full,
    )
    .unwrap();

    let serialized = serde_json::to_string(&retrier).unwrap();
    let deserialized: Retrier = serde_json::from_str(&serialized).unwrap();

    assert_eq!(retrier.error_equals(), deserialized.error_equals());
    assert_eq!(retrier.interval_seconds(), deserialized.interval_seconds());
    assert_eq!(retrier.max_attempts(), deserialized.max_attempts());
    assert_eq!(retrier.backoff_rate(), deserialized.backoff_rate());
    assert_eq!(
        retrier.max_delay_seconds(),
        deserialized.max_delay_seconds()
    );
    assert_eq!(retrier.jitter_strategy(), deserialized.jitter_strategy());
}
