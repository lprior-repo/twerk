//! Reexec utilities for self-reexecution pattern.
//!
//! This module provides functionality for re-executing the current binary,
//! mirroring the Docker/pkg/reexec pattern used in the Go tork implementation.
//!
//! # Architecture
//!
//! - **Data**: `Command` struct holds executable path and arguments
//! - **Calc**: Path resolution via `naive_self`
//! - **Actions**: Process spawning via std::process::Command
//!
//! # Go Parity Notes
//!
//! - `register`: Go panics on duplicate; Rust returns `Result` (preferred)
//! - `command`: Go sets `SysProcAttr.Pdeathsig=SIGTERM` (requires `unsafe`
//!   in Rust); Rust uses `process_group(0)` as the closest safe equivalent
//! - `command`: Go sets `Args: args` (argv[0]=args[0]); Rust uses `arg0`
//!   to match this behavior

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during reexec operations.
#[derive(Debug, Error)]
pub enum ReexecError {
    /// Go panics on duplicate registration; Rust returns this error instead.
    #[error("reexec func already registered under name {0:?}")]
    AlreadyRegistered(String),
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
/// Matches Go's `naiveSelf` exactly:
/// 1. If `os.Args[0]` is a bare name (no path separator), try `LookPath` (PATH search)
/// 2. Convert relative paths to absolute via `filepath.Abs` (prepend cwd)
/// 3. Return the original name as last resort
#[must_use]
pub fn naive_self() -> PathBuf {
    let name = env::args()
        .next()
        .unwrap_or_else(|| "/proc/self/exe".to_string());
    resolve_naive_self(&name)
}

/// Internal: resolves a program name using the Go naiveSelf algorithm.
/// Separated for testability.
#[must_use]
fn resolve_naive_self(name: &str) -> PathBuf {
    let path = Path::new(name);

    // Go: `if filepath.Base(name) == name` — bare filename, search PATH
    if path.parent().is_none() {
        if let Some(found) = env::var("PATH").ok().and_then(|path_var| {
            path_var
                .split(':')
                .map(|dir| PathBuf::from(dir).join(name))
                .find(|candidate| candidate.exists())
        }) {
            return found;
        }
    }

    // Go: `filepath.Abs(name)` — convert to absolute (no-op if already absolute)
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = env::current_dir() {
        cwd.join(name)
    } else {
        PathBuf::from(name)
    }
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
/// Matches Go's `func Command(args ...string) *exec.Cmd`:
/// - `Path` is set to the current binary (via `self_path()`)
/// - `Args` is set to the provided arguments directly
///   (i.e., `argv[0] = args[0]`, NOT the binary path)
///
/// On Linux, Go sets `SysProcAttr.Pdeathsig = SIGTERM` for process cleanup.
/// Rust cannot set this safely (requires `unsafe`), so we use
/// `process_group(0)` to create an isolated process group instead.
#[must_use]
pub fn command(args: &[String]) -> Command {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let mut cmd = Command::new(self_path());
        // Go: `Args: args` means argv[0] = args[0], not the binary path.
        // This is critical for the reexec pattern where Init() checks argv[0]
        // to find the registered initializer.
        if let Some((first, rest)) = args.split_first() {
            cmd.arg0(first);
            cmd.args(rest);
        }
        // Create new process group for cleanup safety (closest safe equivalent
        // to Go's SysProcAttr.Pdeathsig = SIGTERM)
        cmd.process_group(0);
        cmd
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::os::unix::process::CommandExt;

        let mut cmd = Command::new(self_path());
        if let Some((first, rest)) = args.split_first() {
            cmd.arg0(first);
            cmd.args(rest);
        }
        cmd
    }

    #[cfg(not(unix))]
    {
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
    fn test_naive_self_bare_name_searches_path() {
        // Go: naiveSelf("ls") → should find /usr/bin/ls or similar
        let path = resolve_naive_self("ls");
        assert!(path.to_string_lossy().contains("ls"));
    }

    #[test]
    fn test_naive_self_absolute_path_passthrough() {
        // Go: naiveSelf("/usr/bin/ls") → returns "/usr/bin/ls" unchanged
        let path = resolve_naive_self("/usr/bin/ls");
        assert_eq!(PathBuf::from("/usr/bin/ls"), path);
    }

    #[test]
    fn test_naive_self_relative_path_absolved() {
        // Go: naiveSelf("./foo") → returns cwd + "/foo" (filepath.Abs)
        let path = resolve_naive_self("./foo");
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().ends_with("/foo"));
    }

    #[test]
    fn test_self_path_returns_valid() {
        let path = self_path();
        assert!(!path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_command_creation() {
        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let cmd = command(&args);

        // The command should have the correct program
        // We can't easily check Path directly, but we can verify it doesn't panic
        let program = cmd.get_program();
        assert!(!program.to_string_lossy().is_empty());
    }

    #[test]
    fn test_command_args_passed() {
        // Go: Command("arg1", "arg2") → Args=["arg1","arg2"]
        // Rust: command(&["arg1","arg2"]) → argv[0]="arg1", argv[1]="arg2"
        // The program path is set via Command::new(), but arg0 overrides argv[0]
        let args = vec!["--test".to_string(), "value".to_string()];
        let cmd = command(&args);

        // With arg0, the first arg becomes argv[0] (program name position)
        // and the rest follow as argv[1..]
        let all_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        // command() splits first/rest via arg0, so get_args() returns ["value"]
        assert_eq!(1, all_args.len());
        assert_eq!("value", all_args[0]);
    }
}
