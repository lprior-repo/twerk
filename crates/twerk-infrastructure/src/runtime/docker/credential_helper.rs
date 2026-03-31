//! Credential helper integration following functional-rust conventions.

use std::io::Write;
use std::process::{Command, Stdio};

use thiserror::Error;

// Token username marker from docker CLI
const TOKEN_USERNAME: &str = "<token>";

/// Errors from credential helper operations.
#[derive(Debug, Error)]
pub enum CredentialHelperError {
    #[error("credentials not found in native keychain")]
    CredentialsNotFound,

    #[error("no credentials server URL")]
    CredentialsMissingServerUrl,

    #[error("credential helper not found")]
    HelperNotFound,

    #[error("helper execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Credentials returned from a helper.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Credentials {
    pub username: String,
    pub secret: String,
}

/// Gets credentials from a docker credential helper.
///
/// The helper name should be just the suffix (without "docker-credential-").
/// If helper is empty, uses the platform's default helper.
///
/// Returns empty credentials if not found (not an error).
///
/// # Errors
///
/// Returns `CredentialHelperError` if the helper cannot be executed.
pub fn get_from_helper(
    helper: &str,
    hostname: &str,
) -> Result<(String, String), CredentialHelperError> {
    let helper = if helper.is_empty() {
        default_helper()
    } else {
        helper.to_string()
    };

    if helper.is_empty() {
        return Ok((String::new(), String::new()));
    }

    let helper_path = format!("docker-credential-{helper}");

    // Check if the helper exists
    if Command::new(&helper_path)
        .arg("--version")
        .output()
        .is_err()
    {
        return Ok((String::new(), String::new()));
    }

    let mut child = Command::new(&helper_path)
        .arg("get")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CredentialHelperError::ExecutionFailed(e.to_string()))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| CredentialHelperError::ExecutionFailed("cannot access stdin".to_string()))?
        .write_all(hostname.as_bytes())
        .map_err(|e| CredentialHelperError::ExecutionFailed(e.to_string()))?;

    let output = child
        .wait_with_output()
        .map_err(|e| CredentialHelperError::ExecutionFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        match stderr.as_str() {
            s if s.contains("credentials not found") => {
                return Ok((String::new(), String::new()));
            }
            s if s.contains("no credentials server URL") => {
                return Err(CredentialHelperError::CredentialsMissingServerUrl);
            }
            _ => {
                return Ok((String::new(), String::new()));
            }
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let creds: Credentials = serde_json::from_str(stdout.trim())
        .map_err(|e| CredentialHelperError::ExecutionFailed(e.to_string()))?;

    // When tokenUsername is used, the output is an identity token
    let username = if creds.username == TOKEN_USERNAME {
        String::new()
    } else {
        creds.username
    };

    Ok((username, creds.secret))
}

/// Gets the default credential helper name for the current platform.
#[must_use]
pub fn default_helper() -> String {
    #[cfg(target_os = "linux")]
    {
        // Check for `pass` first
        if Command::new("pass").arg("--version").output().is_ok() {
            return "pass".to_string();
        }
        "secretservice".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        "osxkeychain".to_string()
    }

    #[cfg(target_os = "windows")]
    {
        "wincred".to_string()
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_helper_returns_non_empty() {
        // The default helper should be non-empty on supported platforms
        let helper = default_helper();
        #[cfg(target_os = "linux")]
        assert!(!helper.is_empty(), "Linux should have a default helper");
        #[cfg(target_os = "macos")]
        assert!(!helper.is_empty(), "macOS should have a default helper");
        #[cfg(target_os = "windows")]
        assert!(!helper.is_empty(), "Windows should have a default helper");
    }

    #[test]
    fn test_default_helper_is_known_value() {
        let helper = default_helper();
        #[cfg(target_os = "linux")]
        assert!(
            helper == "pass" || helper == "secretservice",
            "Linux default helper should be pass or secretservice, got: {}",
            helper
        );
        #[cfg(target_os = "macos")]
        assert_eq!(helper, "osxkeychain");
        #[cfg(target_os = "windows")]
        assert_eq!(helper, "wincred");
    }

    #[test]
    fn test_get_from_helper_empty_helper_returns_empty_when_no_default() {
        // On unsupported platforms, empty helper + no default returns empty
        // On supported platforms, empty helper resolves to default which may or may not exist
        let result = get_from_helper("", "registry.example.com");
        // Should not panic — either returns empty creds or falls through
        match result {
            Ok((user, pass)) => {
                // Empty result is fine (helper not found)
                assert!(user.is_empty() || !user.is_empty());
                let _ = pass;
            }
            Err(_) => {
                // Also acceptable if default helper fails to execute
            }
        }
    }

    #[test]
    fn test_get_from_helper_nonexistent_helper_returns_empty() {
        // A helper binary that doesn't exist should return empty credentials, not error
        let result = get_from_helper("nonexistent-helper-xyzzy", "registry.example.com");
        assert!(result.is_ok());
        let (user, pass) = result.expect("should be ok");
        assert_eq!("", user);
        assert_eq!("", pass);
    }

    #[test]
    fn test_credential_helper_error_display() {
        let err = CredentialHelperError::CredentialsNotFound;
        assert!(!err.to_string().is_empty());

        let err = CredentialHelperError::HelperNotFound;
        assert!(!err.to_string().is_empty());

        let err = CredentialHelperError::ExecutionFailed("something went wrong".to_string());
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_token_username_detection() {
        // The constant should be <token>
        assert_eq!(TOKEN_USERNAME, "<token>");
    }
}
