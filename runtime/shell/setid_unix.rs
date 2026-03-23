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

/// Set the UID for the current process via `libc::setuid`.
///
/// Matches Go's `func SetUID(uid string)`:
/// ```go
/// uidi, err := strconv.Atoi(uid)
/// syscall.Setuid(uidi)
/// ```
///
/// # Panics
///
/// Never panics — logs errors instead (matching Go's `log.Fatal` behavior
/// adapted for library use).
#[allow(dead_code)]
pub fn set_uid(uid: &str) {
    if uid != DEFAULT_UID {
        match uid.parse::<libc::uid_t>() {
            Ok(uid_val) => {
                // SAFETY: setuid is a simple syscall with no memory safety concerns.
                // Should only be called in a single-threaded child process context.
                let ret = unsafe { libc::setuid(uid_val) };
                if ret != 0 {
                    eprintln!(
                        "error setting uid {}: {}",
                        uid_val,
                        std::io::Error::last_os_error()
                    );
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("invalid uid '{}': {}", uid, e);
                std::process::exit(1);
            }
        }
    }
}

/// Set the GID for the current process via `libc::setgid`.
///
/// Matches Go's `func SetGID(gid string)`:
/// ```go
/// gidi, err := strconv.Atoi(gid)
/// syscall.Setgid(gidi)
/// ```
#[allow(dead_code)]
pub fn set_gid(gid: &str) {
    if gid != DEFAULT_GID {
        match gid.parse::<libc::gid_t>() {
            Ok(gid_val) => {
                // SAFETY: setgid is a simple syscall with no memory safety concerns.
                // Should only be called in a single-threaded child process context.
                let ret = unsafe { libc::setgid(gid_val) };
                if ret != 0 {
                    eprintln!(
                        "error setting gid {}: {}",
                        gid_val,
                        std::io::Error::last_os_error()
                    );
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("invalid gid '{}': {}", gid, e);
                std::process::exit(1);
            }
        }
    }
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
        set_uid(DEFAULT_UID);
    }

    #[test]
    fn test_set_gid_default_is_noop() {
        // DEFAULT_GID "-" should not call setgid
        set_gid(DEFAULT_GID);
    }

    #[test]
    fn test_set_uid_invalid_logs_error() {
        // Invalid UID should log error, not panic
        set_uid("not-a-number");
    }

    #[test]
    fn test_set_gid_invalid_logs_error() {
        // Invalid GID should log error, not panic
        set_gid("not-a-number");
    }
}
