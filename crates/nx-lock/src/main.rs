//! NX-LOCK — NULLXES session-lock client.
//!
//! Speaks `ext-session-lock-v1` to the FRAME compositor:
//!   1. `ext_session_lock_manager_v1::lock` → wait for `locked`.
//!   2. Create one `ext_session_lock_surface_v1` per `wl_output`.
//!   3. Render the lock UI (avatar + password field + retry counter) into
//!      each surface; attach + ack_configure on every configure.
//!   4. On Enter → call PAM (`nullxes-lock` service); on success
//!      `unlock_and_destroy` → exit 0. On failure → bump retry counter.
//!   5. On exit (any path) the lock manager destroys our resources; the
//!      compositor restores keyboard focus to the previous window stack.
//!
//! Robustness:
//!   - PAM authentication runs on a worker thread so we keep dispatching
//!     wayland events while it's pending.
//!   - On a fail we sleep `theme::motion::FAST` to throttle brute force.
//!   - Lockfile-style guard ensures a second invocation simply exits 0.

#![deny(clippy::unwrap_used, clippy::expect_used)]

mod pam_auth;
mod render;
mod state;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

const PAM_SERVICE: &str = "nullxes-lock";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("NULLXES_LOG")
            .unwrap_or_else(|_| EnvFilter::new("nx_lock=info")))
        .compact()
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "NX-LOCK starting");

    state::run_main(PAM_SERVICE)
}
