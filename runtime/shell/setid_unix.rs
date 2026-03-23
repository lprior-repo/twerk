//! SetUID/SetGID for Unix-like systems (Linux, macOS, FreeBSD).
//!
//! Provides direct syscall-level UID/GID setting, matching Go's
//! `syscall.Setuid`/`syscall.Setgid` which are called in the reexec'd
//! child process before executing the actual command.
//!
//! # Safety
//!
//! These functions call raw libc syscalls via FFI. They should only
//! be called in a child process context (after fork, before exec) as
//! changing UID/GID of a multi-threaded process is undefined behavior.

use super::{DEFAULT_GID, DEFAULT_UID};
use std::fmt;

/// Errors that can occur when setting UID/GID.
#[derive(Debug)]
pub enum SetIdError {
    /// Failed to parse the ID string to an integer.
    ParseError(String),
    /// The syscall itself failed.
    SyscallError(String),
}

impl fmt::Display for SetIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetIdError::ParseError(s) => write!(f, "invalid id: {}", s),
            SetIdError::SyscallError(s) => write!(f, "syscall error: {}", s),
        }
    }
}

impl std::error::Error for SetIdError {}

/// Set the UID for the current process via `libc::setuid`.
///
/// Returns `Ok(())` on success, or an error if parsing or the syscall fails.
/// In the reexec context, callers should exit on error.
///
/// Matches Go's `func SetUID(uid string)`:
/// ```go
/// uidi, err := strconv.Atoi(uid)
/// syscall.Setuid(uidi)
/// ```
#[allow(dead_code)]
pub fn set_uid(uid: &str) -> Result<(), SetIdError> {
    if uid != DEFAULT_UID {
        let uid_val = uid
            .parse::<libc::uid_t>()
            .map_err(|e| SetIdError::ParseError(format!("invalid uid '{}': {}", uid, e)))?;
        // SAFETY: setuid is a simple syscall with no memory safety concerns.
        // Should only be called in a single-threaded child process context.
        let ret = unsafe { libc::setuid(uid_val) };
        if ret != 0 {
            return Err(SetIdError::SyscallError(format!(
                "error setting uid {}: {}",
                uid_val,
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}

/// Set the GID for the current process via `libc::setgid`.
///
/// Returns `Ok(())` on success, or an error if parsing or the syscall fails.
/// In the reexec context, callers should exit on error.
///
/// Matches Go's `func SetGID(gid string)`:
/// ```go
/// gidi, err := strconv.Atoi(gid)
/// syscall.Setgid(gidi)
/// ```
#[allow(dead_code)]
pub fn set_gid(gid: &str) -> Result<(), SetIdError> {
    if gid != DEFAULT_GID {
        let gid_val = gid
            .parse::<libc::gid_t>()
            .map_err(|e| SetIdError::ParseError(format!("invalid gid '{}': {}", gid, e)))?;
        // SAFETY: setgid is a simple syscall with no memory safety concerns.
        // Should only be called in a single-threaded child process context.
        let ret = unsafe { libc::setgid(gid_val) };
        if ret != 0 {
            return Err(SetIdError::SyscallError(format!(
                "error setting gid {}: {}",
                gid_val,
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}

/// Apply UID and GID settings to a `std::process::Command` via `CommandExt`.
///
/// This is the safe Rust approach (sets UID/GID on the child process via
/// `pre_exec`), used by the default reexec function instead of the
/// direct `set_uid`/`set_gid` calls.
#[allow(dead_code)]
pub fn apply_uid_gid(cmd: &mut std::process::Command, uid: &str, gid: &str) {
    use std::os::unix::process::CommandExt;
    if uid != DEFAULT_UID {
        if let Ok(uid_val) = uid.parse::<u32>() {
            cmd.uid(uid_val);
        }
    }
    if gid != DEFAULT_GID {
        if let Ok(gid_val) = gid.parse::<u32>() {
            cmd.gid(gid_val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_uid_default_is_noop() {
        // DEFAULT_UID "-" should not call setuid
        assert!(set_uid(DEFAULT_UID).is_ok());
    }

    #[test]
    fn test_set_gid_default_is_noop() {
        // DEFAULT_GID "-" should not call setgid
        assert!(set_gid(DEFAULT_GID).is_ok());
    }

    #[test]
    fn test_set_uid_invalid_returns_error() {
        // Invalid UID should return error
        let result = set_uid("not-a-number");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid uid"));
    }

    #[test]
    fn test_set_gid_invalid_returns_error() {
        // Invalid GID should return error
        let result = set_gid("not-a-number");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid gid"));
    }
}
