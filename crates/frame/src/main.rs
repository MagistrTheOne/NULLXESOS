//! FRAME — NULLXES OS Wayland compositor.
//!
//! ## Lifecycle
//!
//! 1. **Boot**: parse env (`NULLXES_BACKEND`, `NULLXES_LOG`), bring up tracing
//!    (journald when running under systemd, otherwise stderr).
//! 2. **Init**: build [`compositor::NullxesState`], wire smithay handlers
//!    (compositor / xdg-shell / wlr-layer-shell / session-lock / data-device /
//!    seat / output / shm / xwayland), open the wayland socket and write its
//!    name into `$XDG_RUNTIME_DIR/nullxes/wayland-display`.
//! 3. **Run**: drive the calloop event loop on the main thread; tokio runtime
//!    on a dedicated thread serves the Control IPC v1 socket and the D-Bus
//!    listeners (`org.freedesktop.login1`, UPower, NetworkManager, PipeWire).
//! 4. **Shutdown**: SIGTERM/SIGINT → `LoopSignal::stop()` → drain in-flight
//!    clients → unmap layer surfaces → release outputs → unlink runtime files
//!    (wayland socket name file, control IPC socket).
//!
//! ## Process model
//!
//! FRAME is one binary that owns the compositor, XWayland WM, and the on-screen
//! PANEL (rendered as a memory render-element overlay after each frame).
//! External clients (`launcher`, `slate`, `nx-lock`, `nx-notif`, `nx-settings`,
//! `nx-greet`) connect via the wayland socket and the Control IPC socket.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use anyhow::{Context, Result};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod backend;
mod config;
mod handlers;
mod idle;
mod input;
mod ipc_server;
mod panel;
mod render;
mod runtime;
mod signals;
mod state;
mod system;
mod workspace;
mod xwm;

fn main() -> Result<()> {
    init_tracing();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "FRAME compositor starting"
    );

    let cfg = config::FrameConfig::load();
    info!(?cfg, "configuration loaded");

    runtime::Runtime::install_global()
        .context("failed to install async runtime")?;

    let backend = match std::env::var("NULLXES_BACKEND").as_deref() {
        Ok("drm")   => SelectedBackend::Drm,
        Ok("winit") => SelectedBackend::Winit,
        Ok(other)   => {
            error!(backend = %other, "unknown NULLXES_BACKEND, defaulting to winit");
            SelectedBackend::Winit
        }
        Err(_) => detect_backend(),
    };

    match backend {
        SelectedBackend::Winit => {
            #[cfg(feature = "winit")]
            { return backend::winit::run(cfg); }
            #[cfg(not(feature = "winit"))]
            { anyhow::bail!("winit backend not compiled in"); }
        }
        SelectedBackend::Drm => {
            #[cfg(feature = "drm")]
            { return backend::drm::run(cfg); }
            #[cfg(not(feature = "drm"))]
            { anyhow::bail!("drm backend not compiled in"); }
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_env("NULLXES_LOG")
        .unwrap_or_else(|_| EnvFilter::new("frame=info,smithay=warn"));
    let stderr = tracing_subscriber::fmt::layer().with_target(true);

    let journald = tracing_journald::layer().ok();

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr);

    if let Some(j) = journald {
        let _ = subscriber.with(j).try_init();
    } else {
        let _ = subscriber.try_init();
    }
}

#[derive(Debug, Clone, Copy)]
enum SelectedBackend { Winit, Drm }

fn detect_backend() -> SelectedBackend {
    // If we already have a Wayland or X11 session, dev-mode (winit) is correct.
    // On a bare TTY (no display servers) we go DRM.
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("DISPLAY").is_some()
    {
        SelectedBackend::Winit
    } else {
        SelectedBackend::Drm
    }
}
