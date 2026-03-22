//! SetUID/SetGID for unsupported platforms

use tracing::warn;

use super::{DEFAULT_GID, DEFAULT_UID};

/// SetUID is a no-op on unsupported platforms
pub fn set_uid(uid: &str) {
    if uid != DEFAULT_UID {
        warn!(
            "setting uid is only supported on unix/linux systems (attempted: {})",
            uid
        );
    }
}

/// SetGID is a no-op on unsupported platforms
pub fn set_gid(gid: &str) {
    if gid != DEFAULT_GID {
        warn!(
            "setting gid is only supported on unix/linux systems (attempted: {})",
            gid
        );
    }
}

/// ApplyUIDGID is a no-op on unsupported platforms
pub fn apply_uid_gid(_cmd: &mut std::process::Command, _uid: &str, _gid: &str) {
    // No-op on unsupported platforms
}
