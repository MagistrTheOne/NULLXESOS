//! NX-SETTINGS — NULLXES settings UI.
//!
//! Sections (Stage 1):
//!   - **Theme**: edits `~/.config/nullxes/theme.toml`.
//!   - **Input**: edits `~/.config/nullxes/frame.toml` `[input]` block.
//!   - **Idle**:  edits `~/.config/nullxes/frame.toml` `[idle]` block.
//!
//! After a config write, NX-SETTINGS sends `Reload.Config` over Control IPC v1
//! so the running compositor re-reads on the spot. No daemon restart.
//!
//! Display / Audio / Network sections are wired in 0.2 once the compositor
//! exposes wlr-output-management (Display) and PipeWire/NM helpers stabilise.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod conf;
mod render;
mod state;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("nx_settings=info")))
        .compact()
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "NX-SETTINGS starting");

    state::run().await
}
