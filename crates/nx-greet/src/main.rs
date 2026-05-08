//! NX-GREET — NULLXES greeter.
//!
//! greetd flow:
//!   1. Connect to greetd at `$GREETD_SOCK`.
//!   2. `CreateSession { username }` → `AuthMessage` (typically password
//!      prompt, but greetd supports multi-step auth).
//!   3. Reply with `PostAuthMessageResponse { response: password }`.
//!   4. On success, `StartSession { cmd: ["frame"] }` and exit; greetd
//!      replaces our process with the user session.
//!   5. On `AuthError` → reset and retry input.
//!
//! UI: identical look to NX-LOCK but with username editable on the first row
//! and a password row below.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod ipc;
mod render;
mod state;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("nx_greet=info")))
        .compact()
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "NX-GREET starting");
    state::run()
}
