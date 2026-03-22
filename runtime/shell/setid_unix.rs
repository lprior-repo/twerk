//! SetUID/SetGID for Unix-like systems (Linux, macOS, FreeBSD)

use std::os::unix::process::CommandExt;
use tracing::error;

use super::{DEFAULT_GID, DEFAULT_UID};

/// Set the UID for the current process (Unix only)
pub fn set_uid(uid: &str) {
    if uid != DEFAULT_UID {
        match uid.parse::<u32>() {
            Ok(uid_val) => {
                if let Err(e) = std::process::Command::new("true").uid(uid_val).spawn() {
                    error!("error setting uid: {}", e);
                }
            }
            Err(e) => {
                error!("invalid uid '{}': {}", uid, e);
            }
        }
    }
}

/// Set the GID for the current process (Unix only)
pub fn set_gid(gid: &str) {
    if gid != DEFAULT_GID {
        match gid.parse::<u32>() {
            Ok(gid_val) => {
                if let Err(e) = std::process::Command::new("true").gid(gid_val).spawn() {
                    error!("error setting gid: {}", e);
                }
            }
            Err(e) => {
                error!("invalid gid '{}': {}", gid, e);
            }
        }
    }
}

/// Apply UID and GID settings to a Command
pub fn apply_uid_gid(cmd: &mut std::process::Command, uid: &str, gid: &str) {
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
