//! Reexec command builder for privilege dropping
//!
//! This module provides reexec functionality following the Docker/pkg/reexec pattern:
//! - Uses `self_path()` (e.g., `/proc/self/exe`) as the program path
//! - Sets `argv[0]` to the first argument via `arg0()`
//! - Passes remaining arguments to the subprocess

use std::path::PathBuf;

use tokio::process::Command;

/// Returns the path to the current executable for reexec spawning.
///
/// On Linux, this returns `/proc/self/exe`. On other Unix systems,
/// it falls back to resolving the executable via PATH.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn self_path() -> PathBuf {
    PathBuf::from("/proc/self/exe")
}

#[cfg(all(unix, not(target_os = "linux")))]
#[allow(dead_code)]
fn self_path() -> PathBuf {
    std::env::args()
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/proc/self/exe"))
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn self_path() -> PathBuf {
    std::env::args()
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/proc/self/exe"))
}

/// Function type for reexec-style command execution
/// Takes arguments and returns a configured Command
pub type ReexecCommand = Box<dyn Fn(&[String]) -> Command + Send + Sync>;

/// Create a reexec command following the Docker/pkg/reexec pattern.
///
/// Uses `self_path()` (e.g., `/proc/self/exe`) as the program and sets
/// `argv[0]` to `args[0]` via `arg0()`. This allows the binary to reexec
/// itself with a different handler name while maintaining the same executable.
///
/// # Arguments
///
/// * `args` - Arguments where `args[0]` becomes argv[0] and `args[1..]` are passed
///
/// # Example
///
/// ```ignore
/// // Spawns /proc/self/exe with argv[0]="myapp"
/// let cmd = reexec_from_std(&["myapp".to_string(), "arg1".to_string()]);
/// ```
#[must_use]
#[allow(dead_code)]
pub fn reexec_from_std(args: &[String]) -> Command {
    let mut cmd = Command::new(self_path());
    if let Some((first, rest)) = args.split_first() {
        #[cfg(unix)]
        {
            // arg0 sets argv[0] without affecting the executable path
            cmd.arg0(first).args(rest);
        }
        #[cfg(not(unix))]
        {
            cmd.arg(first).args(rest);
        }
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reexec_from_std_single_arg() {
        // With reexec pattern, program is self_path, argv[0] is args[0]
        let args = vec!["reexec".to_string()];
        let cmd = reexec_from_std(&args);
        let program = cmd.as_std().get_program().to_string_lossy().into_owned();
        // Program should be self_path (e.g., /proc/self/exe on Linux)
        assert!(!program.is_empty());
        // argv[0] should be "reexec"
        // Note: argv[0] is set via arg0() on Unix
    }

    #[test]
    fn test_reexec_from_std_multiple_args() {
        let args = vec![
            "myapp".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        let cmd = reexec_from_std(&args);
        let program = cmd.as_std().get_program().to_string_lossy().into_owned();
        // Program is self_path
        assert!(!program.is_empty());
        // Remaining args should be passed through
        let cmd_args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(cmd_args, vec!["hello", "world"]);
    }
}
