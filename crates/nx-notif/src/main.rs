//! NX-NOTIF — notification daemon.
//!
//! - Implements `org.freedesktop.Notifications` on the user session bus
//!   (interface methods: `Notify`, `CloseNotification`, `GetCapabilities`,
//!   `GetServerInformation`; signals: `NotificationClosed`, `ActionInvoked`).
//! - Renders queued notifications as a stack of layer-shell surfaces anchored
//!   top-right of the primary output.
//! - Persists history (capped at 1 MiB rotated) to
//!   `$XDG_STATE_HOME/nullxes/notifications.json`.
//!
//! Process model:
//!   - Main thread: tokio runtime running the zbus connection.
//!   - Inner blocking task: wayland event loop on a dedicated thread.
//!   - Communication: tokio mpsc bounded(64) for new notifications,
//!     broadcast for "closed" events.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod dbus;
mod history;
mod render;
mod surface;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("nx_notif=info")))
        .compact()
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "NX-NOTIF starting");

    // Spawn the wayland thread; receives notifications via tokio mpsc.
    let (tx_to_wl, rx_in_wl) = tokio::sync::mpsc::channel::<surface::Notif>(64);
    let (tx_closed, rx_closed) = tokio::sync::broadcast::channel::<u32>(64);

    let _wayland_join = std::thread::spawn(move || {
        if let Err(e) = surface::wayland_thread(rx_in_wl, tx_closed) {
            tracing::error!(?e, "wayland thread crashed");
        }
    });

    let history = history::open_or_create()?;
    let server = dbus::Server::new(tx_to_wl, rx_closed, history);
    let conn = server.start().await?;

    // Sleep until SIGTERM/SIGINT.
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown");
    drop(conn);
    Ok(())
}
