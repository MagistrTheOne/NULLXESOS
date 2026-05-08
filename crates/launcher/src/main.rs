//! NULLXES LAUNCHER — overlay app launcher.
//!
//! Lifecycle:
//!   1. Acquire single-instance lockfile at `$XDG_RUNTIME_DIR/nullxes/launcher.lock`.
//!      If another instance holds it, exit 0 silently (toggle-like behaviour:
//!      pressing Super while open is a no-op, but the existing instance closes
//!      itself on Esc).
//!   2. Connect to the compositor's wayland socket (defaults to `WAYLAND_DISPLAY`,
//!      falling back to the FRAME-published `wayland-display` file).
//!   3. Bind `wl_compositor`, `wl_shm`, `zwlr_layer_shell_v1`, `wl_seat`.
//!   4. Create an Overlay-layer surface anchored to centre with exclusive keyboard.
//!   5. Run the wayland event loop until the user presses Esc/Enter or the
//!      compositor sends `closed`. Exit 0 in all normal exit paths.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod apps;
mod draw;
mod search;
mod state;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("launcher=info")))
        .compact()
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "NULLXES LAUNCHER starting");

    // Single-instance lockfile.
    let lock = match acquire_lock() {
        Ok(l) => l,
        Err(LockAcquireError::AlreadyHeld) => {
            tracing::info!("another launcher instance already running; exiting");
            return Ok(());
        }
        Err(LockAcquireError::Io(e)) => {
            tracing::error!(?e, "failed to acquire lock; running without exclusivity");
            // We can still run; lockless mode is acceptable for live ISO scenarios.
            LockGuard::lockless()
        }
    };

    let theme_path = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nullxes")
        .join("theme.toml");
    let theme = theme::Theme::load_or_default(&theme_path);

    let result = state::run(theme);
    drop(lock);
    result
}

#[derive(Debug)]
enum LockAcquireError {
    AlreadyHeld,
    Io(std::io::Error),
}

struct LockGuard {
    path: Option<std::path::PathBuf>,
    _fd:  Option<std::os::fd::OwnedFd>,
}

impl LockGuard {
    fn lockless() -> Self {
        Self { path: None, _fd: None }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(p) = &self.path {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn acquire_lock() -> std::result::Result<LockGuard, LockAcquireError> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    let path = ipc::launcher_lock_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LockAcquireError::Io)?;
    }
    let path_c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|e| LockAcquireError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
    // Safety: open(2) is signal-safe; flags O_CREAT|O_RDWR|O_CLOEXEC are valid.
    let fd = unsafe {
        libc::open(
            path_c.as_ptr(),
            libc::O_CREAT | libc::O_RDWR | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(LockAcquireError::Io(std::io::Error::last_os_error()));
    }
    // Safety: flock(2) on a valid fd; we hold ownership.
    let res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if res < 0 {
        // Safety: closing fd we just allocated.
        let _ = unsafe { libc::close(fd) };
        return Err(LockAcquireError::AlreadyHeld);
    }
    // Safety: fd is a valid owned descriptor we just opened.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    Ok(LockGuard { path: Some(path), _fd: Some(owned) })
}
