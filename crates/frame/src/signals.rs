//! Signal handling for FRAME.
//!
//! Installs a calloop event source that fires on SIGTERM / SIGINT / SIGHUP
//! and stops the wayland event loop cleanly. We avoid signalfd-via-tokio
//! because the wayland event loop must observe the signal directly to drain
//! clients before exit.

use anyhow::Result;
use calloop::{signals::{Signal, Signals}, LoopHandle};
use tracing::{info, warn};

use crate::state::NullxesState;

pub fn install(handle: &LoopHandle<'_, NullxesState>) -> Result<()> {
    let signals = Signals::new(&[Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP])?;
    handle
        .insert_source(signals, |event, _, state| {
            let sig = event.signal();
            info!(?sig, "signal received → stopping event loop");
            state.shutdown_reason = Some(sig);
            state.loop_signal.stop();
        })
        .map_err(|e| anyhow::anyhow!("failed to insert signal source: {e}"))?;

    // Ignore SIGPIPE — we will see EPIPE on socket writes and handle it.
    // Safety: signal() with SIG_IGN is documented as POSIX-compliant.
    unsafe {
        let _ = libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }

    if std::env::var_os("NULLXES_LOG_BACKTRACE").is_some() {
        warn!("NULLXES_LOG_BACKTRACE is set — RUST_BACKTRACE=full");
        std::env::set_var("RUST_BACKTRACE", "full");
    }
    Ok(())
}
