//! API context module
//!
//! Provides context types for HTTP request handling.

use axum::http::StatusCode;
use serde_json::json;

/// Context for API requests.
///
/// This wraps the incoming request and provides helper methods
/// for extracting data and setting responses.
#[derive(Debug, Clone)]
pub struct Context {
    /// Error message if any
    pub error: Option<String>,
    /// HTTP status code to return
    pub code: StatusCode,
}

impl Context {
    /// Create a new context with OK status.
    pub fn ok() -> Self {
        Self {
            error: None,
            code: StatusCode::OK,
        }
    }

    /// Create a new context with an error.
    pub fn error(code: StatusCode, message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            code,
        }
    }

    /// Set an error on this context.
    pub fn set_error(&mut self, code: StatusCode, message: impl Into<String>) {
        self.code = code;
        self.error = Some(message.into());
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::ok()
    }
}

#[allow(clippy::unwrap_used)]
impl From<Context> for axum::response::Response<axum::body::Body> {
    fn from(ctx: Context) -> Self {
        match ctx.error {
            Some(err) => {
                let body = json!({ "message": err }).to_string();
                axum::response::Response::builder()
                    .status(ctx.code)
                    .body(axum::body::Body::from(body))
                    .unwrap_or_else(|_| {
                        // Fallback: just use an empty body with the status
                        axum::response::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(axum::body::Body::empty())
                            .expect("Response::builder should always produce a valid response")
                    })
            }
            None => axum::response::Response::builder()
                .status(ctx.code)
                .body(axum::body::Body::empty())
                .unwrap_or_else(|_| {
                    // This should never fail since we're building with OK status and empty body
                    axum::response::Response::builder()
                        .status(StatusCode::OK)
                        .body(axum::body::Body::empty())
                        .expect("Response::builder should always produce a valid response")
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_ok() {
        let ctx = Context::ok();
        assert!(ctx.error.is_none());
        assert_eq!(ctx.code, StatusCode::OK);
    }

    #[test]
    fn test_context_error() {
        let ctx = Context::error(StatusCode::NOT_FOUND, "not found");
        assert_eq!(ctx.error.as_deref(), Some("not found"));
        assert_eq!(ctx.code, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_context_set_error() {
        let mut ctx = Context::ok();
        ctx.set_error(StatusCode::BAD_REQUEST, "bad request");
        assert_eq!(ctx.error.as_deref(), Some("bad request"));
        assert_eq!(ctx.code, StatusCode::BAD_REQUEST);
    }
}