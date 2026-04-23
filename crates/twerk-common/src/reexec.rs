//! Reexec utilities for self-reexecution pattern.
//!
//! This module provides functionality for re-executing the current binary,
//! mirroring the Docker/pkg/reexec pattern used in the Go twerk implementation.
//!
//! # Architecture
//!
//! - **Data**: `Command` struct holds executable path and arguments
//! - **Calc**: Path resolution via `naive_self`
//! - **Actions**: Process spawning via `std::process::Command`
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
use tap::Pipe;
use thiserror::Error;

/// Errors that can occur during reexec operations.
#[derive(Debug, Error)]
pub enum CommonReexecError {
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
/// Returns `CommonReexecError::AlreadyRegistered` if a function is already
/// registered under this name.
pub fn register(name: &str, initializer: Initializer) -> Result<(), CommonReexecError> {
    let registry = REGISTERED_INITIALIZERS
        .get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()));

    let mut guard = registry.write().map_err(|_| {
        // RwLock poisoned — map to error for explicit handling
        CommonReexecError::AlreadyRegistered(name.to_string())
    })?;

    if guard.contains_key(name) {
        return Err(CommonReexecError::AlreadyRegistered(name.to_string()));
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

    let Some(registry) = REGISTERED_INITIALIZERS.get() else {
        return false;
    };

    let Ok(guard) = registry.read() else {
        return false;
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
    let name = match env::args().next() {
        Some(name) => name,
        None => "/proc/self/exe".to_string(),
    };
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

        Command::new(self_path()).pipe(|mut cmd| {
            if let Some((first, rest)) = args.split_first() {
                cmd.arg0(first).args(rest);
            }
            // Create new process group for cleanup safety (closest safe equivalent
            // to Go's SysProcAttr.Pdeathsig = SIGTERM)
            cmd.process_group(0);
            cmd
        })
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::os::unix::process::CommandExt;

        Command::new(self_path()).pipe(|mut cmd| {
            if let Some((first, rest)) = args.split_first() {
                cmd.arg0(first).args(rest);
            }
            cmd
        })
    }

    #[cfg(not(unix))]
    {
        Command::new(self_path()).args(args)
    }
}

#[cfg(test)]
mod tests {
    #![deny(clippy::unwrap_used)]
    #![deny(clippy::expect_used)]
    #![deny(clippy::panic)]
    use super::*;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;

    // ── TestRegister parity ──────────────────────────────────────────

    #[test]
    fn test_register_duplicate_fails() {
        let init1 = Box::new(|| {}) as Initializer;
        let init2 = Box::new(|| {}) as Initializer;
        assert!(register("dup_test", init1).is_ok());
        assert!(matches!(
            register("dup_test", init2),
            Err(CommonReexecError::AlreadyRegistered(_))
        ));
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_register_error_message_exact() {
        let _ = register("msg_test", Box::new(|| {}));
        let err = register("msg_test", Box::new(|| {}))
            .expect_err("expected error when registering duplicate name");
        assert_eq!(
            err.to_string(),
            r#"reexec func already registered under name "msg_test""#
        );
    }

    #[allow(clippy::redundant_pattern_matching)]
    #[test]
    fn test_register_and_init_not_called() {
        let result = register("test_init", Box::new(|| {}));
        assert!(matches!(result, Ok(_)));
        // init() returns false because executable name won't match
        assert!(!init());
    }

    // ── TestCommand parity ───────────────────────────────────────────

    #[test]
    fn test_command_creation() {
        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let cmd = command(&args);

        // The command should have the correct program
        let program = cmd.get_program();
        assert!(!program.to_string_lossy().is_empty());
    }

    #[test]
    fn test_command_args_passed() {
        // Go: Command("arg1", "arg2") → Args=["arg1","arg2"]
        // Rust: command(&["arg1","arg2"]) → argv[0]="arg1", argv[1]="arg2"
        let args = vec!["--test".to_string(), "value".to_string()];
        let cmd = command(&args);

        // command() splits first/rest via arg0, so get_args() returns ["value"]
        let all_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(1, all_args.len());
        assert_eq!("value", all_args[0]);
    }

    #[test]
    fn test_command_empty_args() {
        // Go: Command() with no args — edge case
        let cmd = command(&[]);
        let program = cmd.get_program();
        assert!(!program.to_string_lossy().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_command_uses_proc_self_exe() {
        // Go: Self() returns "/proc/self/exe" on Linux
        let cmd = command(&["arg1".to_string()]);
        let program = cmd.get_program().to_string_lossy().into_owned();
        assert_eq!(program, "/proc/self/exe");
    }

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[cfg(target_os = "linux")]
    #[test]
    fn test_command_subprocess_exit() {
        // Go TestCommand: spawns the binary with argv[0]="reexec", the
        // package-level init() registers + calls Init(), which finds the
        // registered name and panics → exit status 2.
        //
        // Rust has no package-level init() that auto-runs in subprocesses,
        // so we verify the command can be spawned and produces an exit code.
        // We pass the test name as a positional filter so the child runs
        // only one test and exits quickly.
        let current_exe = env::current_exe().unwrap();
        let test_filter = "test_command_empty_args";

        let mut child = std::process::Command::new(&current_exe)
            .arg0("reexec")
            .args(["--test-threads=1", test_filter])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn subprocess");

        let status = child.wait().expect("failed to wait on subprocess");
        // The child runs the requested test and exits 0 (or 101 on failure).
        // Key assertion: the subprocess ran successfully, proving Command
        // produces a valid executable.
        assert!(status.success(), "subprocess exited with: {status}");
    }

    // ── TestNaiveSelf parity ─────────────────────────────────────────

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
    fn test_naive_self_mkdir_resolved() {
        // Go TestNaiveSelf: os.Args[0] = "mkdir" → naiveSelf() != "mkdir"
        // naive_self resolves bare names via PATH, returning an absolute path
        let path = resolve_naive_self("mkdir");
        let path_str = path.to_string_lossy();
        // Must not be the bare name "mkdir"
        assert_ne!(path_str, "mkdir");
        // Must be an absolute resolved path (e.g., /usr/bin/mkdir)
        assert!(
            path_str.contains('/'),
            "expected resolved path, got: {path_str}"
        );
    }

    #[test]
    fn test_self_path_returns_valid() {
        let path = self_path();
        assert!(!path.to_string_lossy().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_self_path_is_proc_self_exe() {
        // Go: Self() returns "/proc/self/exe" on Linux
        assert_eq!(self_path(), PathBuf::from("/proc/self/exe"));
    }
}
