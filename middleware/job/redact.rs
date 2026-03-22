//! Job redaction middleware.
//!
//! Redacts sensitive information from jobs when they are read through the API.
//! Parity with Go's `middleware/job/redact.go`.
//!
//! Uses the production `crate::redact::Redacter` which provides pure-functional
//! redaction (returns new `Job` rather than mutating). The middleware adapts
//! this to the `&mut Job` handler pattern via reassignment.

use std::sync::Arc;

use tork::job::Job;

use crate::middleware::job::{Context, EventType, HandlerFunc, MiddlewareFunc};
use crate::redact::Redacter;

/// Create a redaction middleware.
///
/// Parity with Go `func Redact(redacter *redact.Redacter) MiddlewareFunc`:
/// - Only redacts on `Read` events
/// - Passes through all other events untouched
///
/// The production `Redacter::redact_job` returns a new `Job` (pure function),
/// so we reassign `*job` with the redacted version.
pub fn redact_middleware(redacter: Arc<Redacter>) -> MiddlewareFunc {
    Arc::new(move |next: HandlerFunc| {
        let redacter = redacter.clone();
        Arc::new(move |ctx: Arc<Context>, et: EventType, job: &mut Job| {
            if et == EventType::Read {
                let redacted = redacter.redact_job(job);
                *job = redacted;
            }
            next(ctx, et, job)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::job::{apply_middleware, noop_handler};
    use std::collections::HashMap;

    #[test]
    fn test_redact_on_read() {
        let redacter = Arc::new(Redacter::default());
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(HashMap::from([(
                "api_secret_key".to_string(),
                "my-password".to_string(),
            )])),
            secrets: Some(HashMap::from([(
                "s1".to_string(),
                "my-password".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::Read, &mut job).unwrap();

        // Key "api_secret_key" matches SECRET matcher → [REDACTED]
        assert_eq!(
            job.inputs
                .as_ref()
                .and_then(|m| m.get("api_secret_key").cloned()),
            Some("[REDACTED]".to_string())
        );
        // Secrets themselves get blanked
        assert_eq!(
            job.secrets.as_ref().and_then(|m| m.get("s1").cloned()),
            Some("[REDACTED]".to_string())
        );
    }

    #[test]
    fn test_no_redact_on_state_change() {
        let redacter = Arc::new(Redacter::default());
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let original_value = "my-password".to_string();
        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(HashMap::from([(
                "api_secret_key".to_string(),
                original_value.clone(),
            )])),
            secrets: Some(HashMap::from([(
                "s1".to_string(),
                "my-password".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::StateChange, &mut job).unwrap();

        // StateChange should not trigger redaction
        assert_eq!(
            job.inputs
                .as_ref()
                .and_then(|m| m.get("api_secret_key").cloned()),
            Some(original_value)
        );
    }

    #[test]
    fn test_redact_secret_value_in_inputs() {
        // When the key doesn't match a matcher but contains a secret value
        let redacter = Arc::new(Redacter::new(vec![])); // No key matchers
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(HashMap::from([(
                "connection_string".to_string(),
                "postgres://user:supersecret@host".to_string(),
            )])),
            secrets: Some(HashMap::from([(
                "db_password".to_string(),
                "supersecret".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::Read, &mut job).unwrap();

        // Secret value "supersecret" should be replaced in the input
        assert_eq!(
            job.inputs
                .as_ref()
                .and_then(|m| m.get("connection_string").cloned()),
            Some("postgres://user:[REDACTED]@host".to_string())
        );
    }

    #[test]
    fn test_redact_webhook_headers() {
        let redacter = Arc::new(Redacter::default());
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            secrets: Some(HashMap::from([("s1".to_string(), "token123".to_string())])),
            webhooks: Some(vec![tork::task::Webhook {
                url: Some("http://example.com/hook".to_string()),
                headers: Some(HashMap::from([(
                    "X-Secret-Header".to_string(),
                    "Bearer token123".to_string(),
                )])),
                event: None,
                r#if: None,
            }]),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::Read, &mut job).unwrap();

        // Header key contains "SECRET" → redacted
        let headers = job
            .webhooks
            .as_ref()
            .and_then(|ws| ws.first())
            .and_then(|w| w.headers.as_ref());
        assert_eq!(
            headers.and_then(|h| h.get("X-Secret-Header").cloned()),
            Some("[REDACTED]".to_string())
        );
    }

    #[test]
    fn test_redact_job_no_secrets() {
        let redacter = Arc::new(Redacter::default());
        let mw = redact_middleware(redacter);
        let hm = apply_middleware(noop_handler(), vec![mw]);

        let mut job = Job {
            id: Some("test-id".to_string()),
            inputs: Some(HashMap::from([(
                "url".to_string(),
                "http://api.com".to_string(),
            )])),
            ..Default::default()
        };

        let ctx = Arc::new(Context::new());
        hm(ctx, EventType::Read, &mut job).unwrap();

        // No secrets → nothing should be redacted
        assert_eq!(
            job.inputs.as_ref().and_then(|m| m.get("url").cloned()),
            Some("http://api.com".to_string())
        );
    }
}
