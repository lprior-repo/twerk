//! Helper functions for `RabbitMQ` broker.

use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use crate::broker::BoxedHandlerFuture;

/// Type alias for the JSON message handler used in subscriptions.
pub(crate) type JsonHandler = Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>;

/// Creates a formatted `RabbitMQ` connection error message.
#[inline]
pub(crate) fn rabbitmq_conn_err(conn_idx: usize, e: &impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("RabbitMQ connection {conn_idx} failed: {e}")
}

/// Creates a typed JSON subscription handler that deserializes JSON and invokes the handler.
///
/// This eliminates the repeated `Arc::new(move |val| { ... Box::pin(async move {...}) })`
/// pattern across all `subscribe_for_*` methods.
pub(crate) fn make_json_handler<T>(
    handler: Arc<dyn Fn(T) -> BoxedHandlerFuture + Send + Sync>,
) -> JsonHandler
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    Arc::new(move |val: Value| {
        let handler = handler.clone();
        Box::pin(async move {
            if let Ok(msg) = serde_json::from_value::<T>(val) {
                handler(msg).await?;
            }
            Ok(())
        })
    })
}

/// Creates a typed JSON subscription handler for types wrapped in Arc.
pub(crate) fn make_json_handler_arc<T>(
    handler: Arc<dyn Fn(Arc<T>) -> BoxedHandlerFuture + Send + Sync>,
) -> JsonHandler
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    Arc::new(move |val: Value| {
        let handler = handler.clone();
        Box::pin(async move {
            if let Ok(msg) = serde_json::from_value::<T>(val) {
                handler(Arc::new(msg)).await?;
            }
            Ok(())
        })
    })
}

/// Extracts an i64 from JSON, returning 0 for null/missing values.
#[inline]
pub(crate) fn extract_i64(val: &Value) -> i64 {
    val.as_i64().map_or(0, |v| v)
}

/// Safely converts i64 to i32, clamping to `i32::MAX`/`i32::MIN` on overflow.
#[inline]
pub(crate) fn clamp_i32(val: i64) -> i32 {
    i32::try_from(val).unwrap_or_else(|_| {
        if val > 0 {
            debug!(
                value = val,
                "i64 overflow on i32 conversion, clamping to MAX"
            );
            i32::MAX
        } else {
            debug!(
                value = val,
                "i64 underflow on i32 conversion, clamping to MIN"
            );
            i32::MIN
        }
    })
}

/// Extracts i32 from JSON, returning 0 for null/missing values.
#[inline]
pub(crate) fn extract_i32(val: &Value) -> i32 {
    clamp_i32(extract_i64(val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_i64_returns_zero_for_null() {
        assert_eq!(extract_i64(&Value::Null), 0);
    }

    #[test]
    fn extract_i64_returns_zero_for_missing_field() {
        let obj = json!({"other": 42});
        assert_eq!(extract_i64(&obj["missing"]), 0);
    }

    #[test]
    fn extract_i64_returns_value_for_integer() {
        assert_eq!(extract_i64(&json!(42)), 42);
    }

    #[test]
    fn extract_i64_returns_zero_for_string() {
        assert_eq!(extract_i64(&json!("not a number")), 0);
    }

    #[test]
    fn extract_i64_returns_negative_value() {
        assert_eq!(extract_i64(&json!(-100)), -100);
    }

    #[test]
    fn clamp_i32_returns_value_within_range() {
        assert_eq!(clamp_i32(42), 42);
    }

    #[test]
    fn clamp_i32_clamps_positive_overflow() {
        assert_eq!(clamp_i32(i64::from(i32::MAX) + 1), i32::MAX);
    }

    #[test]
    fn clamp_i32_clamps_negative_underflow() {
        assert_eq!(clamp_i32(i64::from(i32::MIN) - 1), i32::MIN);
    }

    #[test]
    fn clamp_i32_handles_max_i32() {
        assert_eq!(clamp_i32(i64::from(i32::MAX)), i32::MAX);
    }

    #[test]
    fn clamp_i32_handles_min_i32() {
        assert_eq!(clamp_i32(i64::from(i32::MIN)), i32::MIN);
    }

    #[test]
    fn clamp_i32_handles_zero() {
        assert_eq!(clamp_i32(0), 0);
    }

    #[test]
    fn extract_i32_returns_zero_for_null() {
        assert_eq!(extract_i32(&Value::Null), 0);
    }

    #[test]
    fn extract_i32_returns_clamped_large_value() {
        assert_eq!(extract_i32(&json!(i64::MAX)), i32::MAX);
    }

    #[test]
    fn rabbitmq_conn_err_formats_message() {
        let err = rabbitmq_conn_err(3, &"connection refused");
        assert!(err.to_string().contains("RabbitMQ connection 3"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn rabbitmq_conn_err_formats_with_index() {
        let err = rabbitmq_conn_err(1, &"timeout");
        assert!(err.to_string().contains("connection 1"));
    }

    #[test]
    fn make_json_handler_deserializes_and_invokes() {
        #[derive(serde::Deserialize, Debug)]
        struct Msg {
            val: i64,
        }
        let invoked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let invoked_clone = invoked.clone();
        let handler: JsonHandler = make_json_handler(Arc::new(move |msg: Msg| {
            invoked_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(msg.val, 99);
            Box::pin(async { Ok(()) })
        }));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(handler(json!({"val": 99}))).expect("handler");
        assert!(invoked.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn make_json_handler_ignores_invalid_json() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        let handler: JsonHandler = make_json_handler::<String>(Arc::new(move |_msg| {
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(handler(json!(42)))
            .expect("handler should succeed");
        assert!(
            !called.load(std::sync::atomic::Ordering::SeqCst),
            "handler should not be called for non-string"
        );
    }

    #[test]
    fn make_json_handler_arc_wraps_in_arc() {
        #[derive(serde::Deserialize, Debug)]
        struct Payload {
            x: i32,
        }
        let captured = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));
        let captured_clone = captured.clone();
        let handler: JsonHandler =
            make_json_handler_arc(Arc::new(move |msg: std::sync::Arc<Payload>| {
                captured_clone.store(msg.x, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async { Ok(()) })
            }));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(handler(json!({"x": 7}))).expect("handler");
        assert_eq!(captured.load(std::sync::atomic::Ordering::SeqCst), 7);
    }
}
