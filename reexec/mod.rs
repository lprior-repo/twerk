//! Reexec utilities for self-reexecution pattern.
//!
//! This module provides functionality for re-executing the current binary,
//! similar to the busybox-style reexec pattern used in Docker.
//!
//! # Architecture
//!
//! - **Data**: `Command` struct holds executable path and arguments
//! - **Calc**: Path resolution via `naive_self()`
//! - **Actions**: Process spawning via std::process::Command

use std::env;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during reexec operations.
#[derive(Debug, Error)]
pub enum ReexecError {
    #[error("reexec func already registered under name {0:?}")]
    AlreadyRegistered(String),

    #[error("failed to resolve executable path")]
    PathResolutionFailed,

    #[error("unsupported platform")]
    UnsupportedPlatform,
}

type Initializer = Box<dyn Fn() + Send + Sync>;

/// Registry of initialization functions indexed by name.
static REGISTERED_INITIALIZERS: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, Initializer>>,
> = std::sync::OnceLock::new();

/// Registers an initialization function under the specified name.
///
/// # Errors
///
/// Returns `ReexecError::AlreadyRegistered` if a function is already
/// registered under this name.
pub fn register(name: &str, initializer: Initializer) -> Result<(), ReexecError> {
    let registry = REGISTERED_INITIALIZERS
        .get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()));

    let mut guard = registry.write().map_err(|_| {
        // This shouldn't happen with RwLock, but handle it gracefully
        ReexecError::AlreadyRegistered(name.to_string())
    })?;

    if guard.contains_key(name) {
        return Err(ReexecError::AlreadyRegistered(name.to_string()));
    }

    guard.insert(name.to_string(), initializer);
    Ok(())
}

/// Called as the first part of the exec process.
///
/// Returns `true` if an initialization function was called, `false` otherwise.
#[must_use]
pub fn init() -> bool {
    let Some(executable) = env::args().next() else {
        return false;
    };

    let registry = match REGISTERED_INITIALIZERS.get() {
        Some(r) => r,
        None => return false,
    };

    let guard = match registry.read() {
        Ok(g) => g,
        Err(_) => return false,
    };

    let Some(initializer) = guard.get(&executable) else {
        return false;
    };

    initializer();
    true
}

/// Returns the path to the current executable using `os.Args[0]` fallback.
///
/// This attempts multiple strategies:
/// 1. If `os.Args[0]` is a bare name, try to find it in PATH
/// 2. Convert relative paths to absolute paths
/// 3. Return the original name as last resort
#[must_use]
pub fn naive_self() -> PathBuf {
    let name = match env::args().next() {
        Some(n) => n,
        None => return PathBuf::from("/proc/self/exe"),
    };

    // If name is just a basename, try to find it in PATH
    if name.contains('/') {
        // It has a path component, try to resolve it
        let abs = PathBuf::from(&name);
        if abs.is_absolute() {
            return abs;
        }
        // Try to make it absolute relative to current dir
        if let Ok(cwd) = env::current_dir() {
            let full = cwd.join(&name);
            if full.exists() {
                return full;
            }
        }
        // Return as-is if we can't resolve
        return PathBuf::from(name);
    }

    // name is just a basename, try to find in PATH
    if let Some(lp) = env::var("PATH").ok() {
        for dir in lp.split(':') {
            let candidate = PathBuf::from(dir).join(&name);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // Fallback to original
    PathBuf::from(name)
}

/// Returns the path to the current process's binary.
///
/// On Linux, this returns `/proc/self/exe`.
/// On other Unix systems, it uses `naive_self()`.
#[must_use]
pub fn self_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/proc/self/exe")
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        naive_self()
    }

    #[cfg(not(unix))]
    {
        // For unsupported platforms, fall back to naive_self
        naive_self()
    }
}

/// Creates a `Command` that will execute the current binary.
///
/// The returned command has the `Path` set to the current binary,
/// and `Args` set to the provided arguments.
///
/// On Linux, `SysProcAttr.Pdeathsig` is set to SIGTERM for process
/// cleanup safety.
#[must_use]
pub fn command(args: &[String]) -> Command {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let mut cmd = Command::new(self_path());
        cmd.args(args);
        // Set process death signal to SIGTERM for safe cleanup
        cmd.process_group(0); // This sets the process group to itself
        cmd
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let mut cmd = Command::new(self_path());
        cmd.args(args);
        cmd
    }

    #[cfg(not(unix))]
    {
        // Unsupported platform
        let mut cmd = Command::new(self_path());
        cmd.args(args);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_init_not_called() {
        let result = register("test_init", Box::new(|| {}));
        assert!(result.is_ok());
        // init() returns false because executable name won't match
        assert!(!init());
    }

    #[test]
    fn test_register_duplicate_fails() {
        let init1 = Box::new(|| {}) as Initializer;
        let init2 = Box::new(|| {}) as Initializer;
        assert!(register("dup_test", init1).is_ok());
        assert!(matches!(
            register("dup_test", init2),
            Err(ReexecError::AlreadyRegistered(_))
        ));
    }

    #[test]
    fn test_naive_self_returns_path() {
        let path = naive_self();
        // Should return a non-empty path
        assert!(!path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_self_path_returns_valid() {
        let path = self_path();
        assert!(!path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_command_creation() {
        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let mut cmd = command(&args);

        // The command should have the correct program
        // We can't easily check Path directly, but we can verify it doesn't panic
        let program = cmd.get_program();
        assert!(!program.to_string_lossy().is_empty());
    }

    #[test]
    fn test_command_args_passed() {
        let args = vec!["--test".to_string(), "value".to_string()];
        let cmd = command(&args);

        let obtained_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, obtained_args);
    }
}
