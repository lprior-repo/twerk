//! SetUID/SetGID for unsupported platforms

use super::{DEFAULT_GID, DEFAULT_UID};

/// SetUID is a no-op on unsupported platforms
pub fn set_uid(uid: &str) {
    if uid != DEFAULT_UID {
        eprintln!(
            "setting uid is only supported on unix/linux systems (attempted: {})",
            uid
        );
        std::process::exit(1);
    }
}

/// SetGID is a no-op on unsupported platforms
pub fn set_gid(gid: &str) {
    if gid != DEFAULT_GID {
        eprintln!(
            "setting gid is only supported on unix/linux systems (attempted: {})",
            gid
        );
        std::process::exit(1);
    }
}

/// ApplyUIDGID is a no-op on unsupported platforms
pub fn apply_uid_gid(_cmd: &std::process::Command, _uid: &str, _gid: &str) {
    // No-op on unsupported platforms
}
