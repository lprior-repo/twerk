//! Self-reexecution support for busybox-style init handlers.
//!
//! Handlers can be registered with a name and the `argv\[0\]` of the exec
//! of the binary will be used to find and execute custom init paths.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{LazyLock, Mutex};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Type alias for the initializer function type.
type Initializer = fn();

/// Global registry of registered initializers.
#[allow(clippy::type_complexity)]
static REGISTERED_INITIALIZERS: LazyLock<Mutex<HashMap<String, Initializer>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registers an initialization function under the specified name.
///
/// If a handler is already registered under the given name, an error is returned.
///
/// # Errors
///
/// Returns [`ReexecError::LockError`] when the registry lock is poisoned and
/// [`ReexecError::AlreadyRegistered`] when a handler already exists for `name`.
#[allow(non_snake_case)]
pub fn Register(name: &str, initializer: Initializer) -> Result<(), ReexecError> {
    let mut initializers = REGISTERED_INITIALIZERS
        .lock()
        .map_err(|e| ReexecError::LockError(e.to_string()))?;

    if initializers.contains_key(name) {
        return Err(ReexecError::AlreadyRegistered(name.to_string()));
    }

    initializers.insert(name.to_string(), initializer);
    Ok(())
}

/// Init is called as the first part of the exec process and returns true if an
/// initialization function was called.
#[allow(non_snake_case)]
pub fn Init() -> bool {
    let args: Vec<String> = env::args().collect();
    let argv0 = args.first().map_or("", String::as_str);

    let Ok(initializers) = REGISTERED_INITIALIZERS.lock() else {
        return false;
    };

    if let Some(&initializer) = initializers.get(argv0) {
        initializer();
        return true;
    }

    false
}

/// Returns the path to the current process's binary using `os.Args[0]`.
pub fn naive_self() -> String {
    let argv0 = env::args_os().next().map(PathBuf::from).unwrap_or_default();

    // If only the base name is given (no path separator), look it up in PATH
    if argv0.file_name() == Some(argv0.as_ref()) {
        if let Ok(lp) = env::current_exe() {
            return lp.to_string_lossy().to_string();
        }
    }

    // Handle conversion of relative paths to absolute
    if let Ok(abs_name) = argv0.canonicalize() {
        return abs_name.to_string_lossy().to_string();
    }

    // Fall back to the original name if we couldn't resolve it
    argv0.to_string_lossy().to_string()
}

/// Returns the path to the current process's binary.
/// Uses `/proc/self/exe` on Linux, falls back to `naive_self()` elsewhere.
#[cfg(target_os = "linux")]
#[must_use]
pub fn self_path() -> String {
    "/proc/self/exe".to_string()
}

/// Returns the path to the current process's binary.
/// Uses `naive_self()` on non-Linux Unix systems.
#[cfg(all(unix, not(target_os = "linux")))]
#[must_use]
pub fn self_path() -> String {
    naive_self()
}

/// Returns an empty string on unsupported platforms.
#[cfg(not(unix))]
#[must_use]
pub fn self_path() -> String {
    String::new()
}

/// Creates a `Command` that points to the current process's binary.
/// On Linux, creates a new process group for process cleanup safety.
#[cfg(target_os = "linux")]
#[must_use]
pub fn command(args: &[String]) -> Command {
    let mut cmd = Command::new(self_path());
    cmd.args(args);
    cmd.process_group(0);
    cmd
}

/// Creates a `Command` that points to the current process's binary.
/// On non-Linux Unix systems, no special handling is applied.
#[cfg(all(unix, not(target_os = "linux")))]
#[must_use]
pub fn command(args: &[String]) -> Command {
    let mut cmd = Command::new(self_path());
    cmd.args(args);
    cmd
}

/// Returns `None` on unsupported platforms.
#[cfg(not(unix))]
pub fn command(_args: &[String]) -> Option<Command> {
    None
}

/// Errors that can occur during reexec operations.
#[derive(Debug, thiserror::Error)]
pub enum ReexecError {
    #[error("handler already registered under name: {0}")]
    AlreadyRegistered(String),

    #[error("failed to acquire lock: {0}")]
    LockError(String),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::redundant_pattern_matching)]
    use super::*;

    #[test]
    fn test_register_and_init_not_matched() {
        // Register a handler
        let result = Register("test-handler", || {});
        assert!(matches!(result, Ok(_)));

        // Init should return false since argv[0] doesn't match
        assert!(!Init());
    }

    #[test]
    fn test_naive_self_returns_path() {
        let path = naive_self();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_self_path_returns_path() {
        let path = self_path();
        #[cfg(target_os = "linux")]
        assert_eq!(path, "/proc/self/exe");

        #[cfg(all(unix, not(target_os = "linux")))]
        assert!(!path.is_empty());

        #[cfg(not(unix))]
        assert!(path.is_empty());
    }

    #[test]
    fn test_register_duplicate_name_fails() {
        let initializer: fn() = || {};
        let _ = Register("duplicate-test", initializer);
        let result = Register("duplicate-test", initializer);
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_command_creation() {
        let cmd = command(&["--help".to_string()]);

        #[cfg(target_os = "linux")]
        {
            assert_eq!(cmd.get_program(), "/proc/self/exe");
        }

        #[cfg(all(unix, not(target_os = "linux")))]
        {
            // On other Unix, command returns Command directly
            assert_eq!(cmd.get_program(), naive_self());
        }

        #[cfg(not(unix))]
        {
            // On unsupported platforms, command returns None
            assert!(cmd.is_none());
        }
    }
}
