//! Canonical socket path for the FRAME control IPC.

use std::path::PathBuf;

const SOCKET_NAME:    &str = "frame.sock";
const RUNTIME_SUBDIR: &str = "nullxes";

/// Canonical path: `$XDG_RUNTIME_DIR/nullxes/frame.sock`.
/// Falls back to `/tmp/nullxes-<uid>/frame.sock` if XDG_RUNTIME_DIR is unset.
pub fn socket_path() -> PathBuf {
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join(RUNTIME_SUBDIR).join(SOCKET_NAME);
    }
    // Safety: getuid() is always-safe libc call returning the real user id.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/nullxes-{uid}")).join(SOCKET_NAME)
}

/// Path to the file FRAME writes its `WAYLAND_DISPLAY` socket name into.
pub fn wayland_display_path() -> PathBuf {
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join(RUNTIME_SUBDIR).join("wayland-display");
    }
    // Safety: getuid() is always-safe libc call returning the real user id.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/nullxes-{uid}")).join("wayland-display")
}

/// Path to the launcher single-instance lockfile.
pub fn launcher_lock_path() -> PathBuf {
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join(RUNTIME_SUBDIR).join("launcher.lock");
    }
    // Safety: getuid() is always-safe libc call returning the real user id.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/nullxes-{uid}")).join("launcher.lock")
}
