//! SLATE — NULLXES terminal emulator.
//!
//! Architecture:
//!   - **PTY thread** owns an `alacritty_terminal::tty::Pty` and drives the
//!     `EventLoop` from alacritty_terminal. Updates the shared `Term` state
//!     under a lock. Sends a wakeup byte to the wayland thread on dirty.
//!   - **Wayland thread** holds the xdg_toplevel surface, an SHM
//!     double-buffer, the keyboard state, and renders the visible viewport
//!     of the terminal grid via `theme::text::TextRenderer`.
//!   - **Shutdown**: child exit (SIGCHLD) → PTY thread joins → wayland thread
//!     sees `should_close` and exits the dispatch loop.
//!
//! Lifecycle invariants:
//!   - One PTY per process.
//!   - Default shell is `getpwuid(getuid()).pw_shell`, fallback `/bin/sh`.
//!   - Cursor blink driven by `theme::motion::Easing::Linear` over a 1s period.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod render;
mod state;
mod term;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("slate=info")))
        .compact()
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "SLATE terminal starting");

    let theme_path = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nullxes")
        .join("theme.toml");
    let theme = theme::Theme::load_or_default(&theme_path);

    state::run(theme)
}
